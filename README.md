# Cadence

Cockpit de projet logiciel adossé à GitHub : un sprint est une version, une version est un milestone,
et les agents proposent pendant que vous disposez. Application de bureau Tauri 2.

- Maquettes : canvas « Cadence — Écrans de l'app » (19 écrans)
- Plan technique : doc « Cadence — Plan technique d'implémentation »
- **Documentation utilisateur et mainteneur : [`docs/`](docs/README.md)** (installation, premiers
  pas, agents, référence de `.cadence/config.toml`, dépannage, processus de release)

## Démarrer

Prérequis : Node 20+, pnpm, Rust stable (MSVC sous Windows), WebView2.

```bash
pnpm install
pnpm dev          # UI seule dans le navigateur, données mockées (http://localhost:1420)
pnpm tauri dev    # app de bureau, back Rust réel
pnpm tauri:mock   # app de bureau, back mocké (tant que les commandes Rust ne sont pas branchées)
```

| Commande | Rôle |
| --- | --- |
| `pnpm typecheck` | TypeScript strict |
| `pnpm test` | tests front (Vitest) |
| `pnpm test:rust` | tests Rust + régénération des types TS |
| `pnpm bindings` | régénère seulement `src/lib/bindings` depuis Rust (ts-rs) |
| `pnpm format` | Prettier + tri des classes Tailwind |
| `pnpm build` | build de prod du front |

### Connexion GitHub

**Aucune configuration n'est nécessaire** sur une machine où la CLI GitHub ou Git sont installés.
Méthodes, dans l'ordre proposé à l'écran :

| Méthode | Comment | Besoin |
| --- | --- | --- |
| Compte de la CLI GitHub | reprend la session de `gh` en un clic (`gh auth token`) | `gh` connecté |
| Navigateur | Git Credential Manager ouvre GitHub dans le navigateur et rend la main tout seul (il a sa propre application OAuth) | Git pour Windows / GCM |
| Navigateur, OAuth App de Cadence | flux web direct : redirection vers `127.0.0.1`, PKCE | client ID + client secret |
| Code d'appairage | device flow : un code à saisir sur github.com/login/device | client ID, device flow activé |
| Jeton personnel | jeton classique collé dans l'app | rien |

Dans tous les cas, le jeton est validé auprès de GitHub puis copié dans le trousseau de l'OS sous
l'entrée de Cadence ; il n'est jamais affiché ni renvoyé au webview. Seule la portée `repo` est
bloquante : l'écran signale les autres portées manquantes.

#### Optionnel : OAuth App propre à Cadence

Utile pour une distribution publique (l'écran d'autorisation GitHub affiche alors « Cadence »).
Créer l'**OAuth App** : github.com › Settings › Developer settings › OAuth Apps › New OAuth App.

1. **Authorization callback URL** : `http://127.0.0.1/callback` (GitHub accepte tout port sur cette
   adresse loopback ; Cadence en choisit un libre à chaque connexion) ;
2. cocher **Enable Device Flow** (pour le code d'appairage) ;
3. générer un client secret ;
4. exporter les deux valeurs avant `pnpm tauri dev` (lues à l'exécution, ou figées au build) :

```powershell
$env:CADENCE_GITHUB_CLIENT_ID="Ov23li..."
$env:CADENCE_GITHUB_CLIENT_SECRET="..."
```

Le client secret est compilé dans l'app : dans une app native il ne peut pas rester confidentiel
(RFC 8252, même choix que la CLI `gh`). PKCE et le paramètre `state` empêchent qu'un code
d'autorisation intercepté soit utilisable. Le jeton obtenu est rangé dans le trousseau de l'OS
(Keychain, Credential Manager, Secret Service), jamais dans le webview ni sur disque.

### Mode mock

En mode navigateur, l'URL pilote le scénario mocké :

- `?scenario=onboarding` : compte GitHub non connecté ; `?scenario=no-project` : aucun dépôt choisi (sélecteur ouvert) ;
- `&flow=denied|expired|config` : issue du device flow simulé (par défaut, autorisé après 6 s) ;
- `&cli=fail` : session de test de Claude Code en échec ;
- `&update=available|fail` : mise à jour de l'app simulée (Réglages › À propos) ;
- `?view=<id>` force un écran (`?view=design-system`, `?view=issue&number=156`, `?view=agent&kind=dev`).

La **galerie de composants** (`?view=design-system`, ou `Ctrl+K` → « Galerie de composants ») montre
toutes les briques du design system avec les données des maquettes.

## Architecture

```
cadence/
├─ src/                        # front React 19 + TypeScript
│  ├─ app/                     # racine : providers, démarrage, cadre des vues, modales globales
│  ├─ views/                   # un écran du canvas = une vue ; registry.tsx les déclare toutes
│  ├─ components/
│  │  ├─ icons/                # les 34 icônes des maquettes (Icon name="…")
│  │  ├─ ui/                   # primitives du design system (Button, Badge, Field, Dialog…)
│  │  ├─ domain/               # briques métier (IssueCard, AgentBadge, StepTimeline, DiffViewer…)
│  │  └─ layout/               # Sidebar, Page/TopBar, Rail, OnboardingLayout, CommandPalette
│  ├─ stores/                  # Zustand : navigation (vue courante), état d'interface
│  ├─ lib/
│  │  ├─ bindings/             # GÉNÉRÉ par ts-rs — ne pas éditer
│  │  ├─ types.ts              # point d'import unique des types partagés
│  │  ├─ ipc/                  # contrat des commandes, transport Tauri/mock, événements
│  │  ├─ query/                # TanStack Query : client, clés, hooks, pont événements → cache
│  │  └─ domain/               # vocabulaire métier → libellés/couleurs, formats FR, semver
│  └─ styles/globals.css       # tokens du design system (Tailwind v4 @theme)
└─ src-tauri/                  # cœur Rust
   ├─ src/
   │  ├─ commands/             # surface IPC, une commande par intention métier
   │  ├─ domain/               # types du contrat (exportés en TS)
   │  ├─ api/github/           # client GitHub, régimes de quota
   │  ├─ auth/                 # device flow, trousseau, détection des CLI d'agents
   │  ├─ agents/               # runner, cycle de vie, garde-fous
   │  ├─ store/  sync/  vcs/   # cache SQLite, moteur de sync, git/worktrees
   │  ├─ events.rs             # événements back → front
   │  └─ error.rs              # AppError → { kind, message }
   └─ migrations/              # schéma SQLite du cache
```

Principes (du plan technique) :

- Le webview n'a ni jeton ni accès à l'API GitHub : tout passe par une commande Rust.
- Une commande par intention métier ; le front ne compose jamais plusieurs appels pour une vue.
- Les vues lisent le cache via TanStack Query et se rafraîchissent sur `sync://updated`.
- Navigation par état (`useNavigation`), sans routeur d'URL.
- Les types du contrat sont écrits une fois en Rust et générés en TS : un changement casse à la compilation.

## Recettes

**Implémenter un écran** — créer `src/views/<zone>/<Nom>View.tsx` (export par défaut, prop `view`
typée), puis remplacer son `placeholder(...)` dans `src/views/registry.tsx` par
`lazy(() => import("./<zone>/<Nom>View"))`. Composer avec `Page` (en-tête + zone principale),
`components/domain` et `components/ui` ; lire les données avec les hooks de `lib/query/queries.ts`.

**Ajouter une commande** — 1. l'écrire dans `src-tauri/src/commands/<domaine>.rs` avec ses types dans
`domain/` (`#[ts(export)]`) ; 2. l'enregistrer dans `lib.rs` ; 3. `pnpm bindings` ; 4. la déclarer dans
`src/lib/ipc/commands.ts` et lui donner un handler dans `src/lib/ipc/mock/handlers.ts` (le
typage refuse un oubli) ; 5. exposer un hook dans `lib/query/queries.ts`.

**Ajouter une couleur ou une taille** — dans `src/styles/globals.css` (`@theme`) ; une nouvelle taille de
texte ou un nouveau rayon se déclare aussi dans `src/lib/cn.ts` pour que `cn()` fusionne correctement.

## État

- **Fondations** : design system complet, architecture front/back, contrat IPC typé (32 commandes,
  toutes mockées), navigation et palette de commandes.
- **Onboarding 1 — Connexion GitHub** : branché de bout en bout (compte de la CLI GitHub, navigateur via Git Credential Manager ou OAuth App, code d'appairage, trousseau,
  jeton personnel, changement de compte).
- **Onboarding 2 — Agents de code** : détection de Claude Code et Codex CLI (réglage manuel, `PATH`,
  emplacements usuels), session de test headless de Claude Code, localisation manuelle du binaire,
  désactivation, affectation des rôles. Les préférences de la machine sont dans
  `<config de l'app>/settings.json`.
- **Sélecteur de dépôt** (haut de la barre latérale, ou `Ctrl+K` → « Changer de dépôt ») : Cadence
  fonctionne sur n'importe quel dépôt accessible au compte (une requête GraphQL, filtrée localement),
  et permet d'en créer un (compte personnel ou organisation, privé ou public, README initial).
  Ouvrir un dépôt importe ses milestones comme versions ; le choix est mémorisé et rouvert au
  lancement suivant. Sans dépôt mémorisé, le sélecteur s'ouvre de lui-même. `versions_list` est branché.
- **Issues** : toutes les issues du dépôt ouvert (pull requests exclues), groupées par version avec
  leur avancement ; filtres texte / version / état / assigné, sélection multiple. Les actions
  groupées et l'agent CTO arrivent avec l'écriture (0.1.2+).
- **Cache SQLite** (`sqlx`) : une base par dépôt, `<données locales de l'app>/cache/<owner>/<nom>.sqlite3`,
  migrée à l'ouverture (`PRAGMA user_version`). Issues et milestones y sont rangés avec leur JSON
  brut ; une passe au plus par minute, incrémentale (`since` = dernier `updated_at` vu) et
  conditionnelle (ETag : un dépôt calme répond `304` sans consommer de quota). L'app sert le cache
  hors ligne et au lancement. La déconnexion supprime toutes les bases.
- **Pull requests** : PR ouvertes et 30 dernières fusionnées ou fermées, triées par activité (une
  requête GraphQL par minute au plus : CI du dernier commit, approbations, changements demandés,
  attente de revue, issue fermée, version). Filtres version / état / auteur / origine (agent si
  label `agent/*`). Une ligne ouvre l'écran de revue.
- **Revues à faire** : PR dont vous êtes relecteur, et vos PR qui attendent une revue ; pour la PR
  choisie, diff fichier par fichier (patchs lus en REST), description, impact sur la version. Les
  boutons de revue arrivent avec l'écriture (0.1.2), le résumé de l'agent CTO en 0.1.3.
- **Roadmap** : une carte par version, livrées et en cours en haut, sprints planifiés et paliers en
  bas, avec avancement et trois issues (ouvertes d'abord). Convention : la première ligne de la
  description du milestone nomme la version (`# Cockpit Tauri`), ses éléments de liste sont les
  thèmes d'un palier ; un sprint commence à l'échéance du précédent.
- **Nouvelle issue** (bouton des écrans, `Ctrl+K`) : titre et version obligatoires (milestone ouvert),
  type posé en label `type/*`, labels libres, assignation à soi. Créée sur GitHub puis rangée dans
  le cache. L'envoi à l'agent CTO arrive en 0.1.3.
- **Clôture de version** (roadmap, `Ctrl+K`) : version suivante (sprint, palier, majeure ou saisie),
  sort des issues ouvertes (reporter, retirer, fermer comme non retenues), aperçu exact des
  écritures. Tout est vérifié avant la première écriture (version semver plus grande, tag libre,
  branche lisible), puis : milestone suivant créé ou daté (sprint de 14 jours), issues traitées,
  milestone fermé, tag léger posé sur la branche par défaut, brouillon de release avec notes
  générées (jamais publié automatiquement). Si une étape échoue, l'erreur dit ce qui a déjà été
  écrit.
- **Réglages** : « Projet et mapping » et « Agents » éditent `.cadence/config.toml`, lu et écrit
  via l'API GitHub (un commit sur la branche par défaut, refusé si le fichier a changé entre-temps).
  Fichier en `snake_case`, complété par les valeurs par défaut s'il est partiel ; sans fichier, les
  moteurs d'agents reprennent le choix de l'onboarding. La clôture de version en tire la durée de
  sprint, le préfixe de tag et la confirmation avant un palier. « Comptes et jetons » (compte,
  portées, déconnexion) et « Machine locale » (agents de code détectés) montrent l'état de la machine.
- **Agent CTO** (barre latérale › CTO, ou « Agent CTO » dans Issues) : parcourt les issues ouvertes de
  la version qui ne sont pas encore passées par lui. Une session Claude Code headless par issue
  (`claude -p --output-format stream-json`, prompt sur l'entrée standard), outils restreints à
  `Read,Grep,Glob` dans une copie superficielle du dépôt (`<données>/repos/<owner>/<nom>`, jeton
  passé à Git par variables d'environnement), durée plafonnée par `max_session_minutes`. Il rend
  une description (modèle du projet), une estimation, des labels, un risque, des issues liées, ou
  des questions : vos réponses reprennent la même session (`--resume`). Diff avant/après, édition,
  « Passer », « Appliquer » : seule l'application écrit sur GitHub (description, labels
  `agent/cto` et `estimate/*`, commentaire signé). Autonomie lue dans `config.toml` (`propose`
  bloque toute écriture, `auto` applique seul ce qui n'a pas de question). Journal et file gardés
  dans `agent_runs` (reprise après redémarrage), console en direct par `agent://event`.
- **Détail d'une issue** (clic sur une ligne d'Issues) : description rendue (markdown, cases en
  lecture seule), état de l'agent CTO (en file, en cours, proposition à relire, questions, validée)
  avec « Envoyer à l'agent CTO », questions avec réponse renvoyée dans sa session, propriétés
  (version, labels, assigné, branche et PR liée, estimation). Une issue créée avec « Envoyer à
  l'agent CTO » rejoint la file en cours, ou en lance une.
- **Agent développeur** (barre latérale › Développeur) : on coche les issues de la version, elles
  passent l'une après l'autre en six étapes. (1) Analyse dans un worktree isolé
  (`<worktree_root>/<owner>/<nom>/<issue>`, branche `<type>/<num>-<slug>`) ; (2) commit d'ouverture
  vide poussé par Cadence et PR en brouillon (label `agent/dev`) ; (3) implémentation et (4) tests
  dans la même session Claude Code, avec outils d'écriture et commandes de build/test seulement ;
  puis commit par Cadence (identité GitHub noreply de l'utilisateur). (5) Le push attend votre
  validation (sauf autonomie `auto`) ; (6) la PR sort du brouillon avec un commentaire signé. En
  `propose`, rien ne quitte la machine. Garde-fous : `git push`, `git remote`, `git config`, `gh`,
  `curl` interdits à l'agent, aucun assistant d'identifiants dans son environnement, `pushurl`
  invalide sur la copie, et Cadence refuse de pousser sur la branche par défaut. Pause (après
  l'étape en cours), arrêt, et intervention depuis le terminal (reprise à l'étape suivante).
- **Distribution (1.0.0)** : mise à jour automatique (`tauri-plugin-updater`, manifeste
  `latest.json` des releases GitHub, paquets signés minisign) avec vérification silencieuse au
  lancement et Réglages › **À propos** (version, recherche, téléchargement avec progression,
  redémarrage). Workflows `.github/workflows/ci.yml` (typecheck, tests, build, clippy et tests
  Rust sur les trois OS) et `release.yml` (tag `v*` → brouillon de release, macOS universel signé
  et notarisé, Windows signé par Azure Trusted Signing ou certificat, Linux). Avant la première
  release : clé publique de l'updater à coller dans `tauri.conf.json` et secrets, voir
  [docs/release.md](docs/release.md). Build local sans clé : `pnpm tauri:build:unsigned`.
- **Console des agents** (barre latérale › Console agents) : une carte par agent (statut, autonomie,
  actions en attente, exécution en cours avec issue, étape et minuteur, dernier passage) et
  « Ouvrir » vers son écran ; journal des exécutions tous agents confondus (`agent_runs_list`,
  table `agent_runs` du cache, exécutions ouvertes lues en mémoire), « Journal complet » ;
  consommation sur 7 jours (part du temps d'exécution par agent) ; « Tout suspendre » / « Tout
  reprendre » (pause après l'étape ou l'issue en cours, agents CTO et développeur). Garde-fous :
  « Validation avant écriture GitHub » réécrit l'autonomie dans `.cadence/config.toml` (même
  chemin que Réglages › Agents ; l'agent CEO reste en « propose seulement ») ; push sur la branche
  par défaut et accès aux autres dépôts restent interdits, fusion automatique pas encore
  disponible. La barre latérale suit `agents_overview` (rafraîchi par `agent://state`) : pastille
  repos / attente / en cours par agent, carte « Agent … en cours » pour chaque exécution active.
- **Sprint en cours** (écran d'accueil) et **détail d'une version** (barre latérale › Versions, ou
  une carte de la roadmap) : `version_detail(title?)` assemble depuis le cache la version, ses
  issues reliées à leur PR (ouverte d'abord ; CI en échec signalée), ses PR (milestone ou issue
  fermée), ses voisines et son tag si elle est livrée ; `version_current` en renvoie le rythme.
  Burndown jour par jour du début du sprint (échéance du précédent) à aujourd'hui, d'après la
  date de fermeture des issues (portée actuelle du milestone) ; vélocité = issues fermées dans le
  sprint par jour écoulé ; retard = écart en jours avec la droite idéale. Sprint : KPI, tableau
  par état (une carte ouvre l'issue), activité des agents, points à traiter (questions du CTO,
  revues de plus d'un jour). Version : portée, progression par état, aperçu des notes de version
  (issues fermées par type), conditions de clôture, PR de la version ; clôture de la version en
  cours. Les PR comptées sont celles du cache (ouvertes et 30 dernières fusionnées ou fermées).
- **Agent déploiement** (barre latérale › Déploiement, ou « Lancer l'agent déploiement » en fin de
  clôture) : sur une version close (son tag `<préfixe><version>`), cinq étapes. (1) Pré-vol : tag et
  brouillon de release présents, issues closes de la version (hors « non retenues ») et leurs
  critères d'acceptation, lus dans le cache ; (2) Build : worktree détaché sur le tag
  (`<worktree_root>/<owner>/<nom>/deploy-<tag>`), session Claude Code qui construit selon les
  conventions du dépôt et liste ses artefacts ; (3) Tests de vérification : un test par issue,
  dérivé des cases `- [ ]` de sa section « Critères d'acceptation » et rejoué dans la même session
  (réussi, échec, non vérifiable + constat) ; (4) Documentation : notes de version par type
  d'issue, sans les issues en échec ; (5) Tag et release : « Publier la release » met à jour le
  brouillon avec les notes et le publie (ou crée la release), « Rouvrir sur <suivante> » rouvre
  l'issue d'un test en échec sur le milestone suivant, « Ignorer » la garde dans les notes. Rien
  n'est publié sans validation (même en `auto`), jamais en file hors-ligne ; aucun push. Notes
  éditables avant publication (`agent_edit_notes`), « Rejouer les tests » relance sur le même tag.
- **À venir** : l'agent CEO (0.2.0), le mode hors-ligne complet (1.0.0).
