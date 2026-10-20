/**
 * `cadence.conversations` : méthodes du pont (deux variantes du protocole), événement
 * `conversations`, et isolation entre plugins (l'`HostApi` de `host.ts`, branchée ici sur les
 * données du mode mock à la place de l'IPC).
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CommandArgs, CommandName } from "@/lib/ipc/commands";
import { CommandError } from "@/lib/ipc/errors";
import {
  MOCK_SPRINT_GROUP,
  listMockChats,
  mockChats,
  mockConversation,
  mockConversations,
} from "@/lib/ipc/mock/chat";
import { globalConversations } from "@/lib/domain/chat";
import { createBridge } from "./bridge";
import { conversationEvent } from "./conversations";
import type { Envelope } from "./protocol";

const chats = mockChats(false);

vi.mock("@/lib/ipc/transport", () => ({
  isMockIpc: true,
  isTauri: false,
  call: async <C extends CommandName>(command: C, args: CommandArgs<C>) => {
    const a = args as Record<string, string | null>;
    if (command === "plugin_conversations")
      return mockConversations(chats, a.plugin!, a.group ?? null);
    if (command === "plugin_conversation_get") {
      const found = mockConversation(chats, a.plugin!, a.id!);
      if (!found)
        throw new CommandError(command, {
          kind: "validation",
          message: `Conversation « ${a.id} » introuvable.`,
        });
      return found;
    }
    throw new Error(`commande inattendue : ${command}`);
  },
}));

const navigate = vi.fn();
vi.mock("@/stores/navigation", () => ({ navigate: (view: unknown) => navigate(view) }));

const { hostApi } = await import("./host");

function bridgeFor(plugin: string) {
  const sent: Envelope[] = [];
  const frame = { postMessage: (m: unknown) => sent.push(m as Envelope) };
  const bridge = createBridge({
    frame,
    api: hostApi({ id: plugin, name: plugin }),
    hello: () => ({
      api: 1,
      plugin: { id: plugin, version: "0.1.0" },
      route: { view: "sprint", params: {} },
      theme: {},
      locale: "fr-FR",
      host: { version: "0.3.0" },
    }),
  });
  let n = 0;
  const request = async (method: string, params?: unknown) => {
    const id = `r${++n}`;
    await bridge.handle({ source: frame, data: { cadence: 1, kind: "req", id, method, params } });
    return sent.find((m) => m.kind === "res" && m.id === id) as
      { result?: unknown; error?: { code: string } } | undefined;
  };
  const ask = async (method: string, params: unknown) => {
    const id = `p${++n}`;
    await bridge.handle({
      source: frame,
      data: { channel: "cadence", api: 1, kind: "request", id, method, params },
    });
    return sent.find((m) => (m as unknown as { id?: string }).id === id) as unknown;
  };
  return { bridge, sent, request, ask };
}

beforeEach(() => navigate.mockClear());

describe("cadence.conversations", () => {
  it("liste les conversations d'un groupe du plugin, cachées comprises", async () => {
    const { request } = bridgeFor("gestion-projet");
    const res = await request("conversations.list", { group: MOCK_SPRINT_GROUP.id });
    const list = res?.result as { id: string; group?: { id: string }; turns: number }[];
    expect(list.map((c) => c.id).sort()).toEqual(["s12", "s12-214", "s12-217"]);
    expect(list.every((c) => c.group?.id === MOCK_SPRINT_GROUP.id)).toBe(true);
    expect(list[0]).toMatchObject({ action: expect.any(String), turns: 1 });
    // Sans groupe : pas les conversations cachées.
    expect((await request("conversations.list", {}))?.result).toEqual([]);
    expect(await request("conversations.list", { group: "pas valide !" })).toMatchObject({
      error: { code: "invalid_params" },
    });
  });

  it("rend une conversation sans sorties d'outils, et l'ouvre dans le chat", async () => {
    const { request } = bridgeFor("gestion-projet");
    const res = await request("conversations.get", { id: "s12-214" });
    expect(res?.result).toMatchObject({
      info: { id: "s12-214", group: MOCK_SPRINT_GROUP, context: { issue: 214 } },
      messages: [{ role: "user" }, { role: "assistant" }],
      tools: [{ name: "shell", ok: true }],
      usage: { inputTokens: 64_000, outputTokens: 2_100 },
    });
    expect(JSON.stringify(res?.result)).not.toContain("41 tests passés");
    expect(await request("conversations.open", { id: "s12-214" })).toMatchObject({ result: null });
    expect(navigate).toHaveBeenCalledWith({ id: "chat", session: "s12-214" });
    expect(await request("conversations.get", {})).toMatchObject({
      error: { code: "invalid_params" },
    });
  });

  it("refuse les conversations et les groupes d'un autre plugin", async () => {
    const { request, ask } = bridgeFor("documents");
    expect(await request("conversations.get", { id: "s12" })).toMatchObject({
      error: { code: "not_found" },
    });
    expect(await request("conversations.open", { id: "s12" })).toMatchObject({
      error: { code: "not_found" },
    });
    expect(navigate).not.toHaveBeenCalled();
    expect((await request("conversations.list", { group: MOCK_SPRINT_GROUP.id }))?.result).toEqual(
      [],
    );
    expect(await ask("conversations.get", { id: "s12" })).toMatchObject({
      ok: false,
      error: { code: "not_found" },
    });
  });

  it("parle aussi la variante du SDK publié", async () => {
    const { ask } = bridgeFor("gestion-projet");
    expect(await ask("conversations.list", { group: MOCK_SPRINT_GROUP.id })).toMatchObject({
      ok: true,
      result: expect.arrayContaining([expect.objectContaining({ id: "s12" })]),
    });
    expect(await ask("conversations.get", { id: "s12" })).toMatchObject({
      ok: true,
      result: { info: { id: "s12", mode: "background" } },
    });
  });

  it("relaie l'événement `conversations` au seul plugin de la conversation", async () => {
    const payload = {
      session: "s12-214",
      state: "running" as const,
      plugin: "gestion-projet",
      group: MOCK_SPRINT_GROUP,
    };
    expect(conversationEvent("gestion-projet", payload)).toEqual({
      id: "s12-214",
      group: MOCK_SPRINT_GROUP,
      state: "running",
    });
    expect(conversationEvent("documents", payload)).toBeNull();
    expect(conversationEvent("gestion-projet", { session: "c1", state: "idle" })).toBeNull();
    expect(
      conversationEvent("gestion-projet", {
        session: "x",
        state: "idle",
        plugin: "gestion-projet",
      }),
    ).toEqual({ id: "x", state: "idle" });

    // Les deux variantes : `evt` du cœur, sujet `conversations` du SDK publié.
    const core = bridgeFor("gestion-projet");
    core.bridge.emit("conversations", conversationEvent("gestion-projet", payload));
    expect(core.sent.at(-1)).toMatchObject({ kind: "evt", event: "conversations" });
    const published = bridgeFor("gestion-projet");
    await published.ask("conversations.list", {});
    published.bridge.emit("conversations", conversationEvent("gestion-projet", payload));
    expect(published.sent.at(-1)).toMatchObject({
      kind: "event",
      topic: "conversations",
      data: { id: "s12-214", state: "running" },
    });
  });
});

describe("liste globale du chat", () => {
  it("n'affiche pas les conversations cachées, sauf avec un filtre de groupe", () => {
    const global = listMockChats(chats);
    expect(global.some((c) => c.hidden)).toBe(false);
    expect(global.map((c) => c.id)).toEqual(expect.arrayContaining(["c1", "c2"]));
    expect(global.some((c) => c.id.startsWith("s12"))).toBe(false);
    const grouped = listMockChats(chats, `gestion-projet:${MOCK_SPRINT_GROUP.id}`);
    expect(grouped).toHaveLength(3);
    expect(globalConversations(grouped)).toEqual([]);
  });
});
