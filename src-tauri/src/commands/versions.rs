use tauri::State;

use serde_json::json;

use crate::{
    api::github::{repos, writes},
    domain::{
        sync::SyncResource,
        version::{self, CarryOver, CloseReport, CloseRules, Version, VersionCloseInput, VersionDetail},
    },
    error::{AppError, AppResult},
    state::AppState,
    store::RepoCache,
    sync,
};

/// Sprints et paliers, triés par semver. Lus dans le cache, milestones resynchronisés
/// au plus une fois par minute ; le sprint en cours du projet suit (milestone fermé ou ajouté).
#[tauri::command]
pub async fn versions_list(state: State<'_, AppState>) -> AppResult<Vec<Version>> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::Milestones).await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);

    let current = repos::current_version(&versions);
    let mut project = state.project.write().await;
    if let Some(p) = project.as_mut().filter(|p| p.current_version != current) {
        p.current_version = current;
        let saved = p.clone();
        state.settings.update(|s| s.last_project = Some(saved))?;
    }
    Ok(versions)
}

/// Version en cours : portée, burndown, rythme.
#[tauri::command]
pub async fn version_current(state: State<'_, AppState>) -> AppResult<VersionDetail> {
    state.require_project().await?;
    Err(AppError::NotImplemented("version_current"))
}

/// Clôture la version en cours et passe à `input.next`. Non réversible, jamais mise en
/// file hors-ligne : tout est lu et vérifié avant la première écriture, puis, dans l'ordre :
/// milestone suivant créé (ou daté), issues ouvertes reportées, milestone fermé, tag posé
/// sur la branche par défaut, brouillon de release si demandé. Si une étape échoue, l'erreur
/// dit ce qui a déjà été écrit sur GitHub.
#[tauri::command]
pub async fn version_close(
    state: State<'_, AppState>,
    input: VersionCloseInput,
) -> AppResult<CloseReport> {
    let project = state.require_project().await?;
    let current = project
        .current_version
        .clone()
        .ok_or_else(|| AppError::Validation("Aucune version en cours à clôturer.".into()))?;
    let cache = state.repo_cache().await?;
    let token = state.github_token()?;
    let (http, repo) = (&state.http, cache.repo.clone());

    // Lectures : données fraîches (pas de cache servi hors ligne ici), plan, tag libre, SHA.
    {
        let _guard = cache.sync_lock.lock().await;
        sync::sync_milestones(http, &token, &cache).await?;
        sync::sync_issues(http, &token, &cache).await?;
    }
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let open = open_issues_of(&cache, &current).await?;
    let config = crate::commands::settings::load(&state).await?.config;
    let rules = CloseRules {
        sprint_days: i64::from(config.versioning.sprint_days),
        tag_prefix: config.project.tag_prefix.clone(),
    };
    let plan = version::plan_close(&versions, &current, &input, open, &version::today(), &rules)?;
    if writes::tag_exists(http, &token, &repo, &plan.tag).await? {
        return Err(AppError::Validation(format!("Le tag {} existe déjà sur GitHub.", plan.tag)));
    }
    let sha = writes::branch_sha(http, &token, &repo, &project.repo.default_branch).await?;

    // Écritures.
    let mut done: Vec<String> = Vec::new();
    let fail = |done: &[String], err: AppError| interrupted(done, err);

    let next_number = match plan.next_milestone {
        Some(number) => {
            if let Some(due) = &plan.next_due {
                writes::update_milestone(http, &token, &repo, number, &json!({ "due_on": due }))
                    .await
                    .map_err(|e| fail(&done, e))?;
                done.push(format!("milestone {} daté", plan.next));
            }
            number
        }
        None => {
            let (milestone, _) = writes::create_milestone(http, &token, &repo, &plan.next, plan.next_due.as_deref())
                .await
                .map_err(|e| fail(&done, e))?;
            done.push(format!("milestone {} créé", plan.next));
            milestone.number
        }
    };

    let patch = match input.carry_over {
        CarryOver::MoveToNext => json!({ "milestone": next_number }),
        CarryOver::Unversioned => json!({ "milestone": null }),
        CarryOver::CloseAsNotPlanned => json!({ "state": "closed", "state_reason": "not_planned" }),
    };
    let mut moved = 0u32;
    for number in &plan.carry {
        let updated = writes::update_issue(http, &token, &repo, *number, &patch).await.map_err(|e| {
            let mut so_far = done.clone();
            if moved > 0 {
                so_far.push(format!("{moved} issue(s) reportée(s)"));
            }
            fail(&so_far, e)
        })?;
        cache.upsert_issues(&[updated]).await?;
        moved += 1;
    }
    if moved > 0 {
        done.push(format!("{moved} issue(s) reportée(s)"));
    }

    writes::update_milestone(http, &token, &repo, plan.current_milestone, &json!({ "state": "closed" }))
        .await
        .map_err(|e| fail(&done, e))?;
    done.push(format!("milestone {} fermé", plan.current));

    writes::create_tag(http, &token, &repo, &plan.tag, &sha).await.map_err(|e| fail(&done, e))?;

    // La version est close : un échec des notes ne l'annule pas, il est seulement signalé.
    let release_url = if input.generate_notes {
        match writes::create_draft_release(http, &token, &repo, &plan.tag).await {
            Ok(url) => Some(url).filter(|u| !u.is_empty()),
            Err(err) => {
                tracing::warn!(%err, "notes de version non générées");
                None
            }
        }
    } else {
        None
    };

    cache.invalidate(SyncResource::Milestones).await?;
    cache.invalidate(SyncResource::Issues).await?;
    let mut project_slot = state.project.write().await;
    if let Some(p) = project_slot.as_mut() {
        p.current_version = Some(plan.next.clone());
        let saved = p.clone();
        state.settings.update(|s| s.last_project = Some(saved))?;
    }

    Ok(CloseReport {
        closed: plan.current,
        opened: plan.next,
        moved_issues: moved,
        tag: plan.tag,
        release_url,
        deploy_run: None,
    })
}

/// Numéros des issues encore ouvertes de la version `title`.
async fn open_issues_of(cache: &RepoCache, title: &str) -> AppResult<Vec<u32>> {
    Ok(cache
        .issues()
        .await?
        .into_iter()
        .filter(|i| i.state == "open")
        .filter(|i| {
            i.milestone.as_ref().is_some_and(|m| {
                version::parse(&m.title).map(|v| v.to_string()).as_deref() == Some(title)
            })
        })
        .map(|i| i.number)
        .collect())
}

/// Erreur d'une étape de clôture, avec ce qui a déjà été écrit sur GitHub.
fn interrupted(done: &[String], err: AppError) -> AppError {
    if done.is_empty() {
        return err;
    }
    AppError::GitHub(format!(
        "clôture interrompue ({err}). Déjà fait sur GitHub : {}. Reprenez sur GitHub ou relancez la clôture.",
        done.join(", ")
    ))
}
