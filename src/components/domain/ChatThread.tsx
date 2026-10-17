/**
 * Fil d'une conversation (V2-Chat.dc.html) : bulle de l'utilisateur à droite ; réponse de l'IA
 * avec sa tuile, son en-tête (modèle, outils, durée), les cartes d'outil repliables, les
 * commandes shell, les demandes d'approbation, le texte en Markdown et les erreurs.
 *
 * Les éléments viennent de `buildThread` (`lib/domain/chat.ts`) ; le composant ne lit aucune
 * donnée lui-même.
 */
import { useState, type ReactNode } from "react";
import { Icon } from "@/components/icons/Icon";
import { Markdown } from "@/components/domain/Markdown";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import {
  compactArgs,
  formatDuration,
  groupTurns,
  type ChatItem,
  type ChatTurn,
} from "@/lib/domain/chat";
import type { ApprovalDecision } from "@/lib/types";

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
  /** Réponse à une question de l'IA (Codex, OpenCode). */
  onAnswer?: (request: string, answer: string) => void;
  /** Clic sur un lien du texte ; `true` s'il est traité (liens `cadence:`). */
  onLink?: (href: string) => boolean;
}

export function ChatThread({
  items,
  ai,
  model,
  running,
  onApprove,
  onAnswer,
  onLink,
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
}: {
  turn: ChatTurn;
  ai: string;
  model?: string | null;
  running: boolean;
  onApprove?: ChatThreadProps["onApprove"];
  onAnswer?: ChatThreadProps["onAnswer"];
  onLink?: ChatThreadProps["onLink"];
}) {
  const visible = turn.reply.filter((i) => i.kind !== "turn");
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
            {segments(visible).map((segment) =>
              Array.isArray(segment) ? (
                <div key={segment[0]!.key} className="flex flex-col gap-1.5">
                  {segment.map((item) =>
                    item.kind === "tool" ? (
                      <ToolCard key={item.key} item={item} />
                    ) : (
                      <CommandCard key={item.key} item={item as Item<"command">} />
                    ),
                  )}
                </div>
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

/** Regroupe les cartes d'outil et de commande consécutives (6px entre elles). */
function segments(items: ChatItem[]): (ChatItem | ChatItem[])[] {
  const out: (ChatItem | ChatItem[])[] = [];
  for (const item of items) {
    const tool = item.kind === "tool" || item.kind === "command";
    const last = out.at(-1);
    if (tool && Array.isArray(last)) last.push(item);
    else out.push(tool ? [item] : item);
  }
  return out;
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
      <div className="max-w-[520px] rounded-card rounded-br-[4px] border border-line2 bg-p3 px-3.5 py-[11px] text-body leading-[1.6] whitespace-pre-wrap">
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
        <div className="max-w-[720px]">
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
        <details className="max-w-[720px] text-body-sm text-tx3">
          <summary className="cursor-pointer list-none">Réflexion</summary>
          <p className="mt-1.5 leading-[1.6] whitespace-pre-wrap">{item.text}</p>
        </details>
      );
    case "file":
      return (
        <span className="flex items-center gap-2 text-small text-tx2">
          <span className="flex text-tx3">
            <Icon name="file" size={13} />
          </span>
          {item.change === "added"
            ? "Fichier créé"
            : item.change === "deleted"
              ? "Fichier supprimé"
              : "Fichier modifié"}
          <span className="truncate mono text-tx3">{item.path}</span>
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
          <span className="mt-px flex text-red">
            <Icon name="warning" size={14} />
          </span>
          <Markdown source={item.message} className="text-body-sm leading-[1.55] text-tx2" />
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
      <pre className="overflow-x-auto mono text-tiny leading-[1.6] whitespace-pre-wrap text-tx3">
        {JSON.stringify(item.input, null, 2)}
      </pre>
    </ToolShell>
  );
}

/** Commande shell de la CLI : la commande en résumé, sa sortie dépliée. */
export function CommandCard({ item }: { item: Item<"command"> }) {
  const status = item.running ? "running" : item.exitCode && item.exitCode !== 0 ? "error" : "ok";
  return (
    <ToolShell
      status={status}
      icon={<Icon name="bolt" size={14} />}
      name={item.command}
      result={null}
    >
      {item.output && (
        <pre className="max-h-[260px] overflow-auto rounded-control bg-console px-2.5 py-2 mono text-tiny leading-[1.6] whitespace-pre-wrap text-tx2">
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
    <details className="group rounded-field border border-line bg-p2">
      <summary className="flex h-9 cursor-pointer list-none items-center gap-[9px] px-3">
        {status === "running" ? (
          <Spinner size={14} />
        ) : (
          <span className={cn("flex", status === "ok" ? "text-green" : "text-red")}>
            <Icon name={status === "ok" ? "check" : "close"} size={14} />
          </span>
        )}
        <span className="flex text-tx3">{icon}</span>
        <span className="min-w-0 shrink-0 truncate mono text-small text-tx">{name}</span>
        {args && <span className="min-w-0 truncate mono text-caption text-tx3">{args}</span>}
        <div className="grow" />
        {result && <span className="max-w-[260px] truncate text-caption text-tx2">{result}</span>}
        <span className="flex text-tx3 transition-transform group-open:rotate-180">
          <Icon name="chevronDown" size={14} />
        </span>
      </summary>
      {children && (
        <div className="flex flex-col gap-[5px] pt-1 pr-3 pb-2.5 pl-[37px]">{children}</div>
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
          <span className={cn(step.status === "completed" ? "text-tx3 line-through" : "text-tx2")}>
            {step.text}
          </span>
        </span>
      ))}
    </div>
  );
}

/** Question de l'IA : les choix proposés, ou une réponse libre. */
export function QuestionCard({
  item,
  onAnswer,
}: {
  item: Item<"question">;
  onAnswer?: ChatThreadProps["onAnswer"];
}) {
  const [text, setText] = useState("");
  const answered = item.answer !== null;
  return (
    <div
      className={cn(
        "flex max-w-[720px] flex-col gap-2.5 rounded-field border px-3.5 py-3",
        answered ? "border-line bg-p2" : "border-blue-line bg-blue-d",
      )}
    >
      <div className="flex items-start gap-2 text-body-sm">
        <span className={cn("mt-px flex", answered ? "text-tx3" : "text-blue")}>
          <Icon name="chat" size={14} />
        </span>
        <span className="grow leading-[1.55] font-medium whitespace-pre-wrap">{item.text}</span>
        {answered && <Badge tone="green">Répondu</Badge>}
      </div>
      {answered ? (
        <span className="text-small text-tx2">{item.answer}</span>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          {item.options.map((option) => (
            <Button key={option} size="sm" onClick={() => onAnswer?.(item.key, option)}>
              {option}
            </Button>
          ))}
          <form
            className="flex min-w-[240px] grow items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (text.trim()) onAnswer?.(item.key, text.trim());
            }}
          >
            <input
              aria-label="Votre réponse"
              placeholder={item.options.length > 0 ? "Autre réponse…" : "Votre réponse…"}
              value={text}
              onChange={(e) => setText(e.target.value)}
              className="h-7 min-w-0 grow rounded-control border border-line2 bg-panel px-2.5 text-body-sm text-tx outline-none focus:border-blue-line"
            />
            <Button type="submit" size="sm" variant="primary" disabled={!text.trim()}>
              Répondre
            </Button>
          </form>
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
      <span className="grow whitespace-pre-wrap">{item.text}</span>
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
        </div>
      )}
    </div>
  );
}
