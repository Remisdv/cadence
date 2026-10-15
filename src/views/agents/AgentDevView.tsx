/**
 * Agent développeur (maquette Agent-Dev.dc.html) : choix des tâches à gauche, déroulé de la
 * tâche choisie au centre (six étapes, fichiers touchés), terminal de session à droite avec
 * intervention en cours de route.
 *
 * Chaque tâche travaille dans un worktree isolé ; le code n'est poussé qu'après votre
 * validation (sauf autonomie `auto`), jamais sur la branche par défaut.
 */
import { useMemo, useState } from "react";
import type { AgentRunDetail, DevTask } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { FooterBar } from "@/components/domain/Panels";
import { PillTabs } from "@/components/domain/Filters";
import { Diffstat } from "@/components/domain/Inline";
import { SessionConsole } from "@/components/domain/SessionConsole";
import { StepTimeline, type StepState } from "@/components/domain/Steps";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { Switch } from "@/components/ui/Switch";
import { cn } from "@/lib/cn";
import { devSteps } from "@/lib/domain/agents";
import { formatDuration } from "@/lib/domain/format";
import { useElapsedSeconds } from "@/lib/hooks/useElapsedSeconds";
import { openExternal, revealPath } from "@/lib/platform";
import {
  useAgentLatestRun,
  useApplyActions,
  useControlAgent,
  useCurrentProject,
  useIssues,
  useRejectItems,
  useStartAgent,
  useStopAgent,
} from "@/lib/query/queries";
import { useRunLog } from "./RunConsole";

const ESTIMATE_HOURS: Record<string, string> = { S: "1 h", M: "4 h", L: "8 h", XL: "2 j" };

export default function AgentDevView() {
  const latest = useAgentLatestRun("dev");
  const run = latest.data ?? null;
  const active = run !== null && (run.state === "prepare" || run.state === "running");
  const control = useControlAgent(run?.run ?? "");
  const stop = useStopAgent();
  const running = run?.tasks.filter((t) => t.state === "running").length ?? 0;

  return (
    <Page
      title="Agent développeur"
      subtitle={`${running} tâche${running > 1 ? "s" : ""} en cours · worktree isolé · PR en brouillon`}
      padding="none"
      actions={
        active && (
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
        )
      }
    >
      {latest.isPending ? (
        <LoadingState label="Lecture du journal de l'agent…" />
      ) : latest.isError ? (
        <ErrorState error={latest.error} />
      ) : (
        <div className="flex min-w-0 grow">
          <TaskPicker run={run} active={active} />
          {run ? (
            <RunPanel run={run} />
          ) : (
            <div className="flex min-w-0 grow items-center justify-center">
              <EmptyState icon="branch" title="Aucune tâche lancée sur ce dépôt">
                Choisissez des issues à gauche. Pour chacune, l'agent analyse le code, ouvre une
                branche et une PR en brouillon, implémente, lance les tests, puis attend votre
                validation avant de pousser.
              </EmptyState>
            </div>
          )}
        </div>
      )}
    </Page>
  );
}

/* ---------- Choix des tâches ---------- */

function TaskPicker({ run, active }: { run: AgentRunDetail | null; active: boolean }) {
  const project = useCurrentProject().data;
  const current = project?.currentVersion ?? null;
  const issues = useIssues({}).data ?? [];
  const start = useStartAgent();
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [runTests, setRunTests] = useState(true);

  const candidates = issues.filter((i) => i.version === current && i.status !== "done");
  const busy = new Set(run?.tasks.filter((t) => t.state === "running" || t.state === "queued").map((t) => t.number));
  const toggle = (n: number) => {
    const next = new Set(picked);
    if (next.has(n)) next.delete(n);
    else next.add(n);
    setPicked(next);
  };

  return (
    <aside className="flex w-[312px] shrink-0 flex-col gap-3 border-r border-line bg-panel px-3.5 py-4">
      <div className="flex items-center gap-[9px]">
        <AgentBadge agent="dev" size={26} />
        <span className="grow text-body font-semibold">Quelles tâches lancer ?</span>
      </div>
      <div className="flex h-[30px] items-center gap-[7px] rounded-control border border-line bg-p2 px-2.5 text-body-sm text-tx2">
        <Icon name="tag" size={13} /> Version {current ?? "—"}
      </div>
      <div className="flex min-h-0 grow flex-col gap-1.5 overflow-y-auto">
        {candidates.length === 0 && <p className="text-caption text-tx3">Aucune issue ouverte sur cette version.</p>}
        {candidates.map((issue) => (
          <label
            key={issue.number}
            className={cn(
              "flex cursor-pointer items-center gap-2.5 rounded-lg border px-[11px] py-[9px]",
              picked.has(issue.number) ? "border-green-line bg-p3" : "border-line bg-p2",
            )}
          >
            <input
              type="checkbox"
              checked={picked.has(issue.number)}
              onChange={() => toggle(issue.number)}
              disabled={active}
              aria-label={issue.title}
              className="size-3.5 accent-green"
            />
            <span className="w-[30px] mono text-tiny text-tx3">{issue.number}</span>
            <span className="min-w-0 grow truncate text-small">{issue.title}</span>
            {busy.has(issue.number) && <Badge tone="green">en cours</Badge>}
            {!busy.has(issue.number) && issue.flags.includes("cto-questions") && <Badge tone="amber">questions</Badge>}
            <span className="w-9 text-right mono text-tiny whitespace-nowrap text-tx3">
              {issue.estimate ? (ESTIMATE_HOURS[issue.estimate] ?? issue.estimate) : "—"}
            </span>
          </label>
        ))}
      </div>

      <div className="rounded-field border border-line bg-p2 px-3 py-[11px]">
        <SectionLabel>Avant de lancer</SectionLabel>
        <div className="mt-[9px] flex flex-col gap-[9px]">
          <Toggle checked disabled label="Worktree isolé par issue" hint="Copie gérée par Cadence ; votre dépôt n'est pas touché" />
          <Toggle checked disabled label="PR en brouillon dès le départ" hint="Sauf autonomie « propose seulement »" />
          <Toggle checked={runTests} onChange={setRunTests} label="Faire lancer les tests (étape 4)" />
        </div>
      </div>

      {start.isError && <Callout tone="danger">{start.error.message}</Callout>}
      <div>
        <Button
          variant="success"
          icon={start.isPending ? undefined : "play"}
          className="w-full"
          disabled={active || picked.size === 0 || start.isPending}
          onClick={() =>
            start.mutate(
              { kind: "dev", input: { issues: [...picked], runTests } },
              { onSuccess: () => setPicked(new Set()) },
            )
          }
        >
          {start.isPending && <Spinner />}
          Lancer sur {picked.size} tâche{picked.size > 1 ? "s" : ""}
        </Button>
        <p className="mt-2 text-tiny leading-[1.55] text-tx3">Les tâches s'exécutent l'une après l'autre.</p>
      </div>
    </aside>
  );
}

function Toggle({
  checked,
  onChange,
  disabled,
  label,
  hint,
}: {
  checked: boolean;
  onChange?: (v: boolean) => void;
  disabled?: boolean;
  label: string;
  hint?: string;
}) {
  return (
    <div className="flex items-center gap-3">
      <Switch checked={checked} onCheckedChange={onChange ?? (() => {})} disabled={disabled} aria-label={label} />
      <div className="min-w-0">
        <div className="text-body text-tx">{label}</div>
        {hint && <div className="mt-0.5 text-caption text-tx3">{hint}</div>}
      </div>
    </div>
  );
}

/* ---------- Exécution ---------- */

function RunPanel({ run }: { run: AgentRunDetail }) {
  const defaultTask =
    run.tasks.find((t) => t.state === "running") ?? run.tasks.find((t) => t.state === "review") ?? run.tasks[0] ?? null;
  const [picked, setPicked] = useState<number | null>(null);
  const task = run.tasks.find((t) => t.number === picked) ?? defaultTask;

  return (
    <>
      <section className="flex min-w-0 grow flex-col">
        {run.error && (
          <Callout tone="danger" className="mx-5 mt-4">
            {run.error}
          </Callout>
        )}
        {task ? <TaskDetail run={run} task={task} /> : <EmptyState icon="branch" title="Aucune tâche" />}
      </section>
      <DevConsole run={run} selected={task?.number ?? null} onSelect={setPicked} />
    </>
  );
}

function TaskDetail({ run, task }: { run: AgentRunDetail; task: DevTask }) {
  const apply = useApplyActions(run.run);
  const skip = useRejectItems(run.run);
  const elapsed = useElapsedSeconds(task.state === "running" ? task.startedAt : null);
  const steps = useMemo(
    () =>
      devSteps.map((title, i) => {
        const n = i + 1;
        const state: StepState =
          task.state === "done" || n < task.step ? "done" : n === task.step && task.state !== "queued" ? "current" : "pending";
        return { title, detail: task.stepNotes[i], state };
      }),
    [task],
  );
  const failure = apply.data?.failed[0]?.message ?? task.error;
  const review = task.state === "review";

  return (
    <>
      <div className="flex shrink-0 items-center gap-[11px] border-b border-line px-5 py-[15px]">
        <span className="mono text-small text-tx3">#{task.number}</span>
        <h2 className="min-w-0 grow truncate font-display text-title-sm font-semibold">{task.title}</h2>
        <Badge tone={task.state === "failed" ? "red" : task.state === "review" ? "amber" : "green"} dot>
          {task.state === "queued"
            ? "en file"
            : task.state === "review"
              ? "à valider"
              : task.state === "done"
                ? "terminée"
                : task.state === "failed"
                  ? "échec"
                  : task.state === "skipped"
                    ? "passée"
                    : `étape ${task.step}/6`}
        </Badge>
        {task.state === "running" && <span className="mono text-caption text-tx3">{formatDuration(elapsed)}</span>}
      </div>

      <div className="min-h-0 grow overflow-y-auto px-5 py-4">
        <SectionLabel>Déroulé</SectionLabel>
        <div className="mt-[11px]">
          <StepTimeline steps={steps} />
        </div>
        {failure && (
          <Callout tone="danger" className="mt-3">
            {failure}
          </Callout>
        )}
        {task.files.length > 0 && (
          <div className="mt-1.5 rounded-[10px] border border-line bg-p2 px-3.5 py-3">
            <div className="flex items-center gap-2">
              <SectionLabel>Fichiers touchés</SectionLabel>
              <div className="grow" />
              <Diffstat additions={task.additions} deletions={task.deletions} />
            </div>
            <div className="mt-1.5">
              {task.files.map((f) => (
                <div key={f.path} className="flex items-center gap-2 py-1.5 text-caption">
                  <span className={cn("flex", f.added ? "text-green" : "text-blue")}>
                    <Icon name="file" size={13} />
                  </span>
                  <span className="min-w-0 grow truncate mono">{f.path}</span>
                  <Diffstat additions={f.additions} deletions={f.deletions} plain className="text-micro" />
                </div>
              ))}
            </div>
          </div>
        )}
        {task.summary && (
          <p className="mt-3 text-body-sm leading-[1.6] whitespace-pre-wrap text-tx2">{task.summary}</p>
        )}
      </div>

      <FooterBar
        height={50}
        hint={
          review
            ? undefined
            : run.autonomy === "propose"
              ? "Propose seulement : rien n'est poussé."
              : "Le travail n'est poussé qu'à l'étape 5."
        }
      >
        {task.worktree && (
          <Button
            icon="folder"
            size={review ? "sm" : "md"}
            title={task.worktree}
            onClick={() => void revealPath(task.worktree!)}
          >
            {review ? "Worktree" : "Ouvrir le worktree"}
          </Button>
        )}
        {task.prUrl && (
          <Button icon="pullRequest" size={review ? "sm" : "md"} onClick={() => void openExternal(task.prUrl!)}>
            {review ? `PR #${task.pr}` : `Voir la PR #${task.pr}`}
          </Button>
        )}
        {review && (
          <>
            <Button variant="ghost" size="sm" disabled={skip.isPending} onClick={() => skip.mutate([task.number])}>
              Passer
            </Button>
            <Button
              variant="primary"
              icon={apply.isPending ? undefined : "rocket"}
              disabled={apply.isPending}
              title="Pousse la branche, sort la PR du brouillon et laisse un commentaire signé"
              onClick={() => apply.mutate([`push-${task.number}`])}
            >
              {apply.isPending && <Spinner />}
              Pousser la branche
            </Button>
          </>
        )}
      </FooterBar>
    </>
  );
}

function DevConsole({
  run,
  selected,
  onSelect,
}: {
  run: AgentRunDetail;
  selected: number | null;
  onSelect: (n: number) => void;
}) {
  const lines = useRunLog(run);
  const control = useControlAgent(run.run);
  const [message, setMessage] = useState("");
  const active = run.state === "prepare" || run.state === "running";
  const send = () => {
    if (!message.trim()) return;
    control.mutate({ cmd: "message", text: message.trim() }, { onSuccess: () => setMessage("") });
  };

  return (
    <SessionConsole
      className="w-[392px] shrink-0 border-l border-line"
      lines={lines}
      header={
        <>
          <PillTabs
            variant="mono"
            aria-label="Tâches"
            value={String(selected ?? "")}
            onChange={(v) => onSelect(Number(v))}
            items={run.tasks.map((t) => ({ value: String(t.number), label: `#${t.number}` }))}
          />
          <div className="grow" />
          <StatusDot tone={active ? "green" : "neutral"} pulse={active && !run.paused} />
          <span className="mono text-micro text-tx3">claude code · {run.model ?? "défaut"}</span>
        </>
      }
      footer={
        <form
          className="flex grow items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            send();
          }}
        >
          <input
            aria-label="Message à l'agent"
            placeholder={active ? "Intervenir… (repris à l'étape suivante)" : "Exécution terminée"}
            disabled={!active}
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            className="h-[26px] grow bg-transparent text-small text-tx outline-none placeholder:text-tx3"
          />
          <button type="submit" aria-label="Envoyer" disabled={!active || !message.trim()} className="flex text-tx3 hover:text-tx disabled:opacity-50">
            <Icon name="arrowRight" size={15} />
          </button>
        </form>
      }
    />
  );
}
