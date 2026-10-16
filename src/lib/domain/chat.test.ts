import { describe, expect, it } from "vitest";
import type { AiEvent, ChatEventRow } from "@/lib/types";
import { buildThread, toolLabel } from "./chat";

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
