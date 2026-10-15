/**
 * Agent CTO (maquette Agent-CTO.dc.html) : file de traitement à gauche, proposition de l'issue
 * choisie au centre (diff, questions, estimation), session Claude Code en direct à droite.
 *
 * L'agent lit le titre, la description et le code du dépôt (copie en lecture seule) ; il
 * n'écrit sur GitHub qu'après « Appliquer » (ou seul en autonomie `auto`).
 */
import { useMemo, useState } from "react";
import type { AgentRunDetail, CtoItem, CtoItemState } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon, type IconName } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { AgentQuestion } from "@/components/domain/AgentQuestion";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { TextDiff } from "@/components/domain/Diff";
import { FooterBar } from "@/components/domain/Panels";
import { SessionConsole } from "@/components/domain/SessionConsole";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Textarea } from "@/components/ui/Field";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { autonomyLabels } from "@/lib/domain/agents";
import { formatAgo, formatDuration } from "@/lib/domain/format";
import { lineDiff } from "@/lib/domain/textDiff";
import { useElapsedSeconds } from "@/lib/hooks/useElapsedSeconds";
import {
  useAgentLatestRun,
  useAnswerCto,
  useApplyActions,
  useCurrentProject,
  useEditProposal,
  useRejectItems,
  useStartAgent,
  useStopAgent,
} from "@/lib/query/queries";
import { openExternal } from "@/lib/platform";
import { useRunLog } from "./RunConsole";

const itemIcon: Record<CtoItemState, { icon: IconName; cls: string; label: string }> = {
  queued: { icon: "clock", cls: "text-tx3", label: "en file" },
  running: { icon: "refresh", cls: "text-clay", label: "en cours" },
  ready: { icon: "review", cls: "text-blue", label: "à relire" },
  questions: { icon: "review", cls: "text-amber", label: "questions" },
  applied: { icon: "check", cls: "text-green", label: "appliquée" },
  skipped: { icon: "close", cls: "text-tx3", label: "passée" },
  failed: { icon: "warning", cls: "text-red", label: "échec" },
};

export default function AgentCtoView() {
  const project = useCurrentProject().data;
  const latest = useAgentLatestRun("cto");
  const start = useStartAgent();
  const stop = useStopAgent();
  const run = latest.data ?? null;
  const active = run !== null && (run.state === "prepare" || run.state === "running");
  const current = project?.currentVersion ?? null;

  const launch = () => start.mutate({ kind: "cto", input: current ? { version: current } : {} });

  const subtitle = run
    ? [
        run.version && `version ${run.version}`,
        `${run.items.filter((i) => i.state === "applied").length} appliquée(s) sur ${run.items.length}`,
        autonomyLabels[run.autonomy].toLowerCase(),
      ]
        .filter(Boolean)
        .join(" · ")
    : "rend les issues exécutables : contexte, attendu, critères d'acceptation";

  return (
    <Page
      title="Agent CTO"
      subtitle={subtitle}
      padding={run ? "none" : "default"}
      actions={
        active ? (
          <Button icon="stop" disabled={stop.isPending} onClick={() => stop.mutate(run.run)}>
            Arrêter
          </Button>
        ) : (
          <Button variant="primary" icon="play" disabled={start.isPending || !current} onClick={launch}>
            {start.isPending && <Spinner />}
            {run ? "Relancer" : "Lancer"} sur {current ?? "…"}
          </Button>
        )
      }
    >
      {latest.isPending ? (
        <LoadingState label="Lecture du journal de l'agent…" />
      ) : latest.isError ? (
        <ErrorState error={latest.error} />
      ) : !run ? (
        <div className="mx-auto flex max-w-[620px] flex-col gap-4 pt-10">
          <EmptyState icon="bot" title="L'agent CTO n'a pas encore tourné sur ce dépôt">
            Il parcourt les issues ouvertes de la version, lit le code du dépôt et propose pour chacune
            une description structurée, une estimation et des labels. Quand le titre ne suffit pas, il
            pose une question plutôt que d'inventer.
          </EmptyState>
          {start.isError && <Callout tone="danger">{start.error.message}</Callout>}
          <Guardrails />
        </div>
      ) : (
        <RunScreen run={run} startError={start.isError ? start.error.message : null} />
      )}
    </Page>
  );
}

function Guardrails() {
  const rows: [IconName, string][] = [
    ["eye", "Lecture seule : il lit le code d'une copie du dépôt, sans pouvoir rien modifier ni exécuter."],
    ["check", "Rien n'est écrit sur GitHub avant votre validation (sauf autonomie « automatique »)."],
    ["clock", "Chaque session est plafonnée (30 min par défaut, Réglages › Agents)."],
    ["review", "Chaque écriture laisse un commentaire signé sur l'issue."],
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

/* ---------- Exécution ---------- */

function RunScreen({ run, startError }: { run: AgentRunDetail; startError: string | null }) {
  const [picked, setPicked] = useState<number | null>(null);
  // Par défaut : la première proposition à relire, sinon l'issue en cours.
  const selected =
    run.items.find((i) => i.number === picked) ??
    run.items.find((i) => i.state === "questions" || i.state === "ready") ??
    run.items.find((i) => i.state === "running") ??
    run.items[0] ??
    null;
  const done = run.items.filter((i) => i.state !== "queued" && i.state !== "running").length;

  const goNext = (from: number) => {
    const after = run.items.filter((i) => i.number !== from && (i.state === "ready" || i.state === "questions"));
    setPicked(after[0]?.number ?? null);
  };

  return (
    <div className="flex min-w-0 grow">
      <aside className="flex w-[268px] shrink-0 flex-col border-r border-line bg-panel">
        <div className="flex items-center gap-2 px-3.5 pt-3.5">
          <AgentBadge agent="cto" size={22} />
          <span className="grow text-body font-semibold">File de traitement</span>
          <span className="mono text-tiny text-tx3">
            {done}/{run.items.length}
          </span>
        </div>
        <ProgressBar
          value={run.items.length ? (done / run.items.length) * 100 : 0}
          tone="blue"
          height={3}
          className="mx-3.5 mt-2.5"
          label="Avancement de la file"
        />
        <div className="mt-2.5 flex min-h-0 grow flex-col gap-0.5 overflow-y-auto px-2">
          {run.items.map((item) => {
            const meta = itemIcon[item.state];
            return (
              <button
                key={item.number}
                type="button"
                onClick={() => setPicked(item.number)}
                aria-current={selected?.number === item.number ? "true" : undefined}
                title={meta.label}
                className={cn(
                  "flex h-9 items-center gap-2 rounded-control px-2 text-left text-body-sm",
                  selected?.number === item.number ? "bg-p3 text-tx" : "text-tx2 hover:bg-p2",
                )}
              >
                <span className={cn("flex", meta.cls, item.state === "running" && "animate-spin")}>
                  <Icon name={meta.icon} size={14} />
                </span>
                <span className="mono text-tiny text-tx3">{item.number}</span>
                <span className="min-w-0 truncate">{item.title}</span>
              </button>
            );
          })}
        </div>
        <p className="border-t border-line px-3.5 py-3 text-caption leading-[1.55] text-tx3">
          L'agent lit le titre, le code du dépôt et la description actuelle. Il n'écrit sur GitHub
          qu'après votre validation.
        </p>
      </aside>

      <section className="flex min-w-0 grow flex-col">
        {startError && (
          <Callout tone="danger" className="mx-5 mt-4">
            {startError}
          </Callout>
        )}
        {run.error && (
          <Callout tone="danger" className="mx-5 mt-4">
            {run.error}
          </Callout>
        )}
        {selected ? (
          <ItemReview key={selected.number} run={run} item={selected} onDone={() => goNext(selected.number)} />
        ) : (
          <EmptyState icon="bot" title="File vide" />
        )}
      </section>

      <RunConsole run={run} />
    </div>
  );
}

function ItemReview({ run, item, onDone }: { run: AgentRunDetail; item: CtoItem; onDone: () => void }) {
  const project = useCurrentProject().data;
  const apply = useApplyActions(run.run);
  const reject = useRejectItems(run.run);
  const editBody = useEditProposal(run.run);
  const answer = useAnswerCto(item.number);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<string | null>(null);

  const diff = useMemo(() => lineDiff(item.before, item.after ?? ""), [item.before, item.after]);
  const unanswered = item.questions.filter((q) => !q.answer);
  const typed = unanswered.filter((q) => answers[q.id]?.trim());
  const proposeOnly = run.autonomy === "propose";
  const error = apply.error ?? reject.error ?? editBody.error ?? answer.error;
  const failedApply = apply.data?.failed[0]?.message;
  const issueUrl = project ? `https://github.com/${project.repo.fullName}/issues/${item.number}` : null;

  const sendAnswers = () =>
    answer.mutate(typed.map((q) => ({ questionId: q.id, answer: answers[q.id]!.trim() })));
  const applyItem = () =>
    apply.mutate([`update-${item.number}`], {
      onSuccess: (report) => {
        if (report.failed.length === 0) onDone();
      },
    });

  return (
    <>
      <div className="shrink-0 border-b border-line px-5 py-4">
        <div className="flex items-center gap-2">
          <span className="mono text-small text-tx3">#{item.number}</span>
          {item.version && <VersionBadge version={item.version} />}
          {item.issueType && <TypeBadge type={item.issueType} />}
          <div className="grow" />
          {item.state === "questions" && (
            <Badge tone="amber">
              {unanswered.length} question{unanswered.length > 1 ? "s" : ""}
            </Badge>
          )}
          {item.state !== "questions" && <Badge tone="muted">{itemIcon[item.state].label}</Badge>}
          {issueUrl && (
            <Button variant="quiet" size="xs" icon="github" onClick={() => openExternal(issueUrl)}>
              GitHub
            </Button>
          )}
        </div>
        <h2 className="mt-2 font-display text-title font-semibold">{item.title}</h2>
      </div>

      <div className="min-h-0 grow overflow-y-auto px-5 py-4">
        {item.state === "queued" || item.state === "running" ? (
          <LoadingState
            label={item.state === "running" ? "L'agent lit le code et rédige…" : "En attente de son tour dans la file."}
          />
        ) : item.state === "failed" ? (
          <Callout tone="danger">{item.error ?? "La session a échoué."}</Callout>
        ) : !item.after ? (
          <EmptyState icon="close" title="Passée sans proposition" />
        ) : (
          <div className="flex flex-col gap-[18px]">
            <div>
              <SectionLabel>
                {editing !== null ? "Description à appliquer" : "Description proposée — diff avec l'existant"}
              </SectionLabel>
              {editing !== null ? (
                <Textarea
                  rows={14}
                  value={editing}
                  onChange={(e) => setEditing(e.target.value)}
                  className="mt-2 w-full mono text-small"
                  aria-label="Description à appliquer"
                />
              ) : (
                <TextDiff lines={diff} className="mt-2" />
              )}
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <SectionLabel>Questions de l'agent</SectionLabel>
                <div className="mt-2 flex flex-col gap-2">
                  {item.questions.length === 0 ? (
                    <p className="text-caption text-tx3">Aucune : le titre et le code suffisaient.</p>
                  ) : (
                    item.questions.map((q) =>
                      q.answer ? (
                        <div key={q.id} className="rounded-field border border-line bg-p2 px-3 py-[11px] text-body-sm">
                          <div className="text-tx2">{q.question}</div>
                          <div className="mt-1 text-tx">{q.answer}</div>
                        </div>
                      ) : (
                        <AgentQuestion
                          key={q.id}
                          variant="boxed"
                          question={q}
                          value={answers[q.id] ?? ""}
                          onChange={(v) => setAnswers((a) => ({ ...a, [q.id]: v }))}
                        />
                      ),
                    )
                  )}
                </div>
              </div>
              <div>
                <SectionLabel>Également proposé</SectionLabel>
                <dl className="mt-2 flex flex-col">
                  <Proposed label="Estimation">
                    {item.estimate ? `${item.estimate}${item.estimateHours ? ` · ~${item.estimateHours} h` : ""}` : "—"}
                  </Proposed>
                  <Proposed label="Labels ajoutés">{item.labels.length ? item.labels.join(", ") : "—"}</Proposed>
                  <Proposed label="Issues liées">
                    {item.related.length ? item.related.map((n) => `#${n}`).join(", ") : "—"}
                  </Proposed>
                  <Proposed label="Risque">{item.risk ?? "—"}</Proposed>
                </dl>
              </div>
            </div>

            {(error || failedApply) && <Callout tone="danger">{failedApply ?? error?.message}</Callout>}
          </div>
        )}
      </div>

      {(item.state === "ready" || item.state === "questions") && (
        <FooterBar
          hint={
            proposeOnly
              ? "Propose seulement : rien n'est écrit."
              : item.state === "questions"
                ? "Les réponses repartent à l'agent."
                : "Écrit sur l'issue GitHub."
          }
        >
          <Button variant="ghost" disabled={reject.isPending} onClick={() => reject.mutate([item.number], { onSuccess: onDone })}>
            Passer
          </Button>
          {editing === null ? (
            <Button icon="file" onClick={() => setEditing(item.after ?? "")}>
              Éditer avant d'appliquer
            </Button>
          ) : (
            <Button
              icon="check"
              disabled={editBody.isPending}
              onClick={() =>
                editBody.mutate({ number: item.number, body: editing }, { onSuccess: () => setEditing(null) })
              }
            >
              Enregistrer l'édition
            </Button>
          )}
          {item.state === "questions" ? (
            <Button variant="primary" icon="arrowRight" disabled={typed.length === 0 || answer.isPending} onClick={sendAnswers}>
              {answer.isPending && <Spinner />}
              Envoyer les réponses
            </Button>
          ) : (
            <Button
              variant="primary"
              icon="check"
              disabled={proposeOnly || editing !== null || apply.isPending}
              onClick={applyItem}
            >
              {apply.isPending && <Spinner />}
              Appliquer et continuer
            </Button>
          )}
        </FooterBar>
      )}
    </>
  );
}

function Proposed({ label, children }: { label: string; children: string }) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-line py-2 text-body-sm">
      <dt className="text-tx3">{label}</dt>
      <dd className="truncate mono text-small text-tx">{children}</dd>
    </div>
  );
}

/* ---------- Console ---------- */

function RunConsole({ run }: { run: AgentRunDetail }) {
  const lines = useRunLog(run);
  const active = run.state === "prepare" || run.state === "running" || run.items.some((i) => i.state === "running");
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
          {run.model ?? "modèle par défaut"} · {run.items.length} issue{run.items.length > 1 ? "s" : ""}
        </span>
      }
    />
  );
}
