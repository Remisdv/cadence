/**
 * Fil d'une conversation : les `AiEvent` enregistrés (et les deltas en cours) deviennent des
 * éléments affichables — bulles, cartes d'outil, lignes de console, approbations, fins de tour.
 */
import type {
  AiErrorKind,
  AiEvent,
  ApprovalDecision,
  ApprovalKind,
  ChatEventRow,
  ChatGroup,
  ChatSessionRow,
  ChatState,
  AiQuestion,
  QuestionAnswer,
  FileChange,
  PlanItem,
  ToolSource,
} from "@/lib/types";

export type ChatItem =
  | { kind: "user"; key: string; text: string }
  | { kind: "assistant"; key: string; text: string; streaming: boolean }
  | { kind: "reasoning"; key: string; text: string }
  | {
      kind: "tool";
      key: string;
      name: string;
      source: ToolSource;
      input: unknown;
      status: "running" | "ok" | "error";
      summary: string | null;
    }
  | {
      kind: "command";
      key: string;
      command: string;
      output: string | null;
      exitCode: number | null;
      running: boolean;
    }
  | { kind: "file"; key: string; path: string; change: FileChange }
  | { kind: "plan"; key: string; items: PlanItem[] }
  | {
      kind: "approval";
      key: string;
      approval: ApprovalKind;
      title: string;
      detail: unknown;
      /** Outil de la CLI (`Bash`, `mcp__cadence__<plugin>__<outil>`…), quand elle le donne. */
      tool: string | null;
      /** Ce que couvre « Autoriser pour la session » (« commandes git, python »). */
      scope: string | null;
      decision: ApprovalDecision | null;
    }
  | { kind: "error"; key: string; error: AiErrorKind; message: string }
  | {
      kind: "question";
      key: string;
      text: string;
      /** Questions posées (au moins une : les anciennes questions en donnent une seule). */
      questions: AiQuestion[];
      /** Réponse lisible, une fois répondu. */
      answer: string | null;
      answers: QuestionAnswer[];
    }
  | { kind: "report"; key: string; text: string; progress: number | null; final: boolean }
  | {
      kind: "turn";
      key: string;
      ok: boolean;
      interrupted: boolean;
      error: string | null;
      durationMs: number;
      costUsd: number | null;
    };

export interface ChatThread {
  items: ChatItem[];
  /** Coût du tour en cours, reporté sur sa fin de tour. */
  pendingCost: number | null;
}

export const emptyThread: ChatThread = { items: [], pendingCost: null };

function replace(
  items: ChatItem[],
  key: string,
  kind: ChatItem["kind"],
  update: (item: ChatItem) => ChatItem,
) {
  const index = items.findIndex((i) => i.key === key && i.kind === kind);
  if (index < 0) return null;
  const next = items.slice();
  next[index] = update(items[index]!);
  return next;
}

/** Applique un événement ; `key` identifie l'élément quand l'événement n'en porte pas. */
export function applyEvent(thread: ChatThread, event: AiEvent, key: string): ChatThread {
  const { items } = thread;
  const push = (item: ChatItem): ChatThread => ({ ...thread, items: [...items, item] });
  switch (event.type) {
    case "user-message":
      return push({ kind: "user", key, text: event.text });
    case "message-delta": {
      const next = replace(items, event.message, "assistant", (i) =>
        i.kind === "assistant" ? { ...i, text: i.text + event.text } : i,
      );
      return next
        ? { ...thread, items: next }
        : push({ kind: "assistant", key: event.message, text: event.text, streaming: true });
    }
    case "message-completed": {
      const done: ChatItem = {
        kind: "assistant",
        key: event.message,
        text: event.text,
        streaming: false,
      };
      const next = replace(items, event.message, "assistant", () => done);
      return next ? { ...thread, items: next } : push(done);
    }
    case "reasoning":
      return push({ kind: "reasoning", key, text: event.text });
    case "tool-started":
      return push({
        kind: "tool",
        key: event.call,
        name: event.name,
        source: event.source,
        input: event.input,
        status: "running",
        summary: null,
      });
    case "tool-completed": {
      const next = replace(items, event.call, "tool", (i) =>
        i.kind === "tool" ? { ...i, status: event.ok ? "ok" : "error", summary: event.summary } : i,
      );
      return next ? { ...thread, items: next } : thread;
    }
    case "command": {
      const item: ChatItem = {
        kind: "command",
        key: event.call,
        command: event.command,
        output: event.output,
        exitCode: event.exitCode,
        running: event.output === null,
      };
      const next = replace(items, event.call, "command", () => item);
      return next ? { ...thread, items: next } : push(item);
    }
    case "file-changed":
      return push({ kind: "file", key, path: event.path, change: event.change });
    case "plan":
      return push({ kind: "plan", key, items: event.items });
    case "approval-requested":
      return push({
        kind: "approval",
        key: event.request,
        approval: event.kind,
        title: event.title,
        detail: event.detail,
        tool: event.tool ?? null,
        scope: event.scope ?? null,
        decision: null,
      });
    case "approval-answered": {
      const next = replace(items, event.request, "approval", (i) =>
        i.kind === "approval" ? { ...i, decision: event.decision } : i,
      );
      return next ? { ...thread, items: next } : thread;
    }
    case "question":
      return push({
        kind: "question",
        key: event.request,
        text: event.text,
        questions: questionsOf(event),
        answer: null,
        answers: [],
      });
    case "question-answered": {
      const next = replace(items, event.request, "question", (i) =>
        i.kind === "question" ? { ...i, answer: event.answer, answers: event.answers ?? [] } : i,
      );
      return next ? { ...thread, items: next } : thread;
    }
    case "report":
      return push({
        kind: "report",
        key,
        text: event.text,
        progress: event.progress,
        final: event.isFinal,
      });
    case "usage":
      return { ...thread, pendingCost: event.costUsd };
    case "turn-completed": {
      // Un message encore en cours à la fin du tour (interruption) ne bouge plus.
      const settled = items.map((i) =>
        i.kind === "assistant" && i.streaming ? { ...i, streaming: false } : i,
      );
      return {
        items: [
          ...settled,
          {
            kind: "turn",
            key,
            ok: event.ok,
            interrupted: event.interrupted,
            error: event.error,
            durationMs: event.durationMs,
            costUsd: thread.pendingCost,
          },
        ],
        pendingCost: null,
      };
    }
    case "error":
      return push({ kind: "error", key, error: event.kind, message: event.message });
    case "session-started":
      return thread;
  }
}

/** Fil d'une conversation enregistrée, plus les textes en cours de diffusion. */
export function buildThread(
  events: ChatEventRow[],
  drafts: Record<string, string> = {},
): ChatItem[] {
  let thread = events.reduce((t, row) => applyEvent(t, row.event, `e${row.seq}`), emptyThread);
  for (const [message, text] of Object.entries(drafts)) {
    thread = applyEvent(thread, { type: "message-delta", message, text: "" }, message);
    thread = {
      ...thread,
      items: thread.items.map((i) =>
        i.kind === "assistant" && i.key === message && i.streaming ? { ...i, text } : i,
      ),
    };
  }
  return thread.items;
}

/** Libellé court d'un outil : `Plugin › outil` pour un outil de plugin, sinon son nom. */
export function toolLabel(item: Extract<ChatItem, { kind: "tool" }>): string {
  return item.source.kind === "cadence" ? `${item.source.plugin} › ${item.source.tool}` : item.name;
}

/** Un tour affiché : le message de l'utilisateur, puis tout ce que l'IA a produit. */
export interface ChatTurn {
  key: string;
  user: Extract<ChatItem, { kind: "user" }> | null;
  reply: ChatItem[];
}

export function groupTurns(items: ChatItem[]): ChatTurn[] {
  const turns: ChatTurn[] = [];
  for (const item of items) {
    if (item.kind === "user") turns.push({ key: item.key, user: item, reply: [] });
    else if (turns.length === 0) turns.push({ key: item.key, user: null, reply: [item] });
    else turns.at(-1)!.reply.push(item);
  }
  return turns;
}

/**
 * Activité d'un tour : commandes shell, appels d'outils (lectures de fichiers comprises) et
 * fichiers modifiés. Masquée par l'interrupteur « Commandes », elle se résume en une ligne.
 */
export function isActivity(item: ChatItem): boolean {
  return item.kind === "tool" || item.kind === "command" || item.kind === "file";
}

const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;

/** `6 commandes · 3 outils · 2 fichiers modifiés` ; chaîne vide sans activité. */
export function activitySummary(items: ChatItem[]): string {
  const commands = items.filter((i) => i.kind === "command").length;
  const tools = items.filter((i) => i.kind === "tool").length;
  const files = new Set(items.flatMap((i) => (i.kind === "file" ? [i.path] : []))).size;
  return [
    commands > 0 ? plural(commands, "commande", "commandes") : null,
    tools > 0 ? plural(tools, "outil", "outils") : null,
    files > 0 ? plural(files, "fichier modifié", "fichiers modifiés") : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

/** `12 s`, `2 min 08 s`. */
export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s} s`;
  return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")} s`;
}

/** Arguments d'un outil sur une ligne : `dépôts=tous assignée=@moi`. */
export function compactArgs(input: unknown): string {
  if (!input || typeof input !== "object" || Array.isArray(input)) return "";
  return Object.entries(input as Record<string, unknown>)
    .map(([k, v]) => `${k}=${typeof v === "string" ? v : JSON.stringify(v)}`)
    .join(" ");
}

/** Lien `cadence:<plugin>/<vue>?<paramètres>` (plan § 3.4) ; `null` pour un autre lien. */
export function parseCadenceLink(
  href: string,
): { plugin: string; view: string; params: Record<string, string> } | null {
  const m = /^cadence:([a-z0-9-]+)\/([a-z0-9-]+)(?:\?(.*))?$/.exec(href.trim());
  if (!m) return null;
  return {
    plugin: m[1]!,
    view: m[2]!,
    params: Object.fromEntries(new URLSearchParams(m[3] ?? "")),
  };
}

const DAY = 86_400_000;
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** Groupe de la colonne des conversations. */
export function conversationGroup(iso: string, now = new Date()): string {
  const days = Math.floor((startOfDay(now) - startOfDay(new Date(iso))) / DAY);
  if (days <= 0) return "Aujourd'hui";
  if (days === 1) return "Hier";
  if (days < 7) return "Cette semaine";
  return "Plus ancien";
}

/** Heure aujourd'hui (`11:20`), jour cette semaine (`mar.`), date sinon (`3 oct.`). */
export function conversationTime(iso: string, now = new Date()): string {
  const date = new Date(iso);
  const group = conversationGroup(iso, now);
  if (group === "Aujourd'hui" || group === "Hier")
    return date.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  if (group === "Cette semaine") return date.toLocaleDateString("fr-FR", { weekday: "short" });
  return date.toLocaleDateString("fr-FR", { day: "numeric", month: "short" });
}

/** Plugin d'origine d'une conversation ouverte par une action (`action:<plugin>/<action>`). */
export function originPlugin(origin: string): string | null {
  return /^action:([a-z0-9-]+)\//.exec(origin)?.[1] ?? null;
}

/**
 * Plugin de la puce de contexte : celui d'une action (`action:<plugin>/…`) ou de la barre d'où
 * la conversation a été ouverte (`plugin:<plugin>`).
 */
export function contextPlugin(origin: string): string | null {
  return originPlugin(origin) ?? /^plugin:([a-z0-9-]+)$/.exec(origin)?.[1] ?? null;
}

/** Conversations d'un plugin montrées dans sa barre : les siennes, hors groupes cachés. */
export function pluginConversations<T extends Pick<ChatSessionRow, "origin" | "hidden">>(
  rows: T[],
  plugin: string,
  max = 6,
): T[] {
  return rows.filter((row) => !row.hidden && contextPlugin(row.origin) === plugin).slice(0, max);
}

/** Ce qui a ouvert une conversation d'un plugin : le libellé de son action, sinon « Conversation libre ». */
export function conversationOriginLabel(
  origin: string,
  plugin: { actions: { id: string; label: string }[] },
): string {
  const action = /^action:[a-z0-9-]+\/(.+)$/.exec(origin)?.[1];
  if (!action) return "Conversation libre";
  return plugin.actions.find((a) => a.id === action)?.label ?? action;
}

/** Conversations du chat global : sans les conversations cachées (rattachées à un groupe). */
export function globalConversations<T extends Pick<ChatSessionRow, "hidden">>(rows: T[]): T[] {
  return rows.filter((row) => !row.hidden);
}

const DAY_GROUPS = ["Aujourd'hui", "Hier", "Cette semaine", "Plus ancien"];

/** Section de la colonne des conversations ; `attached` : conversations rangées dans un groupe. */
export interface ChatBarSection<T> {
  title: string;
  rows: T[];
  attached?: boolean;
}

type BarRow = Pick<
  ChatSessionRow,
  "id" | "title" | "hidden" | "group" | "mode" | "state" | "updatedAt"
>;

/**
 * Sections de la colonne du chat. Les conversations rattachées à un groupe (un sprint) restent
 * hors de la liste, sauf celle ouverte, celles qui attendent votre réponse et, pendant une
 * recherche, celles qui correspondent (titre ou nom du groupe) : section « Rattachées », en tête.
 */
export function chatBarSections<T extends BarRow>(
  rows: T[],
  query: string,
  active: string | null,
  now = new Date(),
): ChatBarSection<T>[] {
  const q = query.trim().toLowerCase();
  const matches = (s: T) =>
    !q || s.title.toLowerCase().includes(q) || !!s.group?.label.toLowerCase().includes(q);
  const attached = rows.filter(
    (s) => s.hidden && (q ? matches(s) : s.id === active || awaitsUser(s.state)),
  );
  const listed = globalConversations(rows).filter(matches);
  const chats = listed.filter((s) => s.mode !== "background");
  return [
    { title: "Rattachées", rows: attached, attached: true },
    // Exécutions de fond des actions : leur propre groupe (panneau Activité).
    { title: "En fond", rows: listed.filter((s) => s.mode === "background") },
    ...DAY_GROUPS.map((title) => ({
      title,
      rows: chats.filter((s) => conversationGroup(s.updatedAt, now) === title),
    })),
  ].filter((section) => section.rows.length > 0);
}

/** Groupe d'une conversation vu par son plugin : l'id sans le préfixe `<plugin>:` de l'hôte. */
export function localGroup(plugin: string, group: ChatGroup | null): ChatGroup | null {
  if (!group?.id.startsWith(`${plugin}:`)) return null;
  return { id: group.id.slice(plugin.length + 1), label: group.label };
}

/** Lien de retour d'une conversation de groupe vers la vue de son plugin. */
export function groupReturnLabel(group: ChatGroup, pluginName: string): string {
  return /^sprint[:/]/.test(group.id) ? "Retour au sprint" : `Retour à ${pluginName}`;
}

/**
 * Questions d'un événement `question` : les questions détaillées, ou, pour l'historique d'avant
 * les questions multiples, une seule question tirée de `text` et `options`.
 */
export function questionsOf(event: {
  text: string;
  options: string[];
  questions?: AiQuestion[];
}): AiQuestion[] {
  if (event.questions && event.questions.length > 0) return event.questions;
  return [
    {
      id: event.text,
      header: null,
      question: event.text,
      options: event.options.map((label) => ({ label, description: null })),
      multiSelect: false,
      other: true,
    },
  ];
}

/** Réponse d'une question telle que la carte la compose : choix cochés, puis texte « Autre ». */
export function composeAnswer(question: AiQuestion, picked: string[], other: string): string[] {
  const chosen = question.options.map((o) => o.label).filter((label) => picked.includes(label));
  const free = question.other ? other.trim() : "";
  return [...(question.multiSelect ? chosen : chosen.slice(0, 1)), ...(free ? [free] : [])];
}

/**
 * La conversation attend l'utilisateur : un accord ou une question à choix en plein tour
 * (`waiting-approval`), ou une réponse à la question qui a fini le tour (`waiting-reply`,
 * recette du 21/10, N10).
 */
export function awaitsUser(state: ChatState): boolean {
  return state === "waiting-approval" || state === "waiting-reply";
}

/** Libellé court d'un état qui attend l'utilisateur (infobulles, pastilles). */
export function waitingLabel(state: ChatState): string | null {
  if (state === "waiting-approval") return "Attend votre accord";
  if (state === "waiting-reply") return "En attente de votre réponse";
  return null;
}

/**
 * Outil d'un plugin servi par la passerelle, tel que la CLI le nomme dans une demande
 * d'accord : `mcp__cadence__<plugin>__<outil>` (Claude Code, Codex), `cadence_<plugin>__<outil>`
 * (OpenCode). `null` pour un outil natif ou un outil du cœur (`cadence__*`).
 */
export function pluginToolOf(tool: string | null): { plugin: string; tool: string } | null {
  if (!tool) return null;
  const exposed = tool.startsWith("mcp__cadence__")
    ? tool.slice("mcp__cadence__".length)
    : tool.startsWith("cadence_")
      ? tool.slice("cadence_".length)
      : null;
  const at = exposed?.indexOf("__") ?? -1;
  if (!exposed || at <= 0 || at + 2 >= exposed.length) return null;
  const plugin = exposed.slice(0, at);
  return plugin === "cadence" ? null : { plugin, tool: exposed.slice(at + 2) };
}
