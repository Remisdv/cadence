/**
 * Chat du mode mock : deux conversations d'exemple, et une IA simulée qui répond mot à mot
 * par `chat://event`. Message contenant « approbation » : carte d'approbation ; « long » :
 * réponse lente, interruptible ; « erreur » : tour en échec (connexion expirée).
 */
import type {
  AiEvent,
  AiId,
  ApprovalDecision,
  ChatEventPayload,
  ChatSessionDetail,
  ChatSessionRow,
  ChatState,
  ChatStatePayload,
} from "@/lib/types";
import { mockBus } from "./bus";

const at = (minutesAgo: number) => new Date(Date.now() - minutesAgo * 60_000).toISOString();

function session(id: string, title: string, minutesAgo: number, origin = "user"): ChatSessionRow {
  return {
    id,
    title,
    ai: "claude-code",
    model: "claude-opus-5",
    origin,
    mode: "chat",
    state: "idle",
    createdAt: at(minutesAgo + 5),
    updatedAt: at(minutesAgo),
  };
}

function detail(row: ChatSessionRow, events: AiEvent[]): ChatSessionDetail {
  return {
    session: row,
    events: events.map((event, i) => ({ seq: i + 1, turn: 1, at: row.updatedAt, event })),
  };
}

export function mockChats(empty: boolean): Map<string, ChatSessionDetail> {
  const chats = new Map<string, ChatSessionDetail>();
  if (empty) return chats;
  chats.set(
    "c1",
    detail(session("c1", "Où en est la version 0.4 ?", 12), [
      { type: "user-message", text: "Où en est la version 0.4 ?" },
      {
        type: "tool-started",
        call: "t1",
        source: { kind: "cadence", plugin: "gestion-projet", tool: "version_get" },
        name: "mcp__cadence__gestion-projet__version_get",
        input: { title: "0.4.0" },
      },
      {
        type: "tool-completed",
        call: "t1",
        ok: true,
        summary: "0.4.0 : 9 issues sur 14 fermées",
        output: null,
      },
      {
        type: "message-completed",
        message: "m1:0",
        text: "La **0.4.0** avance bien : 9 issues sur 14 sont fermées. Restent deux bugs et la migration du jeton GitHub. [Ouvrir la version](cadence:gestion-projet/versions?title=0.4.0)",
      },
      { type: "usage", inputTokens: 18_400, outputTokens: 210, costUsd: 0.04 },
      { type: "turn-completed", ok: true, interrupted: false, error: null, durationMs: 6400 },
    ]),
  );
  chats.set(
    "c2",
    detail(session("c2", "Fiche de révision : chapitre 3", 180, "action:documents/fiche"), [
      { type: "user-message", text: "Fais-moi une fiche de révision du chapitre 3." },
      {
        type: "message-completed",
        message: "m2:0",
        text: "Voici la fiche du **chapitre 3 — Thermodynamique** : premier principe, enthalpie, cycles de Carnot.",
      },
      { type: "turn-completed", ok: true, interrupted: false, error: null, durationMs: 9100 },
    ]),
  );
  return chats;
}

type Chats = Map<string, ChatSessionDetail>;

function setState(chats: Chats, id: string, state: ChatState) {
  const chat = chats.get(id);
  if (!chat) return;
  chat.session = { ...chat.session, state, updatedAt: new Date().toISOString() };
  mockBus.emit<ChatStatePayload>("chat://state", { session: id, state });
}

function record(chats: Chats, id: string, event: AiEvent) {
  const chat = chats.get(id);
  if (!chat) return;
  const turn = chat.events.filter((e) => e.event.type === "user-message").length;
  const row = { seq: chat.events.length + 1, turn, at: new Date().toISOString(), event };
  chat.events = [...chat.events, row];
  mockBus.emit<ChatEventPayload>("chat://event", { session: id, seq: row.seq, turn, event });
}

function delta(chats: Chats, id: string, message: string, text: string) {
  const turn = chats.get(id)?.events.filter((e) => e.event.type === "user-message").length ?? 0;
  mockBus.emit<ChatEventPayload>("chat://event", {
    session: id,
    seq: null,
    turn,
    event: { type: "message-delta", message, text },
  });
}

const pendingApprovals = new Map<string, (decision: ApprovalDecision) => void>();
const interrupts = new Set<string>();

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Réponse simulée d'un tour. */
export async function simulateTurn(chats: Chats, id: string, text: string) {
  const message = `mock-${id}-${Date.now()}:0`;
  const finish = (ok: boolean, interrupted = false, error: string | null = null) => {
    record(chats, id, { type: "usage", inputTokens: 12_000, outputTokens: 140, costUsd: 0.02 });
    record(chats, id, { type: "turn-completed", ok, interrupted, error, durationMs: 2300 });
    setState(chats, id, interrupted ? "interrupted" : ok ? "idle" : "failed");
  };
  setState(chats, id, "running");
  await sleep(400);

  if (text.includes("erreur")) {
    record(chats, id, {
      type: "error",
      kind: "auth",
      message: "La connexion de Claude Code a expiré : lancez `claude auth login`.",
    });
    return finish(false, false, "Connexion expirée.");
  }

  if (text.includes("approbation")) {
    const request = `perm-${Date.now()}`;
    record(chats, id, {
      type: "approval-requested",
      request,
      kind: "command",
      title: "Bash : pnpm test",
      detail: { command: "pnpm test" },
    });
    setState(chats, id, "waiting-approval");
    const decision = await new Promise<ApprovalDecision>((resolve) =>
      pendingApprovals.set(request, resolve),
    );
    record(chats, id, { type: "approval-answered", request, decision });
    setState(chats, id, "running");
    if (decision !== "deny") {
      record(chats, id, {
        type: "command",
        call: "c1",
        command: "pnpm test",
        exitCode: null,
        output: null,
      });
      await sleep(500);
      record(chats, id, {
        type: "command",
        call: "c1",
        command: "pnpm test",
        exitCode: 0,
        output: "41 tests passés",
      });
    }
  }

  const reply = text.includes("long")
    ? Array.from({ length: 60 }, (_, i) => `Vers ${i + 1} sur la mer qui roule.\n`).join("")
    : `Réponse simulée à « ${text.slice(0, 80)} ». En mode mock, aucune IA n'est appelée : lancez l'app avec Claude Code installé pour une vraie conversation.`;
  let sent = "";
  for (const word of reply.split(/(?<= )/)) {
    if (interrupts.delete(id)) {
      record(chats, id, { type: "message-completed", message, text: sent });
      return finish(false, true);
    }
    sent += word;
    delta(chats, id, message, word);
    await sleep(text.includes("long") ? 60 : 25);
  }
  record(chats, id, { type: "message-completed", message, text: reply });
  finish(true);
}

export function answerApproval(request: string, decision: ApprovalDecision): boolean {
  const resolve = pendingApprovals.get(request);
  if (!resolve) return false;
  pendingApprovals.delete(request);
  resolve(decision);
  return true;
}

export function requestInterrupt(id: string) {
  interrupts.add(id);
}

export function newChat(chats: Chats, text: string, ai: AiId = "claude-code"): ChatSessionRow {
  const line =
    text
      .split("\n")
      .find((l) => l.trim())
      ?.trim() ?? "Conversation";
  const row = session(
    `c${Date.now().toString(36)}`,
    line.length > 60 ? `${line.slice(0, 59)}…` : line,
    0,
  );
  row.ai = ai;
  row.model = ai === "claude-code" ? row.model : null;
  chats.set(row.id, { session: row, events: [] });
  return row;
}

export { record as recordMockChatEvent };
