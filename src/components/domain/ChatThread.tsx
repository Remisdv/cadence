/**
 * Fil d'une conversation (V2-Chat.dc.html) : bulle de l'utilisateur à droite ; réponse de l'IA
 * avec sa tuile, son en-tête (modèle, outils, durée), les cartes d'outil repliables, les
 * commandes shell, les demandes d'approbation, le texte en Markdown et les erreurs.
 *
 * Les éléments viennent de `buildThread` (`lib/domain/chat.ts`) ; le composant ne lit aucune
 * donnée lui-même.
 *
 * `showCommands` faux : commandes, appels d'outils et fichiers modifiés d'un tour se résument en
 * une ligne (« 6 commandes · 2 fichiers modifiés ») qui les déplie au clic.
 *
 * Largeur : tout reste dans la colonne du fil. Les lignes de commande se tronquent (ellipse) et
 * se lisent en entier, coupées n'importe où, une fois la carte dépliée ; les sorties défilent
 * dans leur cadre ; textes et chemins se coupent au besoin (`wrap-anywhere`).
 */
import { useState, type ReactNode } from "react";
import { Icon } from "@/components/icons/Icon";
import { Markdown } from "@/components/domain/Markdown";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import {
  activitySummary,
  compactArgs,
  composeAnswer,
  formatDuration,
  groupTurns,
  isActivity,
  type ChatItem,
  type ChatTurn,
} from "@/lib/domain/chat";
import type { AiQuestion, ApprovalDecision, QuestionAnswer } from "@/lib/types";

type Item<K extends ChatItem["kind"]> = Extract<ChatItem, { kind: K }>;

export interface ChatThreadProps {
  items: ChatItem[];
  /** Nom de l'IA (« Claude Code »). */
  ai: string;
  /** Modèle annoncé par la session (« claude-opus-5 »). */
  model?: string | null;
  /** Un tour est en cours : indicateur d'attente tant que rien n'arrive. */
  running?: boolean;
  onApprove?: (request: string, decision: ApprovalDecision) => void;
  /** Réponses aux questions à choix de l'IA, une par question ; un rejet rouvre la carte. */
  onAnswer?: (request: string, answers: QuestionAnswer[]) => void | Promise<unknown>;
  /** Clic sur un lien du texte ; `true` s'il est traité (liens `cadence:`). */
  onLink?: (href: string) => boolean;
  /** Commandes, outils et fichiers montrés (défaut) ; sinon résumés par tour. */
  showCommands?: boolean;
}

export function ChatThread({
  items,
  ai,
  model,
  running,
  onApprove,
  onAnswer,
  onLink,
  showCommands = true,
}: ChatThreadProps) {
  const turns = groupTurns(items);
  return (
    <div className="flex flex-col gap-5">
      {turns.map((turn, i) => (
        <TurnView
          key={turn.key}
          turn={turn}
          ai={ai}
          model={model}
          running={running === true && i === turns.length - 1}
          onApprove={onApprove}
          onAnswer={onAnswer}
          onLink={onLink}
          showCommands={showCommands}
        />
      ))}
    </div>
  );
}

function TurnView({
  turn,
  ai,
  model,
  running,
  onApprove,
  onAnswer,
  onLink,
  showCommands,
}: {
  turn: ChatTurn;
  ai: string;
  model?: string | null;
  running: boolean;
  onApprove?: ChatThreadProps["onApprove"];
  onAnswer?: ChatThreadProps["onAnswer"];
  onLink?: ChatThreadProps["onLink"];
  showCommands: boolean;
}) {
  /** Activité dépliée pour ce tour malgré l'interrupteur. */
  const [unfolded, setUnfolded] = useState(false);
  const visible = turn.reply.filter((i) => i.kind !== "turn");
  const rows = showCommands ? visible : fold(visible, unfolded);
  const end = turn.reply.find((i): i is Item<"turn"> => i.kind === "turn");
  const tools = turn.reply.filter((i) => i.kind === "tool" || i.kind === "command").length;
  const meta = [
    model ? shortModel(model) : null,
    tools > 0 ? `${tools} outil${tools > 1 ? "s" : ""}` : null,
    end && end.durationMs > 0 ? formatDuration(end.durationMs) : null,
  ].filter(Boolean);
  // « travaille… » tant que rien ne s'écrit et qu'aucune approbation n'attend l'utilisateur.
  const waiting =
    running &&
    !turn.reply.some(
      (i) =>
        (i.kind === "assistant" && i.streaming) ||
        (i.kind === "approval" && !i.decision) ||
        (i.kind === "question" && i.answer === null),
    );

  return (
    <>
      {turn.user && <UserBubble text={turn.user.text} />}
      {(visible.length > 0 || running || end) && (
        <div className="flex gap-3">
          <AiTile />
          <div className="flex min-w-0 grow flex-col gap-2.5">
            <div className="flex items-center gap-2 text-body-sm">
              <span className="font-semibold">{ai}</span>
              {meta.length > 0 && <span className="text-tx3">{meta.join(" · ")}</span>}
            </div>
            {segments(rows).map((segment) =>
              Array.isArray(segment) ? (
                <div key={segment[0]!.key} className="flex min-w-0 flex-col gap-1.5">
                  {segment.map((item) =>
                    item.kind === "tool" ? (
                      <ToolCard key={item.key} item={item} />
                    ) : (
                      <CommandCard key={item.key} item={item as Item<"command">} />
                    ),
                  )}
                </div>
              ) : segment.kind === "activity" ? (
                <ActivitySummary
                  key={segment.key}
                  items={segment.items}
                  open={unfolded}
                  onToggle={() => setUnfolded((open) => !open)}
                />
              ) : (
                <ReplyItem
                  key={segment.key}
                  item={segment}
                  onApprove={onApprove}
                  onAnswer={onAnswer}
                  onLink={onLink}
                />
              ),
            )}
            {waiting && (
              <span className="flex items-center gap-2 text-body-sm text-tx3">
                <Spinner size={13} />
                {ai} travaille…
              </span>
            )}
            {end && !end.ok && (end.interrupted || !visible.some((i) => i.kind === "error")) && (
              <TurnEnd item={end} />
            )}
          </div>
        </div>
      )}
    </>
  );
}

/** Résumé de l'activité d'un tour, à la place de sa première commande. */
type ActivityRow = { kind: "activity"; key: string; items: ChatItem[] };
type Row = ChatItem | ActivityRow;

/**
 * Activité repliée : le résumé remplace commandes, outils et fichiers. Dépliée : le résumé
 * reste en tête, comme bouton pour replier, et tout reprend sa place.
 */
function fold(items: ChatItem[], unfolded: boolean): Row[] {
  const activity = items.filter(isActivity);
  if (activity.length === 0) return items;
  const out: Row[] = [];
  for (const item of items) {
    if (item === activity[0])
      out.push({ kind: "activity", key: `activite-${item.key}`, items: activity });
    if (unfolded || !isActivity(item)) out.push(item);
  }
  return out;
}

/** Regroupe les cartes d'outil et de commande consécutives (6px entre elles). */
function segments(items: Row[]): (Row | ChatItem[])[] {
  const out: (Row | ChatItem[])[] = [];
  for (const item of items) {
    if (item.kind === "tool" || item.kind === "command") {
      const last = out.at(-1);
      if (Array.isArray(last)) last.push(item);
      else out.push([item]);
    } else out.push(item);
  }
  return out;
}

/** Ligne repliée de l'activité d'un tour : « 6 commandes · 2 fichiers modifiés ». */
export function ActivitySummary({
  items,
  open,
  onToggle,
}: {
  items: ChatItem[];
  open: boolean;
  onToggle?: () => void;
}) {
  const running = items.some(
    (i) => (i.kind === "tool" && i.status === "running") || (i.kind === "command" && i.running),
  );
  const failed = items.some(
    (i) =>
      (i.kind === "tool" && i.status === "error") ||
      (i.kind === "command" && !i.running && i.exitCode !== null && i.exitCode !== 0),
  );
  return (
    <button
      type="button"
      aria-expanded={open}
      title={open ? "Replier les commandes de ce tour" : "Afficher les commandes de ce tour"}
      onClick={onToggle}
      className="inline-flex h-7 max-w-full min-w-0 items-center gap-2 self-start rounded-field border border-line bg-p2 px-2.5 text-caption text-tx2 hover:border-line2 hover:text-tx"
    >
      {running ? (
        <Spinner size={12} className="shrink-0" />
      ) : (
        <span className="flex shrink-0 text-tx3">
          <Icon name="terminal" size={13} />
        </span>
      )}
      <span className="min-w-0 truncate">{activitySummary(items)}</span>
      {failed && <span className="shrink-0 text-red">· en échec</span>}
      <span className={cn("flex shrink-0 text-tx3 transition-transform", open && "rotate-180")}>
        <Icon name="chevronDown" size={13} />
      </span>
    </button>
  );
}

/** `claude-opus-5` → `Opus 5`, `claude-haiku-4-5-20251001` → `Haiku 4.5`. */
function shortModel(model: string): string {
  const m = /^claude-([a-z]+)-(\d+)(?:-(\d))?(?:-\d{8})?$/.exec(model);
  if (!m) return model;
  const name = m[1]!.charAt(0).toUpperCase() + m[1]!.slice(1);
  return m[3] ? `${name} ${m[2]}.${m[3]}` : `${name} ${m[2]}`;
}

export function AiTile() {
  return (
    <span
      aria-hidden="true"
      className="inline-flex size-[26px] shrink-0 items-center justify-center rounded-control bg-clay text-ink"
    >
      <Icon name="sparkle" size={15} />
    </span>
  );
}

/** Bulle de l'utilisateur, à droite (520px au plus). */
export function UserBubble({ text }: { text: string }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[520px] min-w-0 rounded-card rounded-br-[4px] border border-line2 bg-p3 px-3.5 py-[11px] text-body leading-[1.6] wrap-anywhere whitespace-pre-wrap">
        {text}
      </div>
    </div>
  );
}

function ReplyItem({
  item,
  onApprove,
  onAnswer,
  onLink,
}: {
  item: ChatItem;
  onApprove?: ChatThreadProps["onApprove"];
  onAnswer?: ChatThreadProps["onAnswer"];
  onLink?: ChatThreadProps["onLink"];
}) {
  switch (item.kind) {
    case "assistant":
      return (
        <div className="max-w-[720px] min-w-0 wrap-anywhere">
          <Markdown
            source={item.text}
            onLink={onLink}
            breaks
            className="gap-2.5 leading-[1.65] text-tx"
          />
          {item.streaming && (
            <span
              aria-hidden="true"
              className="ml-0.5 inline-block h-3.5 w-[7px] animate-pulse bg-tx3 align-middle"
            />
          )}
        </div>
      );
    case "reasoning":
      return (
        <details className="max-w-[720px] min-w-0 text-body-sm text-tx3">
          <summary className="cursor-pointer list-none">Réflexion</summary>
          <p className="mt-1.5 leading-[1.6] wrap-anywhere whitespace-pre-wrap">{item.text}</p>
        </details>
      );
    case "file":
      return (
        <span className="flex min-w-0 items-center gap-2 text-small text-tx2">
          <span className="flex shrink-0 text-tx3">
            <Icon name="file" size={13} />
          </span>
          {item.change === "added"
            ? "Fichier créé"
            : item.change === "deleted"
              ? "Fichier supprimé"
              : "Fichier modifié"}
          <span title={item.path} className="min-w-0 truncate mono text-tx3">
            {item.path}
          </span>
        </span>
      );
    case "plan":
      return <PlanCard item={item} />;
    case "approval":
      return <ApprovalCard item={item} onApprove={onApprove} />;
    case "question":
      return <QuestionCard item={item} onAnswer={onAnswer} />;
    case "report":
      return <ReportLine item={item} />;
    case "error":
      return (
        <div className="flex max-w-[720px] items-start gap-2 rounded-field border border-red-line bg-red-d px-3 py-2.5 text-body-sm leading-[1.55] text-tx2">
          <span className="mt-px flex shrink-0 text-red">
            <Icon name="warning" size={14} />
          </span>
          <Markdown
            source={item.message}
            className="min-w-0 text-body-sm leading-[1.55] wrap-anywhere text-tx2"
          />
        </div>
      );
    default:
      return null;
  }
}

function TurnEnd({ item }: { item: Item<"turn"> }) {
  if (item.interrupted)
    return (
      <span className="flex items-center gap-2 text-small text-tx3">
        <Icon name="stop" size={12} />
        Tour interrompu
      </span>
    );
  return item.error ? <span className="text-small text-red">{item.error}</span> : null;
}

/** Carte d'outil repliable : statut, nom exposé, arguments, résumé du résultat. */
export function ToolCard({ item }: { item: Item<"tool"> }) {
  const name =
    item.source.kind === "cadence" ? `${item.source.plugin}__${item.source.tool}` : item.name;
  return (
    <ToolShell
      status={item.status}
      icon={<Icon name="wrench" size={14} />}
      name={name}
      args={compactArgs(item.input)}
      result={item.summary}
    >
      <pre className="min-w-0 overflow-x-auto mono text-tiny leading-[1.6] wrap-anywhere whitespace-pre-wrap text-tx3">
        {JSON.stringify(item.input, null, 2)}
      </pre>
    </ToolShell>
  );
}

/**
 * Commande shell de la CLI : la commande sur une ligne (tronquée) en résumé ; dépliée, la
 * commande entière (coupée n'importe où) et sa sortie, qui défile dans son cadre.
 */
export function CommandCard({ item }: { item: Item<"command"> }) {
  const status = item.running ? "running" : item.exitCode && item.exitCode !== 0 ? "error" : "ok";
  return (
    <ToolShell
      status={status}
      icon={<Icon name="bolt" size={14} />}
      name={item.command}
      result={item.exitCode !== null && item.exitCode !== 0 ? `code ${item.exitCode}` : null}
    >
      <pre className="min-w-0 mono text-tiny leading-[1.6] break-all whitespace-pre-wrap text-tx">
        {item.command}
      </pre>
      {item.output && (
        <pre className="max-h-[260px] min-w-0 overflow-auto rounded-control bg-console px-2.5 py-2 mono text-tiny leading-[1.6] whitespace-pre text-tx2">
          {item.output}
        </pre>
      )}
    </ToolShell>
  );
}

function ToolShell({
  status,
  icon,
  name,
  args,
  result,
  children,
}: {
  status: "running" | "ok" | "error";
  icon: ReactNode;
  name: string;
  args?: string;
  result: string | null;
  children?: ReactNode;
}) {
  return (
    <details className="group min-w-0 rounded-field border border-line bg-p2">
      <summary className="flex h-9 min-w-0 cursor-pointer list-none items-center gap-[9px] px-3">
        {status === "running" ? (
          <Spinner size={14} className="shrink-0" />
        ) : (
          <span className={cn("flex shrink-0", status === "ok" ? "text-green" : "text-red")}>
            <Icon name={status === "ok" ? "check" : "close"} size={14} />
          </span>
        )}
        <span className="flex shrink-0 text-tx3">{icon}</span>
        <span title={name} className="min-w-0 truncate mono text-small text-tx">
          {name}
        </span>
        {args && (
          <span title={args} className="min-w-0 shrink-[3] truncate mono text-caption text-tx3">
            {args}
          </span>
        )}
        <div className="grow" />
        {result && (
          <span className="max-w-[40%] min-w-0 shrink-0 truncate text-caption text-tx2">
            {result}
          </span>
        )}
        <span className="flex shrink-0 text-tx3 transition-transform group-open:rotate-180">
          <Icon name="chevronDown" size={14} />
        </span>
      </summary>
      {children && (
        <div className="flex min-w-0 flex-col gap-[5px] pt-1 pr-3 pb-2.5 pl-[37px]">{children}</div>
      )}
    </details>
  );
}

function PlanCard({ item }: { item: Item<"plan"> }) {
  return (
    <div className="flex max-w-[720px] flex-col gap-1.5 rounded-field border border-line bg-p2 px-3 py-2.5">
      <span className="text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">Plan</span>
      {item.items.map((step, i) => (
        <span key={i} className="flex items-center gap-2 text-body-sm">
          <span
            className={cn(
              "flex",
              step.status === "completed"
                ? "text-green"
                : step.status === "in-progress"
                  ? "text-clay"
                  : "text-tx3",
            )}
          >
            <Icon
              name={
                step.status === "completed"
                  ? "check"
                  : step.status === "in-progress"
                    ? "play"
                    : "clock"
              }
              size={13}
            />
          </span>
          <span
            className={cn(
              "min-w-0 wrap-anywhere",
              step.status === "completed" ? "text-tx3 line-through" : "text-tx2",
            )}
          >
            {step.text}
          </span>
        </span>
      ))}
    </div>
  );
}

/**
 * Questions à choix de l'IA (`AskUserQuestion`, et l'équivalent de Codex et d'OpenCode) : une
 * ou plusieurs questions, choix unique ou multiple, champ « Autre » ; « Envoyer » quand chaque
 * question a sa réponse. Une fois répondu, la carte montre les réponses données.
 */
export function QuestionCard({
  item,
  onAnswer,
}: {
  item: Item<"question">;
  onAnswer?: ChatThreadProps["onAnswer"];
}) {
  const [picked, setPicked] = useState<Record<string, string[]>>({});
  const [other, setOther] = useState<Record<string, string>>({});
  const [sent, setSent] = useState(false);
  const answered = item.answer !== null;
  const answers: QuestionAnswer[] = item.questions.map((q) => ({
    id: q.id,
    answers: composeAnswer(q, picked[q.id] ?? [], other[q.id] ?? ""),
  }));
  const complete = answers.every((a) => a.answers.length > 0);
  const toggle = (question: AiQuestion, label: string) =>
    setPicked((prev) => {
      const current = prev[question.id] ?? [];
      const next = question.multiSelect
        ? current.includes(label)
          ? current.filter((l) => l !== label)
          : [...current, label]
        : current.includes(label)
          ? []
          : [label];
      return { ...prev, [question.id]: next };
    });
  const submit = () => {
    if (!complete || sent) return;
    setSent(true);
    void Promise.resolve(onAnswer?.(item.key, answers)).catch(() => setSent(false));
  };
  const given = (question: AiQuestion) =>
    item.answers.find((a) => a.id === question.id)?.answers.join(", ");

  return (
    <div
      className={cn(
        "flex max-w-[720px] flex-col gap-3 rounded-field border px-3.5 py-3",
        answered ? "border-line bg-p2" : "border-blue-line bg-blue-d",
      )}
    >
      <div className="flex items-center gap-2 text-body-sm">
        <span className={cn("flex", answered ? "text-tx3" : "text-blue")}>
          <Icon name="chat" size={14} />
        </span>
        <span className="min-w-0 grow font-medium">
          {item.questions.length > 1
            ? `${item.questions.length} questions de l'IA`
            : "Question de l'IA"}
        </span>
        {answered && <Badge tone="green">Répondu</Badge>}
      </div>
      {item.questions.map((question) => (
        <fieldset key={question.id} className="flex min-w-0 flex-col gap-1.5">
          <legend className="contents">
            {question.header && (
              <span className="text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">
                {question.header}
              </span>
            )}
            <span className="text-body-sm leading-[1.55] wrap-anywhere whitespace-pre-wrap text-tx">
              {question.question}
            </span>
          </legend>
          {answered ? (
            <span className="text-small text-tx2">
              {given(question) ?? (item.questions.length === 1 ? item.answer : null) ?? "—"}
            </span>
          ) : (
            <>
              {question.multiSelect && (
                <span className="text-caption text-tx3">Plusieurs choix possibles</span>
              )}
              <div className="flex flex-col gap-[5px]">
                {question.options.map((option) => {
                  const on = (picked[question.id] ?? []).includes(option.label);
                  return (
                    <button
                      key={option.label}
                      type="button"
                      role={question.multiSelect ? "checkbox" : "radio"}
                      aria-label={option.label}
                      aria-checked={on}
                      disabled={sent}
                      onClick={() => toggle(question, option.label)}
                      className={cn(
                        "flex items-start gap-2.5 rounded-control border px-2.5 py-[7px] text-left transition-colors disabled:opacity-60",
                        on ? "border-blue-line bg-panel" : "border-line bg-p2 hover:bg-p3",
                      )}
                    >
                      <span
                        className={cn(
                          "mt-0.5 inline-flex size-3.5 shrink-0 items-center justify-center border",
                          question.multiSelect ? "rounded-[4px]" : "rounded-full",
                          on ? "border-blue bg-blue text-bg" : "border-line2 bg-panel",
                        )}
                      >
                        {on && <Icon name="check" size={10} />}
                      </span>
                      <span className="flex min-w-0 flex-col">
                        <span className="text-body-sm font-medium text-tx">{option.label}</span>
                        {option.description && (
                          <span className="text-caption leading-normal text-tx3">
                            {option.description}
                          </span>
                        )}
                      </span>
                    </button>
                  );
                })}
              </div>
              {question.other && (
                <input
                  aria-label={`Autre réponse : ${question.question}`}
                  placeholder={question.options.length > 0 ? "Autre réponse…" : "Votre réponse…"}
                  value={other[question.id] ?? ""}
                  disabled={sent}
                  onChange={(e) => setOther((prev) => ({ ...prev, [question.id]: e.target.value }))}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      submit();
                    }
                  }}
                  className="h-7 min-w-0 rounded-control border border-line2 bg-panel px-2.5 text-body-sm text-tx outline-none focus:border-blue-line"
                />
              )}
            </>
          )}
        </fieldset>
      ))}
      {!answered && (
        <div className="flex items-center justify-end gap-2">
          {!complete && (
            <span className="text-caption text-tx3">
              {item.questions.length > 1 ? "Répondez à chaque question" : "Choisissez une réponse"}
            </span>
          )}
          <Button
            size="sm"
            variant="primary"
            icon="send"
            disabled={!complete || sent}
            onClick={submit}
          >
            Envoyer
          </Button>
        </div>
      )}
    </div>
  );
}

/** Rapport d'une exécution de fond (`cadence__run_report`). */
function ReportLine({ item }: { item: Item<"report"> }) {
  return (
    <div
      className={cn(
        "flex max-w-[720px] items-start gap-2 rounded-field border px-3 py-2 text-body-sm leading-[1.55]",
        item.final ? "border-green-line bg-green-d text-tx" : "border-line bg-p2 text-tx2",
      )}
    >
      <span className={cn("mt-px flex", item.final ? "text-green" : "text-tx3")}>
        <Icon name={item.final ? "check" : "clock"} size={14} />
      </span>
      <span className="min-w-0 grow wrap-anywhere whitespace-pre-wrap">{item.text}</span>
      {item.progress !== null && !item.final && (
        <span className="mono text-tiny text-tx3">{item.progress} %</span>
      )}
    </div>
  );
}

const decisionLabel: Record<ApprovalDecision, string> = {
  allow: "Autorisé",
  "allow-session": "Autorisé pour la session",
  deny: "Refusé",
};

/** Demande d'approbation de la CLI : ce qu'elle veut faire, et trois réponses. */
export function ApprovalCard({
  item,
  onApprove,
}: {
  item: Item<"approval">;
  onApprove?: ChatThreadProps["onApprove"];
}) {
  const command =
    item.detail && typeof item.detail === "object" && "command" in item.detail
      ? String((item.detail as { command: unknown }).command)
      : null;
  const answered = item.decision !== null;
  return (
    <div
      className={cn(
        "flex max-w-[720px] flex-col gap-2.5 rounded-field border px-3.5 py-3",
        answered ? "border-line bg-p2" : "border-amber-line bg-amber-d",
      )}
    >
      <div className="flex items-center gap-2 text-body-sm">
        <span className={cn("flex", answered ? "text-tx3" : "text-amber")}>
          <Icon
            name={
              item.approval === "command"
                ? "bolt"
                : item.approval === "file-change"
                  ? "pen"
                  : "wrench"
            }
            size={14}
          />
        </span>
        <span className="font-medium">
          Autoriser{" "}
          {item.approval === "command"
            ? "cette commande"
            : item.approval === "file-change"
              ? "cette modification"
              : "cet outil"}{" "}
          ?
        </span>
        <div className="grow" />
        {answered && (
          <Badge tone={item.decision === "deny" ? "red" : "green"}>
            {decisionLabel[item.decision!]}
            {item.decision === "allow-session" && item.scope ? ` · ${item.scope}` : ""}
          </Badge>
        )}
      </div>
      <span className="mono text-small break-all text-tx2">{command ?? item.title}</span>
      {!answered && (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="primary"
            size="sm"
            icon="check"
            onClick={() => onApprove?.(item.key, "allow")}
          >
            Autoriser
          </Button>
          <Button
            variant="secondary"
            size="sm"
            title={
              item.scope
                ? `Plus de demande pour les ${item.scope} dans cette conversation`
                : "Plus de demande pour ce type d'action dans cette conversation"
            }
            onClick={() => onApprove?.(item.key, "allow-session")}
          >
            Autoriser pour la session
          </Button>
          <Button
            variant="ghost"
            size="sm"
            icon="close"
            onClick={() => onApprove?.(item.key, "deny")}
          >
            Refuser
          </Button>
          {item.scope && (
            <span className="text-caption text-tx3">Pour la session : {item.scope}</span>
          )}
        </div>
      )}
    </div>
  );
}
