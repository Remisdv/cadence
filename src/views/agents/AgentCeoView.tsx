/**
 * Agent CEO (maquette Agent-CEO.dc.html) : contexte utilisé à gauche (palier visé, thèmes,
 * historique lu), propositions d'issues au centre avec leur justification, console de la
 * session à droite pendant qu'elle tourne (ou sur demande).
 *
 * Verrouillé en « propose » : rien n'est créé sur GitHub sans « Créer les … issues ».
 */
import { useState } from "react";
import type { AgentRunDetail, CeoProposal, Version } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon, type IconName } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { FooterBar } from "@/components/domain/Panels";
import { SessionConsole } from "@/components/domain/SessionConsole";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Input, Textarea } from "@/components/ui/Field";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { ceoTarget, isDefaultPick, milestonesLabel, plural, themeCoverage } from "@/lib/domain/ceo";
import { formatAgo, formatDay, formatDuration } from "@/lib/domain/format";
import { useElapsedSeconds } from "@/lib/hooks/useElapsedSeconds";
import {
  useAgentLatestRun,
  useApplyActions,
  useCurrentProject,
  useEditCeoProposal,
  useIssues,
  useRejectItems,
  useStartAgent,
  useStopAgent,
  useVersions,
} from "@/lib/query/queries";
import { openExternal } from "@/lib/platform";
import { useRunLog } from "./RunConsole";

export default function AgentCeoView() {
  const project = useCurrentProject().data;
  const versions = useVersions().data ?? [];
  const latest = useAgentLatestRun("ceo");
  const start = useStartAgent();
  const stop = useStopAgent();
  const run = latest.data ?? null;
  const active = run !== null && (run.state === "prepare" || run.state === "running");
  const target = run?.version ?? ceoTarget(versions, project?.currentVersion ?? null);

  const launch = () => start.mutate({ kind: "ceo", input: target ? { version: target } : {} });
  const applied = run?.proposals.filter((p) => p.state === "applied").length ?? 0;
  const selection = useSelection(run);
  const { picked } = selection;

  const subtitle = !run
    ? "dérive les issues qui manquent à partir des thèmes de la roadmap"
    : active
      ? `session en cours · ${plural(run.proposals.length, "issue proposée", "issues proposées")}`
      : [
          plural(
            run.proposals.length,
            "issue dérivée de la roadmap",
            "issues dérivées de la roadmap",
          ),
          applied > 0
            ? `${applied} créée${applied > 1 ? "s" : ""} sur GitHub`
            : "aucune n'existe encore sur GitHub",
        ].join(" · ");

  return (
    <Page
      title={run?.version ? `Agent CEO — propositions pour ${run.version}` : "Agent CEO"}
      subtitle={subtitle}
      padding={run ? "none" : "default"}
      actions={
        active ? (
          <Button icon="stop" disabled={stop.isPending} onClick={() => stop.mutate(run.run)}>
            Arrêter
          </Button>
        ) : (
          <>
            <Button
              variant={run ? "secondary" : "primary"}
              icon={run ? "refresh" : "play"}
              disabled={start.isPending || !target}
              onClick={launch}
              title={
                target
                  ? `Dériver les issues de ${target}`
                  : "Aucun palier ouvert après la version en cours"
              }
            >
              {start.isPending && <Spinner />}
              {run ? "Relancer" : `Lancer sur ${target ?? "…"}`}
            </Button>
            {run && (
              <Button
                variant="primary"
                icon="plus"
                disabled={picked.length === 0 || selection.apply.isPending}
                onClick={selection.create}
              >
                Créer {picked.length > 1 ? `les ${picked.length} sélectionnées` : "la sélection"}
              </Button>
            )}
          </>
        )
      }
    >
      {latest.isPending ? (
        <LoadingState label="Lecture du journal de l'agent…" />
      ) : latest.isError ? (
        <ErrorState error={latest.error} />
      ) : !run ? (
        <div className="mx-auto flex max-w-[620px] flex-col gap-4 pt-10">
          <EmptyState icon="sparkle" title="L'agent CEO n'a pas encore tourné sur ce dépôt">
            Il lit les thèmes du prochain palier (la liste de la description du milestone), les
            versions livrées, les issues existantes et la documentation, puis propose les issues qui
            manquent, chacune avec sa justification.
          </EmptyState>
          {!target && (
            <Callout tone="warning">
              Aucun palier ouvert après la version en cours : créez un milestone sans date dont la
              description liste les thèmes (« - Rôles et permissions »).
            </Callout>
          )}
          {start.isError && <Callout tone="danger">{start.error.message}</Callout>}
          <Guardrails />
        </div>
      ) : (
        <RunScreen
          key={run.run}
          run={run}
          versions={versions}
          active={active}
          selection={selection}
          startError={start.isError ? start.error.message : null}
        />
      )}
    </Page>
  );
}

function Guardrails() {
  const rows: [IconName, string][] = [
    ["eye", "Lecture seule : il lit une copie du dépôt, sans rien modifier ni exécuter."],
    ["check", "Toujours en « propose » : aucune issue n'est créée sans votre clic."],
    ["tag", "Chaque issue créée porte le label agent/ceo et un pied signé avec son thème."],
    ["clock", "La session est plafonnée (30 min par défaut, Réglages › Agents)."],
  ];
  return (
    <div className="flex flex-col gap-2 rounded-card border border-line bg-panel px-4 py-3.5">
      <SectionLabel>Garde-fous</SectionLabel>
      {rows.map(([icon, text]) => (
        <div key={text} className="flex items-start gap-2.5 text-body-sm text-tx2">
          <span className="mt-0.5 flex text-tx3">
            <Icon name={icon} size={14} />
          </span>
          {text}
        </div>
      ))}
    </div>
  );
}

/* ---------- Sélection ---------- */

const isActionable = (p: CeoProposal) => p.state === "ready" || p.state === "failed";

/**
 * Propositions cochées : vos choix explicites, sinon la règle par défaut (`isDefaultPick`).
 * Partagée entre l'en-tête (« Créer les N sélectionnées ») et le pied de l'écran.
 */
function useSelection(run: AgentRunDetail | null) {
  const apply = useApplyActions(run?.run ?? "");
  const [state, setState] = useState<{ run: string | null; picks: Record<number, boolean> }>({
    run: null,
    picks: {},
  });
  const picks = state.run === run?.run ? state.picks : {};
  const update = (next: Record<number, boolean>) =>
    setState({ run: run?.run ?? null, picks: next });

  const actionable = run?.proposals.filter(isActionable) ?? [];
  const isPicked = (p: CeoProposal) =>
    isActionable(p) && (picks[p.index] ?? isDefaultPick(p, run?.version ?? null));
  const picked = actionable.filter(isPicked);
  const allPicked = actionable.length > 0 && picked.length === actionable.length;
  return {
    apply,
    actionable,
    picked,
    allPicked,
    isPicked,
    toggle: (p: CeoProposal) => update({ ...picks, [p.index]: !isPicked(p) }),
    toggleAll: () => update(Object.fromEntries(actionable.map((p) => [p.index, !allPicked]))),
    create: () =>
      apply.mutate(
        picked.map((p) => `propose-${p.index}`),
        { onSuccess: () => update({}) },
      ),
  };
}

type Selection = ReturnType<typeof useSelection>;

/* ---------- Exécution ---------- */

function RunScreen({
  run,
  versions,
  active,
  selection,
  startError,
}: {
  run: AgentRunDetail;
  versions: Version[];
  active: boolean;
  selection: Selection;
  startError: string | null;
}) {
  const project = useCurrentProject().data;
  const reject = useRejectItems(run.run);
  const [showLog, setShowLog] = useState(false);
  const { apply, actionable, picked, allPicked, isPicked, toggle, toggleAll, create } = selection;
  const failures = apply.data?.failed ?? [];
  const error = apply.error ?? reject.error;

  return (
    <div className="flex min-w-0 grow">
      <ContextPanel run={run} versions={versions} />

      <section className="flex min-w-0 grow flex-col overflow-hidden">
        <div className="flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-5">
          <label className="flex items-center gap-2 text-body-sm text-tx2">
            <input
              type="checkbox"
              checked={allPicked}
              disabled={actionable.length === 0}
              onChange={toggleAll}
              className="size-3.5 accent-purple"
            />
            Tout sélectionner
          </label>
          <Badge tone="purple">
            {picked.length} sur {actionable.length} sélectionnée{picked.length > 1 ? "s" : ""}
          </Badge>
          <div className="grow" />
          {!active && (
            <Button variant="quiet" size="xs" icon="bot" onClick={() => setShowLog((v) => !v)}>
              {showLog ? "Masquer le journal" : "Journal"}
            </Button>
          )}
          <span className="text-caption text-tx3">
            {active ? "Génération en cours…" : `Généré ${formatAgo(run.endedAt ?? run.startedAt)}`}
          </span>
        </div>

        <div className="flex min-h-0 grow flex-col gap-2.5 overflow-y-auto px-5 py-4">
          {startError && <Callout tone="danger">{startError}</Callout>}
          {run.error && <Callout tone="danger">{run.error}</Callout>}
          {error && <Callout tone="danger">{error.message}</Callout>}
          {failures.length > 0 && (
            <Callout tone="danger">
              {failures.length} issue{failures.length > 1 ? "s" : ""} non créée
              {failures.length > 1 ? "s" : ""} : {failures[0]!.message}
            </Callout>
          )}
          {run.proposals.length === 0 ? (
            active ? (
              <LoadingState label="L'agent lit la roadmap, les issues et la documentation…" />
            ) : run.state === "failed" ? null : (
              <EmptyState icon="check" title="Rien ne manque">
                Chaque thème du palier est déjà couvert par une issue existante.
              </EmptyState>
            )
          ) : (
            run.proposals.map((p) => (
              <ProposalCard
                key={p.index}
                run={run}
                proposal={p}
                target={run.version}
                picked={isPicked(p)}
                onToggle={() => toggle(p)}
                onReject={() => reject.mutate([p.index])}
              />
            ))
          )}
          {active && run.proposals.length > 0 && (
            <div className="flex items-center gap-2 px-1 text-caption text-tx3">
              <Spinner /> D'autres propositions peuvent arriver…
            </div>
          )}
        </div>

        <FooterBar
          hint={
            picked.length === 0 ? (
              "Cochez les propositions à créer sur GitHub."
            ) : (
              <>
                {plural(picked.length, "issue sera créée", "issues seront créées")} sur{" "}
                <span className="mono">{project?.repo.fullName ?? "le dépôt"}</span> avec{" "}
                {milestonesLabel(picked)} et le label <span className="mono">agent/ceo</span>.
              </>
            )
          }
        >
          <Button
            variant="ghost"
            disabled={actionable.length === 0 || reject.isPending || active}
            onClick={() => reject.mutate(actionable.map((p) => p.index))}
          >
            Tout rejeter
          </Button>
          <Button
            variant="primary"
            icon="plus"
            disabled={picked.length === 0 || apply.isPending}
            onClick={create}
          >
            {apply.isPending && <Spinner />}
            Créer {picked.length > 1 ? `les ${picked.length} issues` : "l'issue"}
          </Button>
        </FooterBar>
      </section>

      {(active || showLog) && <CeoConsole run={run} active={active} />}
    </div>
  );
}

function ContextPanel({ run, versions }: { run: AgentRunDetail; versions: Version[] }) {
  const issues = useIssues().data ?? [];
  const target = versions.find((v) => v.title === run.version);
  const themes = themeCoverage(target, run.proposals);
  const shipped = versions.filter((v) => v.state === "closed");
  const closed = issues.filter((i) => i.status === "done").length;
  const attached = target ? target.openIssues + target.closedIssues : 0;
  const kind = !target
    ? null
    : target.kind === "palier"
      ? "Palier · sans date"
      : `Sprint · échéance ${target.dueOn ? formatDay(target.dueOn) : "non fixée"}`;

  return (
    <aside className="flex w-[292px] shrink-0 flex-col gap-3.5 overflow-y-auto border-r border-line bg-panel px-3.5 py-4">
      <div className="flex items-center gap-[9px]">
        <AgentBadge agent="ceo" size={26} />
        <span className="text-body font-semibold">Contexte utilisé</span>
      </div>

      <div>
        <SectionLabel>Version cible</SectionLabel>
        <div className="mt-2 rounded-field border border-line bg-p2 px-3 py-[11px]">
          <div className="flex items-center gap-2">
            {run.version && <VersionBadge version={run.version} tone="blue" />}
            <span className="truncate text-body-sm">
              {target?.name ?? (run.version ? `Version ${run.version}` : "—")}
            </span>
          </div>
          {kind && (
            <div className="mt-1.5 text-tiny leading-[1.55] text-tx3">
              {kind} ·{" "}
              {attached === 0
                ? "aucune issue rattachée"
                : plural(attached, "issue déjà rattachée", "issues déjà rattachées")}
            </div>
          )}
        </div>
      </div>

      <div>
        <SectionLabel>Thèmes de la roadmap</SectionLabel>
        <div className="mt-2 flex flex-col gap-1.5">
          {themes.length === 0 && (
            <p className="text-caption text-tx3">Aucun thème listé dans le milestone.</p>
          )}
          {themes.map(({ theme, covered }) => (
            <div
              key={theme}
              title={
                covered ? "Au moins une proposition en découle" : "Aucune proposition pour ce thème"
              }
              className={cn(
                "flex items-center gap-2 rounded-control border border-line bg-p2 px-2.5 py-[7px] text-small",
                covered ? "text-tx2" : "text-tx3",
              )}
            >
              <span className={cn("flex", covered ? "text-purple" : "text-tx3")}>
                <Icon name={covered ? "check" : "clock"} size={12} />
              </span>
              <span className="min-w-0 truncate">{theme}</span>
            </div>
          ))}
        </div>
      </div>

      <div>
        <SectionLabel>Aussi lu</SectionLabel>
        <div className="mt-2 text-caption leading-[1.8] text-tx3">
          <div>
            · {plural(shipped.length, "version livrée", "versions livrées")}
            {shipped.length > 0 && ` (${shipped.map((v) => v.title).join(", ")})`}
          </div>
          <div>· {plural(closed, "issue fermée", "issues fermées")}</div>
          <div>
            · <span className="mono">README.md</span> et <span className="mono">docs/</span>
          </div>
        </div>
      </div>

      <div className="mt-auto rounded-field border border-purple-line bg-purple-d px-3 py-[11px] text-caption leading-[1.6] text-tx2">
        Cet agent ne crée jamais d'issue directement. Il propose, vous tranchez.
      </div>
    </aside>
  );
}

function ProposalCard({
  run,
  proposal: p,
  target,
  picked,
  onToggle,
  onReject,
}: {
  run: AgentRunDetail;
  proposal: CeoProposal;
  target: string | null;
  picked: boolean;
  onToggle: () => void;
  onReject: () => void;
}) {
  const edit = useEditCeoProposal(run.run);
  const [draft, setDraft] = useState<{ title: string; body: string } | null>(null);
  const actionable = p.state === "ready" || p.state === "failed";
  const why = [p.theme && `Découle du thème « ${p.theme} ».`, p.rationale, p.overlap]
    .filter(Boolean)
    .join(" ");

  const save = () =>
    draft &&
    edit.mutate(
      { index: p.index, title: draft.title, body: draft.body },
      { onSuccess: () => setDraft(null) },
    );

  return (
    <div
      className={cn(
        "shrink-0 rounded-[11px] border px-[15px] py-3.5",
        picked ? "border-purple-line bg-p2" : "border-line bg-panel",
        (p.state === "skipped" || p.state === "applied") && "opacity-70",
      )}
    >
      <div className="flex items-start gap-[11px]">
        {p.state === "applied" ? (
          <span className="mt-[3px] flex text-green">
            <Icon name="check" size={14} />
          </span>
        ) : (
          <input
            type="checkbox"
            checked={picked}
            disabled={!actionable || p.state === "queued"}
            onChange={onToggle}
            aria-label={p.title}
            className="mt-[3px] size-3.5 accent-purple"
          />
        )}
        <div className="min-w-0 grow">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                "min-w-0 truncate text-body font-semibold",
                p.state === "skipped" && "line-through",
              )}
            >
              {p.title}
            </span>
            <div className="grow" />
            {p.state === "skipped" && <Badge tone="neutral">rejetée</Badge>}
            {p.state === "queued" && (
              <Badge tone="purple">
                <Spinner /> création…
              </Badge>
            )}
            {p.state === "applied" && p.issue !== null && (
              <Badge tone="green">#{p.issue} créée</Badge>
            )}
            <VersionBadge version={p.milestone} tone={p.milestone === target ? "blue" : "muted"} />
            {p.issueType && <TypeBadge type={p.issueType} />}
          </div>

          <div className="mt-[7px] flex gap-2 text-small leading-[1.6] text-tx2">
            <span className="mt-0.5 flex shrink-0 text-purple">
              <Icon name="link" size={13} />
            </span>
            <span>{why || "Sans justification."}</span>
          </div>

          {p.labels.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {p.labels.map((l) => (
                <Badge key={l} tone="muted" mono>
                  {l}
                </Badge>
              ))}
            </div>
          )}

          {p.state === "failed" && p.error && (
            <Callout tone="danger" className="mt-2.5">
              {p.error}
            </Callout>
          )}

          {draft && (
            <div className="mt-2.5 flex flex-col gap-2">
              <Input
                size="sm"
                value={draft.title}
                onChange={(e) => setDraft({ ...draft, title: e.target.value })}
                aria-label="Titre de l'issue"
              />
              <Textarea
                rows={10}
                value={draft.body}
                onChange={(e) => setDraft({ ...draft, body: e.target.value })}
                className="mono text-small"
                aria-label="Description de l'issue"
              />
              {edit.isError && <Callout tone="danger">{edit.error.message}</Callout>}
            </div>
          )}

          {actionable && (
            <div className="mt-2.5 flex gap-[7px]">
              {draft ? (
                <>
                  <Button variant="ghost" size="sm" onClick={() => setDraft(null)}>
                    Annuler
                  </Button>
                  <Button
                    size="sm"
                    icon="check"
                    disabled={edit.isPending || !draft.title.trim() || !draft.body.trim()}
                    onClick={save}
                  >
                    Enregistrer
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    icon="file"
                    onClick={() => setDraft({ title: p.title, body: p.body })}
                  >
                    Modifier
                  </Button>
                  <Button variant="ghost" size="sm" icon="close" onClick={onReject}>
                    Rejeter
                  </Button>
                </>
              )}
            </div>
          )}
          {p.state === "applied" && p.issueUrl && (
            <div className="mt-2.5">
              <Button
                variant="quiet"
                size="xs"
                icon="github"
                onClick={() => openExternal(p.issueUrl!)}
              >
                Ouvrir sur GitHub
              </Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function CeoConsole({ run, active }: { run: AgentRunDetail; active: boolean }) {
  const lines = useRunLog(run);
  const elapsed = useElapsedSeconds(active ? run.startedAt : null);
  return (
    <SessionConsole
      className="w-[340px] shrink-0 border-l border-line"
      lines={lines}
      header={
        <>
          <StatusDot tone={active ? "green" : "neutral"} pulse={active} />
          <span className="grow text-small font-semibold">Session Claude Code</span>
          <span className="mono text-tiny text-tx3">
            {active ? formatDuration(elapsed) : run.endedAt ? formatAgo(run.endedAt) : ""}
          </span>
        </>
      }
      footer={
        <span className="mono text-tiny text-tx3">
          {run.model ?? "modèle par défaut"} · lecture seule
        </span>
      }
    />
  );
}
