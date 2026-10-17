import { describe, expect, it } from "vitest";
import type { AiEvent, ChatEventRow } from "@/lib/types";
import {
  buildThread,
  compactArgs,
  conversationGroup,
  formatDuration,
  groupTurns,
  originPlugin,
  parseCadenceLink,
  toolLabel,
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
  it("garde la réponse donnée et affiche les rapports de fond", () => {
    const items = buildThread(
      rows([
        {
          type: "question",
          request: "q1",
          text: "Quelle langue ?",
          options: ["Français", "Anglais"],
        },
        { type: "question-answered", request: "q1", answer: "Français" },
        { type: "report", text: "Moitié faite", progress: 50, isFinal: false, data: null },
        { type: "report", text: "Terminé", progress: 100, isFinal: true, data: null },
      ]),
    );
    expect(items[0]).toMatchObject({ kind: "question", key: "q1", answer: "Français" });
    expect(
      items.filter((i) => i.kind === "report").map((i) => i.kind === "report" && i.final),
    ).toEqual([false, true]);
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

  it("range les conversations par jour", () => {
    const now = new Date(2026, 9, 17, 12, 0);
    expect(conversationGroup(new Date(2026, 9, 17, 9).toISOString(), now)).toBe("Aujourd'hui");
    expect(conversationGroup(new Date(2026, 9, 16, 23).toISOString(), now)).toBe("Hier");
    expect(conversationGroup(new Date(2026, 9, 13).toISOString(), now)).toBe("Cette semaine");
    expect(conversationGroup(new Date(2026, 8, 1).toISOString(), now)).toBe("Plus ancien");
  });
});
