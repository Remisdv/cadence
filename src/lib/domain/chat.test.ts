import { describe, expect, it } from "vitest";
import type { AiEvent, AiQuestion, ChatEventRow, ChatSessionRow } from "@/lib/types";
import {
  activitySummary,
  awaitsUser,
  buildThread,
  chatBarSections,
  contextPlugin,
  conversationOriginLabel,
  pluginConversations,
  compactArgs,
  composeAnswer,
  conversationGroup,
  formatDuration,
  globalConversations,
  groupReturnLabel,
  groupTurns,
  isActivity,
  localGroup,
  originPlugin,
  parseCadenceLink,
  pluginApprovals,
  pluginToolOf,
  railWaiting,
  toolLabel,
  waitingLabel,
  waitingSummary,
} from "./chat";

const rows = (events: AiEvent[]): ChatEventRow[] =>
  events.map((event, i) => ({ seq: i + 1, turn: 1, at: "2026-10-16T12:00:00Z", event }));

describe("buildThread", () => {
  it("assemble bulles, outils, approbations et fin de tour", () => {
    const items = buildThread(
      rows([
        { type: "user-message", text: "Prépare l'issue 142" },
        {
          type: "tool-started",
          call: "t1",
          source: { kind: "cadence", plugin: "gestion-projet", tool: "issue_get" },
          name: "mcp__cadence__gestion-projet__issue_get",
          input: { number: 142 },
        },
        { type: "tool-completed", call: "t1", ok: true, summary: "Bug d'affichage", output: null },
        {
          type: "approval-requested",
          request: "p1",
          kind: "file-change",
          title: "Write ok.txt",
          detail: {},
        },
        { type: "approval-answered", request: "p1", decision: "allow" },
        { type: "message-completed", message: "m:0", text: "C'est prêt." },
        { type: "usage", inputTokens: 10, outputTokens: 5, costUsd: 0.02 },
        { type: "turn-completed", ok: true, interrupted: false, error: null, durationMs: 1200 },
      ]),
    );
    expect(items.map((i) => i.kind)).toEqual(["user", "tool", "approval", "assistant", "turn"]);
    const tool = items[1]!;
    expect(tool.kind === "tool" && tool.status).toBe("ok");
    expect(tool.kind === "tool" && toolLabel(tool)).toBe("gestion-projet › issue_get");
    expect(items[2]!.kind === "approval" && items[2]!.decision).toBe("allow");
    expect(items[4]!.kind === "turn" && items[4]!.costUsd).toBe(0.02);
  });

  it("ajoute les textes en cours et fige un message interrompu", () => {
    const streaming = buildThread(rows([{ type: "user-message", text: "Un poème" }]), {
      "m:0": "Sur la mer",
    });
    expect(streaming[1]).toMatchObject({ kind: "assistant", text: "Sur la mer", streaming: true });

    const interrupted = buildThread(
      rows([
        { type: "user-message", text: "Un poème" },
        { type: "message-delta", message: "m:0", text: "Sur la " },
        { type: "message-delta", message: "m:0", text: "mer" },
        { type: "turn-completed", ok: false, interrupted: true, error: null, durationMs: 0 },
      ]),
    );
    expect(interrupted[1]).toMatchObject({
      kind: "assistant",
      text: "Sur la mer",
      streaming: false,
    });
    expect(interrupted[2]).toMatchObject({ kind: "turn", interrupted: true });
  });

  it("met à jour une commande shell avec sa sortie", () => {
    const items = buildThread(
      rows([
        { type: "command", call: "c1", command: "cargo test", exitCode: null, output: null },
        { type: "command", call: "c1", command: "cargo test", exitCode: 1, output: "échec" },
      ]),
    );
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      kind: "command",
      running: false,
      exitCode: 1,
      output: "échec",
    });
  });
});

describe("questions et rapports", () => {
  it("lit une question d'avant les questions multiples et affiche les rapports de fond", () => {
    // Historique enregistré avant `questions` / `answers` : champs absents.
    const old = [
      {
        type: "question",
        request: "q1",
        text: "Quelle langue ?",
        options: ["Français", "Anglais"],
      },
      { type: "question-answered", request: "q1", answer: "Français" },
    ] as unknown as AiEvent[];
    const items = buildThread(
      rows([
        ...old,
        { type: "report", text: "Moitié faite", progress: 50, isFinal: false, data: null },
        { type: "report", text: "Terminé", progress: 100, isFinal: true, data: null },
      ]),
    );
    expect(items[0]).toMatchObject({
      kind: "question",
      key: "q1",
      answer: "Français",
      answers: [],
      questions: [
        {
          id: "Quelle langue ?",
          options: [{ label: "Français" }, { label: "Anglais" }],
          multiSelect: false,
          other: true,
        },
      ],
    });
    expect(
      items.filter((i) => i.kind === "report").map((i) => i.kind === "report" && i.final),
    ).toEqual([false, true]);
  });

  it("garde les questions détaillées et les réponses données", () => {
    const questions: AiQuestion[] = [
      {
        id: "Quel format ?",
        header: "Format",
        question: "Quel format ?",
        options: [
          { label: "Résumé", description: "Court" },
          { label: "Détaillé", description: null },
        ],
        multiSelect: false,
        other: true,
      },
      {
        id: "Quelles sections ?",
        header: "Sections",
        question: "Quelles sections ?",
        options: [
          { label: "Intro", description: null },
          { label: "Conclusion", description: null },
        ],
        multiSelect: true,
        other: false,
      },
    ];
    const answers = [
      { id: "Quel format ?", answers: ["Résumé"] },
      { id: "Quelles sections ?", answers: ["Intro", "Conclusion"] },
    ];
    const items = buildThread(
      rows([
        { type: "question", request: "q2", text: "Quel format ?", options: [], questions },
        {
          type: "question-answered",
          request: "q2",
          answer: "Format : Résumé · Sections : Intro, Conclusion",
          answers,
        },
      ]),
    );
    expect(items[0]).toMatchObject({ kind: "question", questions, answers });

    // Choix de la carte : un seul pour un choix unique, dans l'ordre des options, puis « Autre ».
    const [format, sections] = questions as [AiQuestion, AiQuestion];
    expect(composeAnswer(format, ["Détaillé", "Résumé"], "")).toEqual(["Résumé"]);
    expect(composeAnswer(format, [], "  Une page  ")).toEqual(["Une page"]);
    expect(composeAnswer(sections, ["Conclusion", "Intro"], "ignoré")).toEqual([
      "Intro",
      "Conclusion",
    ]);
    expect(composeAnswer(sections, [], "")).toEqual([]);
  });
});

describe("aides du chat", () => {
  it("groupe les tours derrière chaque message", () => {
    const turns = groupTurns(
      buildThread(
        rows([
          { type: "user-message", text: "a" },
          { type: "message-completed", message: "m:0", text: "b" },
          { type: "user-message", text: "c" },
        ]),
      ),
    );
    expect(turns.map((t) => [t.user?.text, t.reply.length])).toEqual([
      ["a", 1],
      ["c", 0],
    ]);
  });

  it("formate durées, arguments et liens cadence:", () => {
    expect(formatDuration(12_300)).toBe("12 s");
    expect(formatDuration(128_000)).toBe("2 min 08 s");
    expect(compactArgs({ depots: "tous", n: 3 })).toBe("depots=tous n=3");
    expect(parseCadenceLink("cadence:documents/document?doc=01J&page=12")).toEqual({
      plugin: "documents",
      view: "document",
      params: { doc: "01J", page: "12" },
    });
    expect(parseCadenceLink("https://example.org")).toBeNull();
    expect(originPlugin("action:gestion-projet/preparer-issue")).toBe("gestion-projet");
    expect(originPlugin("user")).toBeNull();
  });

  it("écarte les conversations cachées et lit le groupe vu par son plugin", () => {
    const list = [
      { id: "a", hidden: false },
      { id: "b", hidden: true },
    ];
    expect(globalConversations(list).map((r) => r.id)).toEqual(["a"]);
    const group = { id: "gestion-projet:sprint:o/n:3", label: "Sprint 3 · o/n" };
    expect(localGroup("gestion-projet", group)).toEqual({ id: "sprint:o/n:3", label: group.label });
    expect(localGroup("gestion", group)).toBeNull();
    expect(localGroup("documents", group)).toBeNull();
    expect(localGroup("gestion-projet", null)).toBeNull();
    expect(groupReturnLabel({ id: "sprint:o/n:3", label: "" }, "Gestion de projet")).toBe(
      "Retour au sprint",
    );
    expect(groupReturnLabel({ id: "lot:4", label: "" }, "Documents")).toBe("Retour à Documents");
  });

  it("range les conversations par jour", () => {
    const now = new Date(2026, 9, 17, 12, 0);
    expect(conversationGroup(new Date(2026, 9, 17, 9).toISOString(), now)).toBe("Aujourd'hui");
    expect(conversationGroup(new Date(2026, 9, 16, 23).toISOString(), now)).toBe("Hier");
    expect(conversationGroup(new Date(2026, 9, 13).toISOString(), now)).toBe("Cette semaine");
    expect(conversationGroup(new Date(2026, 8, 1).toISOString(), now)).toBe("Plus ancien");
  });
});

describe("activité d'un tour", () => {
  const items = buildThread(
    rows([
      { type: "user-message", text: "Lance les tests" },
      { type: "command", call: "c1", command: "ls -la", exitCode: null, output: null },
      { type: "command", call: "c1", command: "ls -la", exitCode: 0, output: "total 0" },
      { type: "command", call: "c2", command: "go test ./...", exitCode: 1, output: "FAIL" },
      {
        type: "tool-started",
        call: "t1",
        source: { kind: "builtin" },
        name: "Read",
        input: { file_path: "README.md" },
      },
      { type: "tool-completed", call: "t1", ok: true, summary: null, output: null },
      { type: "file-changed", path: "a.go", change: "modified" },
      { type: "file-changed", path: "a.go", change: "modified" },
      { type: "file-changed", path: "b.go", change: "added" },
      { type: "message-completed", message: "m:0", text: "Fini." },
    ]),
  );

  it("remplace une commande en cours par son résultat (même appel)", () => {
    const commands = items.filter((i) => i.kind === "command");
    expect(commands).toHaveLength(2);
    expect(commands[0]).toMatchObject({ command: "ls -la", running: false, exitCode: 0 });
    expect(commands[1]).toMatchObject({ running: false, exitCode: 1, output: "FAIL" });
  });

  it("classe commandes, outils et fichiers comme activité, pas les messages", () => {
    expect(items.filter(isActivity).map((i) => i.kind)).toEqual([
      "command",
      "command",
      "tool",
      "file",
      "file",
      "file",
    ]);
    expect(items.filter((i) => !isActivity(i)).map((i) => i.kind)).toEqual(["user", "assistant"]);
  });

  it("résume l'activité (fichiers comptés une fois, accords au pluriel)", () => {
    expect(activitySummary(items)).toBe("2 commandes · 1 outil · 2 fichiers modifiés");
    expect(activitySummary(items.filter((i) => i.kind === "command").slice(0, 1))).toBe(
      "1 commande",
    );
    expect(activitySummary(items.filter((i) => i.kind === "file").slice(0, 1))).toBe(
      "1 fichier modifié",
    );
    expect(activitySummary([])).toBe("");
  });
});

describe("colonne du chat et barre du plugin (recette du 21/10)", () => {
  const now = new Date("2026-10-21T12:00:00");
  const row = (id: string, extra: Partial<ChatSessionRow> = {}): ChatSessionRow => ({
    id,
    title: id,
    ai: "claude-code",
    model: null,
    origin: "user",
    mode: "chat",
    permissions: "ask",
    state: "idle",
    createdAt: "2026-10-21T09:00:00",
    updatedAt: "2026-10-21T09:00:00",
    group: null,
    hidden: false,
    ...extra,
  });
  const sprint = { id: "gestion-projet:sprint:o/n:3", label: "Sprint 0.2.0 · o/n" };
  const all = [
    row("libre"),
    row("Relire et valider", {
      hidden: true,
      group: sprint,
      origin: "action:gestion-projet/relire",
    }),
    row("Développer le sprint", {
      hidden: true,
      group: sprint,
      origin: "action:gestion-projet/developper-sprint",
      state: "waiting-approval",
    }),
    row("fond", { mode: "background" }),
  ];
  const titles = (sections: ReturnType<typeof chatBarSections<ChatSessionRow>>) =>
    sections.map((s) => `${s.title}: ${s.rows.map((r) => r.id).join(", ")}`);

  it("garde les conversations de sprint hors de la liste, même en attente, sauf ouverte (R06)", () => {
    expect(titles(chatBarSections(all, "", null, now))).toEqual([
      "En fond: fond",
      "Aujourd'hui: libre",
    ]);
    const opened = chatBarSections(all, "", "Relire et valider", now);
    expect(opened[0]!.attached).toBe(true);
    expect(opened[0]!.rows.map((r) => r.id)).toEqual(["Relire et valider"]);
  });

  it("la recherche trouve les conversations de sprint par titre ou nom du sprint (P18)", () => {
    expect(titles(chatBarSections(all, "relire", null, now))).toEqual([
      "Rattachées: Relire et valider",
    ]);
    expect(chatBarSections(all, "sprint 0.2.0", null, now)[0]!.rows).toHaveLength(2);
    expect(chatBarSections(all, "introuvable", null, now)).toEqual([]);
  });

  it("liste les conversations non cachées d'un plugin et leur origine (P19)", () => {
    const rows = [
      row("a", { origin: "action:gestion-projet/extraire-issues" }),
      row("b", { origin: "plugin:gestion-projet" }),
      row("c", { origin: "action:gestion-projet/relire", hidden: true, group: sprint }),
      row("d", { origin: "plugin:documents" }),
      row("e"),
    ];
    expect(pluginConversations(rows, "gestion-projet").map((r) => r.id)).toEqual(["a", "b"]);
    expect(contextPlugin("plugin:gestion-projet")).toBe("gestion-projet");
    expect(contextPlugin("action:documents/fiche")).toBe("documents");
    expect(contextPlugin("user")).toBeNull();
    const plugin = { actions: [{ id: "extraire-issues", label: "Créer des issues" }] };
    expect(conversationOriginLabel("action:gestion-projet/extraire-issues", plugin)).toBe(
      "Créer des issues",
    );
    expect(conversationOriginLabel("action:gestion-projet/autre", plugin)).toBe("autre");
    expect(conversationOriginLabel("plugin:gestion-projet", plugin)).toBe("Conversation libre");
  });

  it("garde la portée de « pour la session » sur la carte d'approbation (P12)", () => {
    const items = buildThread(
      rows([
        {
          type: "approval-requested",
          request: "r1",
          kind: "command",
          title: "Bash : git status",
          detail: { command: "git status" },
          tool: "Bash",
          scope: "commande git",
        },
      ]),
    );
    expect(items[0]).toMatchObject({ kind: "approval", scope: "commande git", decision: null });
  });
});

describe("attente de l'utilisateur (N10) et outils de plugin (N12)", () => {
  it("une question en fin de tour compte comme une attente de réponse", () => {
    expect(awaitsUser("waiting-reply")).toBe(true);
    expect(awaitsUser("waiting-approval")).toBe(true);
    expect(awaitsUser("idle")).toBe(false);
    expect(waitingLabel("waiting-reply")).toBe("En attente de votre réponse");
    expect(waitingLabel("waiting-approval")).toBe("Attend votre accord");
    expect(waitingLabel("running")).toBeNull();
  });

  const base = {
    ai: "claude-code" as const,
    model: null,
    mode: "chat" as const,
    permissions: "ask" as const,
    createdAt: "2026-10-21T09:00:00",
    updatedAt: "2026-10-21T09:00:00",
    group: null,
    hidden: false,
    origin: "user",
    title: "",
    state: "idle" as ChatSessionRow["state"],
  };
  const sprintGroup = { id: "gestion-projet:sprint:1", label: "Sprint 1" };

  it("une conversation de sprint en attente ne remonte ni dans la liste ni au rail (R06)", () => {
    const relire = {
      ...base,
      id: "relire",
      title: "Relire et valider",
      group: sprintGroup,
      hidden: true,
      origin: "action:gestion-projet/relire-valider",
      state: "waiting-reply" as const,
    };
    expect(chatBarSections([relire], "", null, new Date("2026-10-21T12:00:00"))).toEqual([]);
    expect(railWaiting([relire])).toEqual({ reply: 0, approval: 0, sessions: [] });
    // Une réponse attendue (sprint clos ou non) n'allume pas non plus la tuile du plugin.
    expect(pluginApprovals([relire], "gestion-projet")).toEqual([]);
    // Un accord en attente (tour bloqué) s'y signale, pour son plugin seulement.
    const developper = { ...relire, id: "dev", state: "waiting-approval" as const };
    expect(railWaiting([developper]).sessions).toEqual([]);
    expect(pluginApprovals([developper], "gestion-projet").map((s) => s.id)).toEqual(["dev"]);
    expect(pluginApprovals([developper], "documents")).toEqual([]);
    expect(pluginApprovals([{ ...developper, hidden: false }], "gestion-projet")).toEqual([]);
  });

  it("le rail compte à part les réponses et les accords attendus (R04)", () => {
    const rows = [
      { ...base, id: "a", state: "waiting-reply" as const },
      { ...base, id: "b", state: "waiting-approval" as const },
      { ...base, id: "c", state: "running" as const },
    ];
    const waiting = railWaiting(rows);
    expect(waiting).toMatchObject({ reply: 1, approval: 1 });
    expect(waiting.sessions.map((s) => s.id)).toEqual(["a", "b"]);
    expect(waitingSummary(waiting)).toBe(
      "2 conversations attendent votre action · 1 réponse, 1 accord",
    );
    expect(waitingSummary({ reply: 2, approval: 3 })).toBe(
      "5 conversations attendent votre action · 2 réponses, 3 accords",
    );
    expect(waitingSummary({ reply: 1, approval: 0 })).toBe("1 conversation attend votre réponse");
    expect(waitingSummary({ reply: 0, approval: 2 })).toBe(
      "2 conversations attendent votre accord",
    );
    expect(waitingSummary({ reply: 0, approval: 0 })).toBe("Chat global · tous les plugins");
  });

  it("reconnaît les outils de plugin dans les demandes d'accord des trois IA", () => {
    expect(pluginToolOf("mcp__cadence__gestion-projet__merge_pull_request")).toEqual({
      plugin: "gestion-projet",
      tool: "merge_pull_request",
    });
    expect(pluginToolOf("cadence_gestion-projet__create_issue")).toEqual({
      plugin: "gestion-projet",
      tool: "create_issue",
    });
    expect(pluginToolOf("mcp__cadence__cadence__repo_checkout")).toBeNull();
    expect(pluginToolOf("Bash")).toBeNull();
    expect(pluginToolOf(null)).toBeNull();
  });

  it("garde l'outil de la demande dans la carte d'accord", () => {
    const [card] = buildThread(
      rows([
        {
          type: "approval-requested",
          request: "r1",
          kind: "tool",
          title: "Gestion de projet › merge_pull_request · pull_number: 22",
          detail: { pull_number: 22 },
          tool: "mcp__cadence__gestion-projet__merge_pull_request",
        },
      ]),
    );
    expect(card).toMatchObject({
      kind: "approval",
      tool: "mcp__cadence__gestion-projet__merge_pull_request",
    });
  });
});
