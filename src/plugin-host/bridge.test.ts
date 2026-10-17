import { describe, expect, it, vi } from "vitest";
import { CommandError } from "@/lib/ipc/errors";
import { createBridge, type HostApi } from "./bridge";
import type { Envelope } from "./protocol";

function setup(overrides: Partial<HostApi> = {}) {
  const sent: Envelope[] = [];
  const frame = { postMessage: (m: unknown) => sent.push(m as Envelope) };
  const store = new Map<string, unknown>();
  const api: HostApi = {
    call: vi.fn(async () => ({ isError: false, structured: { valeur: 1 }, text: "" })),
    navigate: vi.fn(),
    notify: vi.fn(),
    openExternal: vi.fn(),
    shortcut: vi.fn(),
    settingsGet: vi.fn(async () => ({
      plugin: "exemple",
      schema: null,
      value: { a: 1 },
      errors: [],
    })),
    settingsSet: vi.fn(async (value) => ({ plugin: "exemple", schema: null, value, errors: [] })),
    secretSet: vi.fn(async () => {}),
    secretHas: vi.fn(async () => true),
    storageGet: (key) => store.get(key),
    storageSet: (key, value) => store.set(key, value),
    runAction: vi.fn(async () => ({ runId: "c1", sessionId: null, mode: "background" })),
    pickFiles: vi.fn(async () => ["C:/cours.pdf"]),
    reposList: vi.fn(async () => ["RemiSDV/cadence"]),
    reposSet: vi.fn(async (update: (r: string[]) => string[]) => update(["RemiSDV/cadence"])),
    reposLocalPaths: vi.fn(async () => ({ "remisdv/cadence": "C:/repos/remisdv/cadence" })),
    reposCheckout: vi.fn(
      async (repo: string, task: string | null) => `C:/repos/.worktrees/${repo}/${task}`,
    ),
    toolStatus: vi.fn(async () => ({ installed: true, version: "2.47.1", authenticated: null })),
    ...overrides,
  };
  const bridge = createBridge({
    frame,
    api,
    hello: () => ({
      api: 1,
      plugin: { id: "exemple", version: "0.1.0" },
      route: { view: "accueil", params: {} },
      theme: { "--color-bg": "#000" },
      locale: "fr-FR",
      host: { version: "0.3.0" },
    }),
  });
  let n = 0;
  const request = async (method: string, params?: unknown, source: unknown = frame) => {
    const id = `r${++n}`;
    await bridge.handle({ source, data: { cadence: 1, kind: "req", id, method, params } });
    return sent.find((m) => m.kind === "res" && m.id === id);
  };
  return { bridge, api, sent, frame, request };
}

describe("pont de l'iframe", () => {
  it("répond à la poignée de main avec les méthodes connues", async () => {
    const { request } = setup();
    const res = await request("hello", { sdk: "1.0.0" });
    expect(res).toMatchObject({ kind: "res", result: { api: 1, plugin: { id: "exemple" } } });
    const methods = (res as { result: { methods: string[] } }).result.methods;
    expect(methods).toEqual(
      expect.arrayContaining(["hello", "tools.call", "nav.go", "storage.set"]),
    );
  });

  it("ignore les messages d'une autre source et les enveloppes mal formées", async () => {
    const { bridge, sent, request } = setup();
    expect(
      await bridge.handle({
        source: {},
        data: { cadence: 1, kind: "req", id: "x", method: "hello" },
      }),
    ).toBe(false);
    expect(await request("hello", undefined, { postMessage() {} })).toBeUndefined();
    await bridge.handle({ source: setup().frame, data: "bonjour" });
    expect(sent).toHaveLength(0);
  });

  it("appelle les outils du plugin seulement, et type leurs erreurs", async () => {
    const { request, api } = setup();
    expect(await request("tools.call", { tool: "compteur", args: { pas: 2 } })).toMatchObject({
      result: { valeur: 1 },
    });
    expect(api.call).toHaveBeenCalledWith("compteur", { pas: 2 });

    expect(await request("tools.call", { tool: "autre__issue_get" })).toMatchObject({
      error: { code: "invalid_params" },
    });
    expect(await request("tools.call", { tool: "Majuscule" })).toMatchObject({
      error: { code: "invalid_params" },
    });

    const failing = setup({
      call: async () => ({ isError: true, structured: { détail: 1 }, text: "Échec" }),
    });
    expect(await failing.request("tools.call", { tool: "x" })).toMatchObject({
      error: { code: "tool_error", message: "Échec", data: { détail: 1 } },
    });
    const down = setup({
      call: async () => {
        throw new CommandError("plugin_call", { kind: "plugin-unavailable", message: "arrêté" });
      },
    });
    expect(await down.request("tools.call", { tool: "x" })).toMatchObject({
      error: { code: "unavailable", message: "arrêté" },
    });
  });

  it("rend le texte d'un outil, en JSON quand c'est possible", async () => {
    const json = setup({
      call: async () => ({ isError: false, structured: null, text: '{"a":1}' }),
    });
    expect(await json.request("tools.call", { tool: "x" })).toMatchObject({ result: { a: 1 } });
    const plain = setup({
      call: async () => ({ isError: false, structured: null, text: "bonjour" }),
    });
    expect(await plain.request("tools.call", { tool: "x" })).toMatchObject({ result: "bonjour" });
  });

  it("navigue, notifie, ouvre les seuls liens http(s) et range le stockage", async () => {
    const { request, api } = setup();
    await request("nav.go", { view: "outils", params: { n: 3 } });
    expect(api.navigate).toHaveBeenCalledWith("outils", { n: "3" });
    await request("ui.notify", { message: "Fait", kind: "success" });
    expect(api.notify).toHaveBeenCalledWith("Fait", "success");
    expect(await request("ui.open", { url: "javascript:alert(1)" })).toMatchObject({
      error: { code: "invalid_params" },
    });
    await request("ui.open", { url: "https://example.org" });
    expect(api.openExternal).toHaveBeenCalledWith("https://example.org");

    await request("storage.set", { key: "onglet", value: { n: 2 } });
    expect(await request("storage.get", { key: "onglet" })).toMatchObject({ result: { n: 2 } });
    expect(await request("storage.get", { key: "absent" })).toMatchObject({ result: null });
    expect(await request("inconnue")).toMatchObject({ error: { code: "method_not_found" } });
  });

  it("lance les actions du plugin, IA vérifiée", async () => {
    const { request, api } = setup();
    expect(await request("actions.run", { id: "resumer", context: { type: "doc" } })).toMatchObject(
      {
        result: { runId: "c1", mode: "background" },
      },
    );
    expect(api.runAction).toHaveBeenCalledWith("resumer", { type: "doc" }, null);
    await request("actions.run", { id: "resumer", ai: "codex" });
    expect(api.runAction).toHaveBeenLastCalledWith("resumer", null, "codex");
    expect(await request("actions.run", { id: "resumer", ai: "gpt" })).toMatchObject({
      error: { code: "invalid_params" },
    });
  });

  it("parle aussi la variante du SDK publié (@cadence/plugin-sdk)", async () => {
    const { bridge, sent, frame } = setup();
    const ask = async (id: string, method: string, params: unknown) => {
      await bridge.handle({
        source: frame,
        data: { channel: "cadence", api: 1, kind: "request", id, method, params },
      });
      return sent.find((m) => (m as unknown as { id?: string }).id === id) as unknown;
    };
    expect(await ask("h", "hello", { api: 1, sdk: "1.0.0" })).toMatchObject({
      channel: "cadence",
      kind: "response",
      ok: true,
      result: { api: 1, plugin: { id: "exemple" }, route: { view: "accueil" } },
    });
    expect(await ask("t", "tool.call", { tool: "lister", args: {} })).toMatchObject({
      ok: true,
      result: { structuredContent: { valeur: 1 }, isError: false },
    });
    expect(await ask("a", "action.run", { action: "resumer", context: null })).toMatchObject({
      ok: true,
      result: { runId: "c1" },
    });
    expect(await ask("r", "repos.list", {})).toMatchObject({
      result: [
        {
          fullName: "RemiSDV/cadence",
          owner: "RemiSDV",
          name: "cadence",
          localPath: "C:/repos/remisdv/cadence",
        },
      ],
    });
    expect(await ask("x", "inconnue", {})).toMatchObject({
      ok: false,
      error: { code: "method_not_found" },
    });
    // Les événements suivent la variante de l'iframe.
    bridge.emit("run", { runId: "c1", text: "fini", final: true, data: null });
    bridge.emit("sync.updated", { ids: [1] });
    expect(sent.slice(-2)).toEqual([
      {
        channel: "cadence",
        api: 1,
        kind: "event",
        topic: "run",
        data: { runId: "c1", status: "succeeded", message: "fini", result: "fini" },
      },
      {
        channel: "cadence",
        api: 1,
        kind: "event",
        topic: "event",
        data: { name: "sync.updated", data: { ids: [1] } },
      },
    ]);
  });

  it("prépare le worktree d'une tâche pour un dépôt suivi (repos.checkout)", async () => {
    const { bridge, sent, frame, api } = setup();
    const ask = async (id: string, params: unknown) => {
      await bridge.handle({
        source: frame,
        data: { channel: "cadence", api: 1, kind: "request", id, method: "repos.checkout", params },
      });
      return sent.find((m) => (m as unknown as { id?: string }).id === id) as unknown;
    };
    expect(await ask("c1", { repo: "remisdv/Cadence", task: "issue-42" })).toMatchObject({
      ok: true,
      result: { path: "C:/repos/.worktrees/remisdv/Cadence/issue-42" },
    });
    expect(api.reposCheckout).toHaveBeenCalledWith("remisdv/Cadence", "issue-42");
    expect(await ask("c2", { repo: "me/autre" })).toMatchObject({
      ok: false,
      error: { code: "not_found" },
    });
    expect(await ask("c3", { repo: "pas un dépôt" })).toMatchObject({
      ok: false,
      error: { code: "invalid_params" },
    });
    expect(api.reposCheckout).toHaveBeenCalledTimes(1);
  });

  it("fusionne les réglages et pousse les événements", async () => {
    const { request, api, bridge, sent } = setup();
    await request("settings.set", { patch: { b: 2 } });
    expect(api.settingsSet).toHaveBeenCalledWith({ a: 1, b: 2 });
    bridge.emit("route", { view: "outils", params: {} });
    expect(sent.at(-1)).toEqual({
      cadence: 1,
      kind: "evt",
      event: "route",
      data: { view: "outils", params: {} },
    });
  });
});
