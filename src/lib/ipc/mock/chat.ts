/**
 * Chat du mode mock : deux conversations d'exemple, et une IA simulée qui répond mot à mot
 * par `chat://event`. Message contenant « approbation » : carte d'approbation (sauf en mode
 * Auto) ; « question » : deux questions à choix (dans tous les modes) ; « commandes » :
 * plusieurs commandes (dont une très longue), une lecture et deux fichiers modifiés ;
 * « long » : réponse lente, interruptible ; « erreur » : tour en échec (connexion expirée).
 */
import type {
  AiEvent,
  AiQuestion,
  AiId,
  ApprovalDecision,
  ChatEventPayload,
  ChatPermissions,
  ChatSessionDetail,
  ChatSessionRow,
  ChatState,
  ChatStatePayload,
  ConversationDetail,
  ConversationInfo,
  QuestionAnswer,
} from "@/lib/types";
import { globalConversations, localGroup, originPlugin } from "@/lib/domain/chat";
import { sdkState } from "@/plugin-host/conversations";
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
    permissions: "ask",
    state: "idle",
    createdAt: at(minutesAgo + 5),
    updatedAt: at(minutesAgo),
    group: null,
    hidden: false,
  };
}

/** Contexte d'action de chaque conversation ouverte par une action (le back le garde en base). */
export const mockContexts = new Map<string, Record<string, unknown>>();

/** Sprint de démonstration de Gestion de projet (`group` du contexte des actions). */
export const MOCK_SPRINT_GROUP = {
  id: "sprint:cadence-app/desktop:12",
  label: "Sprint 12 · cadence-app/desktop",
};

/**
 * Champ réservé `group` d'un contexte d'action, comme `action_group` côté Rust : id préfixé par
 * le plugin, `hidden` vrai par défaut. Lève une erreur lisible si le groupe est mal formé.
 */
export function mockActionGroup(
  plugin: string,
  context: Record<string, unknown> | null,
): { group: ChatSessionRow["group"]; hidden: boolean } {
  const raw = context?.group;
  if (raw === undefined || raw === null) return { group: null, hidden: false };
  const group = raw as { id?: unknown; label?: unknown; hidden?: unknown };
  if (typeof raw !== "object" || Array.isArray(raw))
    throw new Error("Contexte d'action : « group » doit être un objet { id, label, hidden }.");
  if (typeof group.id !== "string" || !/^[A-Za-z0-9:/._-]{1,120}$/.test(group.id))
    throw new Error("Contexte d'action : « group » a un id invalide.");
  if (group.hidden !== undefined && group.hidden !== null && typeof group.hidden !== "boolean")
    throw new Error("Contexte d'action : « group » a un « hidden » qui n'est pas un booléen.");
  const label =
    typeof group.label === "string" && group.label.trim() ? group.label.trim() : group.id;
  return {
    group: { id: `${plugin}:${group.id}`, label: label.slice(0, 120) },
    hidden: typeof group.hidden === "boolean" ? group.hidden : true,
  };
}

/**
 * `chat_sessions` : sans les cachées, sauf avec un filtre `group` explicite (id préfixé) ou
 * `all`.
 */
export function listMockChats(
  chats: Chats,
  group: string | null = null,
  all = false,
): ChatSessionRow[] {
  const rows = [...chats.values()].map((c) => c.session);
  const listed = group
    ? rows.filter((s) => s.group?.id === group)
    : all
      ? rows
      : globalConversations(rows);
  return listed.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

const ofPlugin = (plugin: string, row: ChatSessionRow) =>
  row.origin.startsWith(`action:${plugin}/`);

function conversationInfo(plugin: string, chat: ChatSessionDetail): ConversationInfo {
  const row = chat.session;
  const group = localGroup(plugin, row.group);
  const context = mockContexts.get(row.id);
  return {
    id: row.id,
    title: row.title,
    ai: row.ai,
    model: row.model,
    // Même règle que `sdk_state` côté Rust : `waiting-reply` reste invisible du SDK (API 1).
    ...sdkState(row.state),
    mode: row.mode,
    action: row.origin.slice(`action:${plugin}/`.length),
    ...(context ? { context } : {}),
    ...(group ? { group } : {}),
    createdAt: row.createdAt,
    updatedAt: row.updatedAt,
    turns: Math.max(0, ...chat.events.map((e) => e.turn)),
  };
}

/** `plugin_conversations` : conversations des actions du plugin (`group` : id vu par lui). */
export function mockConversations(
  chats: Chats,
  plugin: string,
  group: string | null,
): ConversationInfo[] {
  const rows = group
    ? listMockChats(chats, `${plugin}:${group}`)
    : listMockChats(chats).filter((row) => !row.hidden);
  return rows
    .filter((row) => ofPlugin(plugin, row))
    .map((row) => conversationInfo(plugin, chats.get(row.id)!));
}

/** `plugin_conversation_get` : messages complets, outils sans sorties ; un autre plugin : `null`. */
export function mockConversation(
  chats: Chats,
  plugin: string,
  id: string,
): ConversationDetail | null {
  const chat = chats.get(id);
  if (!chat || !ofPlugin(plugin, chat.session)) return null;
  const names = new Map<string, string>();
  const detail: ConversationDetail = {
    info: conversationInfo(plugin, chat),
    messages: [],
    tools: [],
  };
  for (const { event, at } of chat.events) {
    if (event.type === "user-message") detail.messages.push({ role: "user", text: event.text, at });
    else if (event.type === "message-completed" && event.text.trim())
      detail.messages.push({ role: "assistant", text: event.text, at });
    else if (event.type === "tool-started") names.set(event.call, event.name);
    else if (event.type === "tool-completed")
      detail.tools.push({ name: names.get(event.call) ?? "outil", ok: event.ok, at });
    else if (event.type === "command" && event.exitCode !== null)
      detail.tools.push({ name: "shell", ok: event.exitCode === 0, at });
    else if (event.type === "usage") {
      const usage = (detail.usage ??= { inputTokens: 0, outputTokens: 0 });
      usage.inputTokens += event.inputTokens;
      usage.outputTokens += event.outputTokens;
    }
  }
  return detail;
}

/** Charge de `chat://state`, avec le plugin et le groupe vu par lui (comme le back). */
export function statePayload(row: ChatSessionRow, state: ChatState): ChatStatePayload {
  const plugin = originPlugin(row.origin);
  const group = plugin ? localGroup(plugin, row.group) : null;
  return {
    session: row.id,
    state,
    ...(plugin ? { plugin } : {}),
    ...(group ? { group } : {}),
  };
}

function detail(row: ChatSessionRow, events: AiEvent[]): ChatSessionDetail {
  return {
    session: row,
    events: events.map((event, i) => ({ seq: i + 1, turn: 1, at: row.updatedAt, event })),
    worktrees: [],
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
  for (const [id, chat, context] of sprintChats()) {
    chats.set(id, chat);
    mockContexts.set(id, context);
  }
  return chats;
}

/**
 * Conversations du sprint de démonstration (« Développer le sprint », « Développer l'issue »),
 * rattachées au groupe et cachées du chat global : on les voit depuis la vue du sprint.
 */
function sprintChats(): [string, ChatSessionDetail, Record<string, unknown>][] {
  const repo = "cadence-app/desktop";
  const grouped = (row: ChatSessionRow, mode: ChatSessionRow["mode"] = "background") => ({
    ...row,
    mode,
    permissions: mode === "background" ? ("auto-edit" as const) : ("ask" as const),
    group: { id: `gestion-projet:${MOCK_SPRINT_GROUP.id}`, label: MOCK_SPRINT_GROUP.label },
    hidden: true,
  });
  const done = { type: "turn-completed", ok: true, interrupted: false, error: null } as const;
  const sprint = grouped(
    session("s12", "Développer le sprint 12", 95, "action:gestion-projet/developper-sprint"),
  );
  const issue = grouped(
    session("s12-214", "Développer l'issue #214", 40, "action:gestion-projet/developper-issue"),
    "chat",
  );
  const failed = {
    ...grouped(
      session("s12-217", "Développer l'issue #217", 25, "action:gestion-projet/developper-issue"),
    ),
    state: "failed" as const,
  };
  return [
    [
      "s12",
      detail(sprint, [
        { type: "user-message", text: "/cadence:gestion-projet-developper-sprint\n\nSprint 12" },
        {
          type: "message-completed",
          message: "s12:0",
          text: "Sprint 12 : 3 issues prêtes. Je lance #214 et #217 ; #219 attend sa description.",
        },
        { type: "usage", inputTokens: 21_000, outputTokens: 420, costUsd: 0.05 },
        { ...done, durationMs: 31_000 },
      ]),
      { type: "version", repo, version: "0.1.2", group: MOCK_SPRINT_GROUP },
    ],
    [
      "s12-214",
      detail(issue, [
        { type: "user-message", text: "/cadence:gestion-projet-developper-issue\n\nIssue #214" },
        {
          type: "command",
          call: "k1",
          command: "pnpm test",
          exitCode: 0,
          output: "41 tests passés",
        },
        {
          type: "message-completed",
          message: "s12-214:0",
          text: "L'issue **#214** est implémentée ; la PR #88 est ouverte et la CI passe.",
        },
        { type: "usage", inputTokens: 64_000, outputTokens: 2_100, costUsd: 0.21 },
        { ...done, durationMs: 182_000 },
      ]),
      { type: "issue", repo, issue: 214, group: MOCK_SPRINT_GROUP },
    ],
    [
      "s12-217",
      detail(failed, [
        { type: "user-message", text: "/cadence:gestion-projet-developper-issue\n\nIssue #217" },
        {
          type: "error",
          kind: "quota",
          message: "Quota de l'abonnement atteint : reprenez plus tard.",
        },
        {
          type: "turn-completed",
          ok: false,
          interrupted: false,
          error: "Quota atteint.",
          durationMs: 4_000,
        },
      ]),
      { type: "issue", repo, issue: 217, group: MOCK_SPRINT_GROUP },
    ],
  ];
}

type Chats = Map<string, ChatSessionDetail>;

function setState(chats: Chats, id: string, state: ChatState) {
  const chat = chats.get(id);
  if (!chat) return;
  chat.session = { ...chat.session, state, updatedAt: new Date().toISOString() };
  mockBus.emit<ChatStatePayload>("chat://state", statePayload(chat.session, state));
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

const pendingApprovals = new Map<
  string,
  { session: string; resolve: (decision: ApprovalDecision) => void }
>();
const interrupts = new Set<string>();

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Réponse simulée d'un tour. */
export async function simulateTurn(chats: Chats, id: string, text: string) {
  const message = `mock-${id}-${Date.now()}:0`;
  const finish = (ok: boolean, interrupted = false, error: string | null = null, asks = false) => {
    record(chats, id, { type: "usage", inputTokens: 12_000, outputTokens: 140, costUsd: 0.02 });
    record(chats, id, { type: "turn-completed", ok, interrupted, error, durationMs: 2300 });
    setState(
      chats,
      id,
      interrupted ? "interrupted" : !ok ? "failed" : asks ? "waiting-reply" : "idle",
    );
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

  if (text.includes("commandes")) await simulateCommands(chats, id);

  const mode = chats.get(id)?.session.permissions ?? "ask";
  if (text.includes("approbation") && mode === "full") {
    record(chats, id, {
      type: "command",
      call: "c1",
      command: "pnpm test",
      exitCode: 0,
      output: "41 tests passés (mode Auto : aucune demande)",
    });
  } else if (text.includes("approbation")) {
    const request = `perm-${Date.now()}`;
    record(chats, id, {
      type: "approval-requested",
      request,
      kind: "command",
      title: "Bash : pnpm test",
      detail: { command: "pnpm test" },
      tool: "Bash",
      scope: "commande pnpm",
    });
    setState(chats, id, "waiting-approval");
    const decision = await new Promise<ApprovalDecision>((resolve) =>
      pendingApprovals.set(request, { session: id, resolve }),
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

  // Questions à choix (comme `AskUserQuestion`) : posées dans tous les modes, « Auto » compris.
  let answered: string | null = null;
  if (text.includes("question")) {
    const request = `question-${Date.now()}`;
    const questions = MOCK_QUESTIONS;
    record(chats, id, {
      type: "question",
      request,
      text: questions.map((q) => q.question).join("\n"),
      options: questions[0]!.options.map((o) => o.label),
      questions,
    });
    setState(chats, id, "waiting-approval");
    const answers = await new Promise<QuestionAnswer[]>((resolve) =>
      pendingQuestions.set(request, { questions, resolve }),
    );
    answered = answers
      .map((a) => {
        const header = questions.find((q) => q.id === a.id)?.header;
        return `${header ? `${header} : ` : ""}${a.answers.join(", ")}`;
      })
      .join(" · ");
    record(chats, id, { type: "question-answered", request, answer: answered, answers });
    setState(chats, id, "running");
  }

  const reply = answered
    ? `Merci : ${answered}. En mode mock, aucune IA n'est appelée.`
    : text.includes("long")
      ? Array.from({ length: 60 }, (_, i) => `Vers ${i + 1} sur la mer qui roule.\n`).join("")
      : text.includes("fusion")
        ? "Les deux PR sont relues et vertes.\n\nJe les fusionne dans la release ?"
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
  // « En attente de votre réponse » (N10) : le tour se termine sur une question.
  finish(true, false, null, reply.trimEnd().endsWith("?"));
}

/** Commande du retour utilisateur qui sortait du cadre du fil. */
export const LONG_MOCK_COMMAND =
  'cd "C:/Users/lhuil/AppData/Roaming/app.cadence.desktop/repos/.worktrees/remisdv/go-alternance-crm-poc/extraction-issues" && ls -la && echo "--- TREE ---" && find . -path ./.git -prune -o -type f -print | head -100';

async function simulateCommands(chats: Chats, id: string) {
  const commands: [string, string, number][] = [
    [
      LONG_MOCK_COMMAND,
      [
        "total 48",
        "drwxr-xr-x 1 moi 197121    0 oct. 20 09:12 .",
        "-rw-r--r-- 1 moi 197121 2310 oct. 20 09:12 C:/Users/lhuil/AppData/Roaming/app.cadence.desktop/repos/.worktrees/remisdv/go-alternance-crm-poc/extraction-issues/README.md",
        "--- TREE ---",
        "./src/extraction/issues/github_issues_extractor_with_a_really_long_file_name_without_spaces.go",
        "./go.mod",
      ].join("\n"),
      0,
    ],
    ["git status --short", " M src/extraction/issues.go\n?? src/extraction/labels.go", 0],
    ["go test ./...", "--- FAIL: TestLabels (0.00s)\nFAIL", 1],
  ];
  for (const [i, [command, output, exitCode]] of commands.entries()) {
    const call = `cmd-${Date.now()}-${i}`;
    record(chats, id, { type: "command", call, command, exitCode: null, output: null });
    await sleep(250);
    record(chats, id, { type: "command", call, command, exitCode, output });
  }
  const call = `read-${Date.now()}`;
  record(chats, id, {
    type: "tool-started",
    call,
    source: { kind: "builtin" },
    name: "Read",
    input: {
      file_path:
        "C:/Users/lhuil/AppData/Roaming/app.cadence.desktop/repos/.worktrees/remisdv/go-alternance-crm-poc/extraction-issues/src/extraction/issues.go",
    },
  });
  await sleep(150);
  record(chats, id, {
    type: "tool-completed",
    call,
    ok: true,
    summary: "212 lignes",
    output: null,
  });
  for (const path of ["src/extraction/issues.go", "src/extraction/labels.go"])
    record(chats, id, { type: "file-changed", path, change: "modified" });
}

/** Questions du mode mock (message contenant « question »). */
export const MOCK_QUESTIONS: AiQuestion[] = [
  {
    id: "Quel format pour la fiche ?",
    header: "Format",
    question: "Quel format pour la fiche ?",
    options: [
      { label: "Résumé", description: "Une page, l'essentiel" },
      { label: "Détaillé", description: "Définitions, exemples et exercices" },
    ],
    multiSelect: false,
    other: true,
  },
  {
    id: "Quelles sections inclure ?",
    header: "Sections",
    question: "Quelles sections inclure ?",
    options: [
      { label: "Introduction", description: null },
      { label: "Formules", description: "Avec leurs unités" },
      { label: "Conclusion", description: null },
    ],
    multiSelect: true,
    other: true,
  },
];

const pendingQuestions = new Map<
  string,
  { questions: AiQuestion[]; resolve: (answers: QuestionAnswer[]) => void }
>();

/**
 * Réponse à une question en attente, comme `ChatService::answer` : une entrée non vide par
 * question, remises dans l'ordre. Renvoie un message d'erreur, ou `null` si c'est transmis.
 */
export function answerQuestion(request: string, answers: QuestionAnswer[]): string | null {
  const pending = pendingQuestions.get(request);
  if (!pending) return "Cette question n'attend plus de réponse.";
  const ordered = pending.questions.map((q) => ({
    id: q.id,
    answers: (answers.find((a) => a.id === q.id)?.answers ?? [])
      .map((s) => s.trim())
      .filter(Boolean),
  }));
  const missing = pending.questions.find((_, i) => ordered[i]!.answers.length === 0);
  if (missing) return `Répondez à « ${missing.question} ».`;
  pendingQuestions.delete(request);
  pending.resolve(ordered);
  return null;
}

export function answerApproval(request: string, decision: ApprovalDecision): boolean {
  const pending = pendingApprovals.get(request);
  if (!pending) return false;
  pendingApprovals.delete(request);
  pending.resolve(decision);
  return true;
}

/** Mode « Auto » choisi pendant un tour : les demandes en attente sont acceptées (comme le back). */
export function allowPendingApprovals(session: string) {
  for (const [request, pending] of pendingApprovals)
    if (pending.session === session) answerApproval(request, "allow");
}

export function requestInterrupt(id: string) {
  interrupts.add(id);
}

export function newChat(
  chats: Chats,
  text: string,
  ai: AiId = "claude-code",
  permissions: ChatPermissions = "ask",
  origin = "user",
): ChatSessionRow {
  const line =
    text
      .split("\n")
      .find((l) => l.trim())
      ?.trim() ?? "Conversation";
  const row = session(
    `c${Date.now().toString(36)}`,
    line.length > 60 ? `${line.slice(0, 59)}…` : line,
    0,
    origin,
  );
  row.ai = ai;
  row.permissions = permissions;
  row.model = ai === "claude-code" ? row.model : null;
  chats.set(row.id, { session: row, events: [], worktrees: [] });
  return row;
}

export { record as recordMockChatEvent };
