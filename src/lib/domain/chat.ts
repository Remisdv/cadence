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
      decision: ApprovalDecision | null;
    }
  | { kind: "error"; key: string; error: AiErrorKind; message: string }
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
        decision: null,
      });
    case "approval-answered": {
      const next = replace(items, event.request, "approval", (i) =>
        i.kind === "approval" ? { ...i, decision: event.decision } : i,
      );
      return next ? { ...thread, items: next } : thread;
    }
    case "question":
      return push({ kind: "assistant", key, text: event.text, streaming: false });
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
