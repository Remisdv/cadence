/**
 * SDK des iframes (`public/mock-plugins/cadence-sdk.js`), exécuté dans une fenêtre simulée :
 * file d'attente avant `hello`, réponses et erreurs typées, événements, thème, `supports`.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import source from "../../public/mock-plugins/cadence-sdk.js?raw";
import type { Envelope } from "./protocol";


type Listener = (event: { source: unknown; data: unknown }) => void;

function loadSdk() {
  const sent: Envelope[] = [];
  const listeners: Record<string, Listener[]> = {};
  const styles: Record<string, string> = {};
  const parent = { postMessage: (m: Envelope) => sent.push(m) };
  const window: Record<string, unknown> = {
    parent,
    addEventListener: (type: string, fn: Listener) => (listeners[type] ??= []).push(fn),
    document: {
      documentElement: { style: { setProperty: (k: string, v: string) => (styles[k] = v) } },
    },
    setTimeout,
    clearTimeout,
    Promise,
    Error,
    Object,
  };
  // Le SDK est un script classique qui lit la variable globale `window`.
  new Function("window", source)(window);
  const deliver = (data: unknown, from: unknown = parent) =>
    (listeners.message ?? []).forEach((fn) => fn({ source: from, data }));
  const reply = (id: string, result: unknown) => deliver({ cadence: 1, kind: "res", id, result });
  const cadence = window.cadence as {
    ready: Promise<unknown>;
    route: { view: string };
    supports(m: string): boolean;
    call(tool: string, args?: unknown): Promise<unknown>;
    on(event: string, fn: (d: unknown) => void): () => void;
  };
  return { sent, styles, deliver, reply, cadence };
}

const hello = {
  api: 1,
  plugin: { id: "exemple", version: "0.1.0" },
  route: { view: "accueil", params: {} },
  theme: { "--color-bg": "#0b0d10" },
  locale: "fr-FR",
  methods: ["hello", "tools.call"],
  host: { version: "0.3" },
};

const flush = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => vi.useRealTimers());

describe("SDK des iframes", () => {
  it("attend la poignée de main avant tout autre appel", async () => {
    const { sent, reply, cadence, styles } = loadSdk();
    const pending = cadence.call("compteur", { pas: 1 });
    await flush();
    expect(sent.map((m) => (m.kind === "req" ? m.method : m.kind))).toEqual(["hello"]);

    reply(sent[0]!.kind === "req" ? sent[0]!.id : "", hello);
    await flush();
    expect(styles["--color-bg"]).toBe("#0b0d10");
    expect(cadence.supports("tools.call")).toBe(true);
    expect(cadence.supports("actions.run")).toBe(false);
    const call = sent[1];
    expect(call).toMatchObject({
      kind: "req",
      method: "tools.call",
      params: { tool: "compteur", args: { pas: 1 } },
    });
    reply(call!.kind === "req" ? call!.id : "", { valeur: 3 });
    await expect(pending).resolves.toEqual({ valeur: 3 });
  });

  it("rejette avec le code de l'erreur et ignore les messages d'autres fenêtres", async () => {
    const { sent, reply, deliver, cadence } = loadSdk();
    reply((sent[0] as { id: string }).id, hello);
    await cadence.ready;
    const failing = cadence.call("erreur");
    await flush();
    const id = (sent[1] as { id: string }).id;
    deliver({ cadence: 1, kind: "res", id, result: "usurpé" }, { autre: true });
    deliver({
      cadence: 1,
      kind: "res",
      id,
      error: { code: "tool_error", message: "Échec", data: { a: 1 } },
    });
    await expect(failing).rejects.toMatchObject({
      code: "tool_error",
      message: "Échec",
      data: { a: 1 },
    });
  });

  it("relaie les événements et met la route à jour", async () => {
    const { sent, reply, deliver, cadence } = loadSdk();
    reply((sent[0] as { id: string }).id, hello);
    await cadence.ready;
    const seen: unknown[] = [];
    const off = cadence.on("route", (d) => seen.push(d));
    deliver({ cadence: 1, kind: "evt", event: "route", data: { view: "outils", params: {} } });
    expect(cadence.route.view).toBe("outils");
    off();
    deliver({ cadence: 1, kind: "evt", event: "route", data: { view: "accueil", params: {} } });
    expect(seen).toHaveLength(1);
  });

  it("abandonne un appel sans réponse", async () => {
    vi.useFakeTimers();
    const { sent, reply, cadence } = loadSdk();
    reply((sent[0] as { id: string }).id, hello);
    await cadence.ready;
    const lost = cadence.call("lent");
    const check = expect(lost).rejects.toMatchObject({ code: "timeout" });
    await vi.advanceTimersByTimeAsync(131_000);
    await check;
  });
});
