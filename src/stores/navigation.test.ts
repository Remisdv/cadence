import { describe, expect, it } from "vitest";
import { parseStoredView } from "./navigation";

const known = (id: string) => ["chat", "plugin", "settings", "onboarding"].includes(id);

describe("vue gardée pour un rechargement (recette P13)", () => {
  it("rouvre la conversation ouverte, celle d'un sprint comprise", () => {
    const raw = JSON.stringify({ id: "chat", session: "c1a15d" });
    expect(parseStoredView(raw, known)).toEqual({ id: "chat", session: "c1a15d" });
    const plugin = { id: "plugin", plugin: "gestion-projet", view: "sprint", params: {} };
    expect(parseStoredView(JSON.stringify(plugin), known)).toEqual(plugin);
  });

  it("ignore une valeur absente, illisible, inconnue ou de l'onboarding", () => {
    expect(parseStoredView(null, known)).toBeNull();
    expect(parseStoredView("{", known)).toBeNull();
    expect(parseStoredView("[]", known)).toBeNull();
    expect(parseStoredView(JSON.stringify({ id: "inconnue" }), known)).toBeNull();
    expect(parseStoredView(JSON.stringify({ id: "onboarding", step: "ai" }), known)).toBeNull();
  });
});
