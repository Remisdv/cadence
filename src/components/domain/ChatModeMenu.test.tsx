import { afterEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { ChatModeMenu } from "./ChatModeMenu";
import { chatModeOrder, chatModes, defaultChatMode } from "@/lib/domain/chatModes";

describe("ChatModeMenu", () => {
  it("affiche le mode courant dans le composeur", () => {
    const html = renderToStaticMarkup(<ChatModeMenu value="auto-edit" onChange={() => {}} />);
    expect(html).toContain('aria-label="Mode de la conversation : Édition auto"');
    expect(html).toContain(">Édition auto<");
    expect(html).toContain(`title="${chatModes["auto-edit"].description}"`);
    expect(html).toContain('aria-haspopup="menu"');
  });

  it("propose les trois modes de Claude Code, « Demander » par défaut", () => {
    expect(chatModeOrder).toEqual(["ask", "auto-edit", "full"]);
    expect(chatModeOrder.map((m) => chatModes[m].label)).toEqual([
      "Demander",
      "Édition auto",
      "Auto",
    ]);
    expect(defaultChatMode).toBe("ask");
    const html = renderToStaticMarkup(<ChatModeMenu value={defaultChatMode} onChange={() => {}} />);
    expect(html).toContain(">Demander<");
  });

  it("peut être désactivé", () => {
    const html = renderToStaticMarkup(<ChatModeMenu value="full" onChange={() => {}} disabled />);
    expect(html).toContain("disabled");
    expect(html).toContain(">Auto<");
  });
});

describe("préférence « Commandes »", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  const storage = (initial: Record<string, string>) => {
    const data = new Map(Object.entries(initial));
    return {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
      data,
    };
  };

  it("est relue au lancement et gardée quand elle change", async () => {
    const local = storage({ "cadence.chat.showCommands": "false" });
    vi.stubGlobal("localStorage", local);
    const { useUiStore, SHOW_COMMANDS_KEY } = await import("@/stores/ui");
    expect(useUiStore.getState().showCommands).toBe(false);
    useUiStore.getState().setShowCommands(true);
    expect(useUiStore.getState().showCommands).toBe(true);
    expect(local.data.get(SHOW_COMMANDS_KEY)).toBe("true");
  });

  it("vaut « affichées » sans stockage", async () => {
    vi.stubGlobal("localStorage", undefined);
    const { useUiStore } = await import("@/stores/ui");
    expect(useUiStore.getState().showCommands).toBe(true);
    useUiStore.getState().setShowCommands(false);
    expect(useUiStore.getState().showCommands).toBe(false);
  });
});
