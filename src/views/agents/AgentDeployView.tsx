/**
 * Agent déploiement (maquette Agent-Deploy.dc.html) : pipeline en cinq étapes en haut, tests
 * d'acceptation à gauche (un par issue de la version, avec la décision à prendre sur un
 * échec), notes de version et artefacts à droite.
 *
 * Rien n'est publié sans vous : « Publier la release » et « Rouvrir sur … » passent par
 * `agent_apply`, jamais automatiquement.
 */
import { useState } from "react";
import type { AgentRunDetail, DeployState, DeployTest } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { TitledCard } from "@/components/domain/Panels";
import { SessionConsole } from "@/components/domain/SessionConsole";
import { PipelineStages } from "@/components/domain/Steps";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { Select, Textarea } from "@/components/ui/Field";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import {
  deploySubtitle,
  formatSize,
  formatTestDuration,
  inNotes,
  needsDecision,
  noteLines,
  pipelineStages,
  testCounts,
} from "@/lib/domain/deploy";
import { openExternal, revealPath } from "@/lib/platform";
import {
  useAgentLatestRun,
  useApplyActions,
  useControlAgent,
  useEditReleaseNotes,
  useRejectItems,
  useStartAgent,
  useStopAgent,
  useVersions,
} from "@/lib/query/queries";
import { useRunLog } from "./RunConsole";

export default function AgentDeployView() {
  const latest = useAgentLatestRun("deploy");
  const run = latest.data ?? null;
  const deploy = run?.deploy ?? null;
  const active = run !== null && (run.state === "prepare" || run.state === "running");
  const start = useStartAgent();
  const control = useControlAgent(run?.run ?? "");
  const stop = useStopAgent();
  const apply = useApplyActions(run?.run ?? "");
  const canPublish = Boolean(
    deploy && deploy.step >= 5 && !deploy.published && deploy.notes !== null,
  );

  return (
    <Page
      title={deploy ? `Agent déploiement — ${deploy.tag}` : "Agent déploiement"}
      subtitle={
        deploy ? deploySubtitle(deploy) : "Livrer une version close et vérifier ce qu'elle promet"
      }
      padding="none"
      actions={
        run && (
          <>
            {active ? (
              <>
                <Button
                  icon={run.paused ? "play" : "pause"}
                  disabled={control.isPending}
                  onClick={() => control.mutate({ cmd: run.paused ? "resume" : "pause" })}
                >
                  {run.paused ? "Reprendre" : "Pause"}
                </Button>
                <Button icon="stop" disabled={stop.isPending} onClick={() => stop.mutate(run.run)}>
                  Arrêter
                </Button>
              </>
            ) : (
              <Button
                icon={start.isPending ? undefined : "refresh"}
                disabled={start.isPending || !run.version}
                title="Relance le build et les tests de vérification sur le même tag"
                onClick={() =>
                  start.mutate({ kind: "deploy", input: { version: run.version ?? undefined } })
                }
              >
                {start.isPending && <Spinner />}
                Rejouer les tests
              </Button>
            )}
            <Button
              variant="primary"
              icon={apply.isPending ? undefined : "rocket"}
              disabled={!canPublish || run.autonomy === "propose" || apply.isPending}
              title={
                run.autonomy === "propose"
                  ? "Propose seulement : rien n'est publié"
                  : "Met à jour le brouillon avec ces notes et publie la release"
              }
              onClick={() => apply.mutate(["publish-release"])}
            >
              {apply.isPending && <Spinner />}
              {deploy?.published ? "Release publiée" : "Publier la release"}
            </Button>
          </>
        )
      }
    >
      {latest.isPending ? (
        <LoadingState label="Lecture du journal de l'agent…" />
      ) : latest.isError ? (
        <ErrorState error={latest.error} />
      ) : run && deploy ? (
        <DeployRun
          run={run}
          deploy={deploy}
          error={
            start.error?.message ?? apply.data?.failed[0]?.message ?? apply.error?.message ?? null
          }
        />
      ) : (
        <StartPanel />
      )}
    </Page>
  );
}

/* ---------- Lancement ---------- */

function StartPanel() {
  const versions = useVersions().data ?? [];
  const start = useStartAgent();
  const closed = versions.filter((v) => v.state === "closed").reverse();
  const [picked, setPicked] = useState<string | null>(null);
  const version = picked ?? closed[0]?.title ?? null;

  return (
    <div className="flex grow items-center justify-center">
      <EmptyState
        icon="rocket"
        title="Aucun déploiement sur ce dépôt"
        action={
          <div className="flex flex-col items-center gap-3">
            <div className="flex items-center gap-2">
              {closed.length > 1 && (
                <Select
                  aria-label="Version à déployer"
                  value={version ?? ""}
                  onChange={(e) => setPicked(e.target.value)}
                  className="mono"
                >
                  {closed.map((v) => (
                    <option key={v.title} value={v.title}>
                      {v.title}
                    </option>
                  ))}
                </Select>
              )}
              <Button
                variant="primary"
                icon={start.isPending ? undefined : "play"}
                disabled={!version || start.isPending}
                onClick={() => version && start.mutate({ kind: "deploy", input: { version } })}
              >
                {start.isPending && <Spinner />}
                {version ? `Déployer ${version}` : "Aucune version close"}
              </Button>
            </div>
            {start.isError && (
              <Callout tone="danger" className="text-left">
                {start.error.message}
              </Callout>
            )}
          </div>
        }
      >
        L'agent construit le tag d'une version close, rejoue un test d'acceptation par issue (dérivé
        de ses critères d'acceptation), rédige les notes de version, puis attend votre validation
        pour publier la release.
      </EmptyState>
    </div>
  );
}

/* ---------- Exécution ---------- */

function DeployRun({
  run,
  deploy,
  error,
}: {
  run: AgentRunDetail;
  deploy: DeployState;
  error: string | null;
}) {
  return (
    <div className="flex min-w-0 grow flex-col gap-[13px] overflow-hidden px-5 py-[18px]">
      <PipelineStages stages={pipelineStages(deploy)} />
      {(run.error ?? error) && <Callout tone="danger">{run.error ?? error}</Callout>}
      <div className="flex min-h-0 grow gap-[13px]">
        <TestsCard run={run} deploy={deploy} />
        <div className="flex w-[430px] shrink-0 flex-col gap-[13px]">
          <NotesCard run={run} deploy={deploy} />
          <ArtifactsCard deploy={deploy} />
        </div>
      </div>
    </div>
  );
}

function TestsCard({ run, deploy }: { run: AgentRunDetail; deploy: DeployState }) {
  const counts = testCounts(deploy.tests);
  const pending = deploy.tests.filter(needsDecision);
  return (
    <TitledCard
      className="min-w-0 grow"
      title="Tests d'acceptation"
      subtitle="un test par issue de la version"
      actions={
        <>
          <Badge tone="green">{counts.passed} ✓</Badge>
          {counts.failed > 0 && <Badge tone="red">{counts.failed} ✗</Badge>}
          {counts.unverifiable > 0 && <Badge tone="amber">{counts.unverifiable} ?</Badge>}
        </>
      }
      footer={
        pending[0] && (
          <Decision run={run} deploy={deploy} test={pending[0]} last={pending.length === 1} />
        )
      }
    >
      {deploy.tests.map((t) => (
        <TestRow key={t.number} test={t} />
      ))}
    </TitledCard>
  );
}

const statusIcon = {
  passed: { icon: "check", tone: "text-green" },
  failed: { icon: "close", tone: "text-red" },
  unverifiable: { icon: "warning", tone: "text-amber" },
  running: { icon: "clock", tone: "text-amber" },
  queued: { icon: "clock", tone: "text-tx3" },
} as const;

function TestRow({ test }: { test: DeployTest }) {
  const s = statusIcon[test.status];
  return (
    <div
      className="flex h-9 items-center gap-2.5 border-b border-line px-[13px]"
      title={[test.detail, ...test.criteria.map((c) => `· ${c}`)].filter(Boolean).join("\n")}
    >
      <span className={cn("flex", s.tone)}>
        {test.status === "running" ? <Spinner /> : <Icon name={s.icon} size={14} />}
      </span>
      <span className="w-8 mono text-tiny text-tx3">{test.number}</span>
      <span
        className={cn(
          "min-w-0 grow truncate text-body-sm",
          test.status === "failed" || test.status === "running" ? "text-tx" : "text-tx2",
        )}
      >
        {test.title}
      </span>
      {test.status === "failed" ? (
        <Badge tone={test.decision === "ignored" ? "neutral" : "red"}>
          {test.decision === "reopened"
            ? "rouverte"
            : test.decision === "ignored"
              ? "ignoré"
              : "à revoir"}
        </Badge>
      ) : test.status === "unverifiable" ? (
        <Badge tone="amber">non vérifiable</Badge>
      ) : (
        <span className="mono text-micro text-tx3">{formatTestDuration(test.durationMs)}</span>
      )}
    </div>
  );
}

/** Test en échec : rouvrir l'issue sur la version suivante, ou ignorer (et publier). */
function Decision({
  run,
  deploy,
  test,
  last,
}: {
  run: AgentRunDetail;
  deploy: DeployState;
  test: DeployTest;
  last: boolean;
}) {
  const apply = useApplyActions(run.run);
  const ignore = useRejectItems(run.run);
  const proposeOnly = run.autonomy === "propose";
  const publishAfter = last && !deploy.published && deploy.step >= 5 && !proposeOnly;
  const busy = apply.isPending || ignore.isPending;
  return (
    <div className="border-t border-line bg-p2 px-[13px] py-3">
      <div className="flex gap-[9px]">
        <span className="mt-px flex text-red">
          <Icon name="warning" size={15} />
        </span>
        <div className="text-small leading-[1.6] text-tx2">
          <span className="font-medium text-tx">
            #{test.number} — {test.title}.
          </span>{" "}
          {test.detail ?? "Un critère d'acceptation n'est pas rempli."}{" "}
          {deploy.nextVersion
            ? `L'agent propose de rouvrir l'issue sur ${deploy.nextVersion} plutôt que de bloquer la release.`
            : "Aucune version ouverte pour y rouvrir l'issue."}
          <div className="mt-[9px] flex gap-2">
            {deploy.nextVersion && (
              <Button
                size="sm"
                icon="issue"
                disabled={busy || proposeOnly || deploy.step < 4}
                onClick={() => apply.mutate([`reopen-${test.number}`])}
              >
                Rouvrir sur {deploy.nextVersion}
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              disabled={busy}
              onClick={() =>
                ignore.mutate([test.number], {
                  onSuccess: () => {
                    if (publishAfter) apply.mutate(["publish-release"]);
                  },
                })
              }
            >
              {publishAfter ? "Ignorer et publier" : "Ignorer"}
            </Button>
          </div>
          {(apply.data?.failed[0] ?? apply.error ?? ignore.error) && (
            <p className="mt-2 text-caption text-red">
              {apply.data?.failed[0]?.message ?? apply.error?.message ?? ignore.error?.message}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function NotesCard({ run, deploy }: { run: AgentRunDetail; deploy: DeployState }) {
  const edit = useEditReleaseNotes(run.run);
  const [editing, setEditing] = useState<string | null>(null);
  const [showLog, setShowLog] = useState(false);
  const lines = useRunLog(run);
  const active = run.state === "prepare" || run.state === "running";
  const showConsole = showLog || deploy.notes === null;
  const removed = deploy.tests.filter((t) => t.status === "failed" && !inNotes(t));

  if (showConsole) {
    return (
      <Card className="flex min-h-0 grow flex-col overflow-hidden">
        <SessionConsole
          className="min-h-0 grow"
          lines={lines}
          header={
            <>
              <StatusDot tone={active ? "green" : "neutral"} pulse={active && !run.paused} />
              <span className="grow mono text-micro text-tx3">
                claude code · {run.model ?? "défaut"}
              </span>
              {deploy.notes !== null && (
                <Button variant="ghost" size="xs" icon="file" onClick={() => setShowLog(false)}>
                  Notes
                </Button>
              )}
            </>
          }
        />
      </Card>
    );
  }

  return (
    <TitledCard
      className="grow"
      leading={<AgentBadge agent="deploy" size={22} />}
      title={deploy.published ? "Notes de version — publiées" : "Notes de version — brouillon"}
      actions={
        editing === null ? (
          <>
            <Button variant="quiet" size="xs" onClick={() => setShowLog(true)}>
              Journal
            </Button>
            {!deploy.published && (
              <Button
                variant="ghost"
                size="xs"
                icon="file"
                onClick={() => setEditing(deploy.notes ?? "")}
              >
                Éditer
              </Button>
            )}
          </>
        ) : (
          <>
            <Button variant="quiet" size="xs" onClick={() => setEditing(null)}>
              Annuler
            </Button>
            <Button
              variant="ghost"
              size="xs"
              icon="check"
              disabled={edit.isPending || !editing.trim()}
              onClick={() => edit.mutate(editing, { onSuccess: () => setEditing(null) })}
            >
              Enregistrer
            </Button>
          </>
        )
      }
      bodyClassName="px-3.5 py-3.5 text-body-sm leading-[1.75] text-tx2"
    >
      {editing !== null ? (
        <Textarea
          aria-label="Notes de version"
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          className="h-full min-h-[220px] w-full mono text-small"
        />
      ) : (
        <NotesBody notes={deploy.notes ?? ""} />
      )}
      {edit.isError && <p className="mt-2 text-caption text-red">{edit.error.message}</p>}
      <div className="mt-3.5 border-t border-line pt-3 text-caption text-tx3">
        Rédigé à partir des titres d'issues de la version et des tests de vérification
        {deploy.notesEdited && ", puis édité à la main"}.
        {removed.length > 0 && (
          <span className="text-amber">
            {" "}
            {removed.map((t) => `#${t.number}`).join(", ")} retiré{removed.length > 1 ? "s" : ""} :
            test en échec.
          </span>
        )}
        {deploy.releaseUrl && (
          <>
            {" "}
            <button
              type="button"
              className="text-blue hover:underline"
              onClick={() => void openExternal(deploy.releaseUrl!)}
            >
              Voir sur GitHub
            </button>
          </>
        )}
      </div>
    </TitledCard>
  );
}

function NotesBody({ notes }: { notes: string }) {
  return (
    <>
      {noteLines(notes).map((line, i) =>
        line.kind === "title" ? (
          <div key={i} className="mono text-body font-semibold text-tx">
            {line.text}
          </div>
        ) : line.kind === "section" ? (
          <div key={i} className="mt-2.5 font-semibold text-tx">
            {line.text}
          </div>
        ) : line.kind === "item" ? (
          <div key={i}>· {line.text}</div>
        ) : (
          <div key={i}>{line.text}</div>
        ),
      )}
    </>
  );
}

function ArtifactsCard({ deploy }: { deploy: DeployState }) {
  const file = (path: string) => path.split("/").pop() ?? path;
  return (
    <Card className="px-3.5 py-[13px]">
      <div className="flex items-center gap-2">
        <SectionLabel className="grow">Artefacts et documentation</SectionLabel>
        {deploy.worktree && (
          <button
            type="button"
            title={deploy.worktree}
            className="flex text-tx3 hover:text-tx"
            aria-label="Ouvrir le worktree"
            onClick={() => void revealPath(deploy.worktree!)}
          >
            <Icon name="folder" size={13} />
          </button>
        )}
      </div>
      <div className="mt-[9px] flex flex-col gap-[7px]">
        {deploy.artifacts.length === 0 && deploy.notes === null && (
          <p className="text-small text-tx3">Les artefacts apparaissent après le build.</p>
        )}
        {deploy.artifacts.map((a) => (
          <div key={a.path} className="flex items-center gap-2 text-small" title={a.path}>
            <span className="flex text-tx3">
              <Icon name="download" size={13} />
            </span>
            <span className="min-w-0 grow truncate mono text-tx2">{file(a.path)}</span>
            <span className="text-micro text-tx3">{formatSize(a.sizeKb)}</span>
          </div>
        ))}
        {deploy.notes !== null && (
          <div className="flex items-center gap-2 text-small">
            <span className="flex text-tx3">
              <Icon name="file" size={13} />
            </span>
            <span className="min-w-0 grow truncate mono text-tx2">
              notes de version {deploy.tag}
            </span>
            <span className="text-micro text-tx3">
              {deploy.published ? "publiées" : deploy.notesEdited ? "modifiées" : "rédigées"}
            </span>
          </div>
        )}
      </div>
    </Card>
  );
}
