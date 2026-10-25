import { describe, expect, it } from "vitest";
import type { InstalledPluginView } from "@/lib/types";
import { formatSize, nextStep, previousStep, setupTasks, taskTitle } from "./onboarding";

function plugin(patch: Partial<InstalledPluginView>): InstalledPluginView {
  return {
    id: "p",
    name: "P",
    version: "0.1.0",
    description: "",
    category: "outils",
    color: "blue",
    railIcon: "layers",
    views: [],
    secrets: [],
    services: [],
    setup: [],
    actions: [],
    skills: [],
    hasSettings: false,
    uiEntry: null,
    uiDevUrl: null,
    enabled: true,
    source: "catalog",
    status: "ok",
    problem: null,
    ...patch,
  };
}

describe("onboarding", () => {
  it("enchaîne les étapes", () => {
    expect(nextStep("welcome")).toBe("ai");
    expect(nextStep("tools")).toBe("setup");
    expect(previousStep("ai")).toBe("welcome");
    expect(previousStep("setup")).toBe("tools");
  });

  it("intercale la connexion GitHub au premier plugin qui utilise le service", () => {
    const tasks = setupTasks([
      plugin({
        id: "gp",
        services: ["repos"],
        secrets: [{ id: "opt", label: "Optionnel", description: null, required: false }],
        setup: [{ id: "depot", title: "Dépôt par défaut", view: "setup", required: false }],
      }),
      plugin({ id: "off", enabled: false, services: ["repos"] }),
      plugin({ id: "autre", services: ["repos"] }),
    ]);
    expect(tasks.map((t) => `${t.plugin.id}:${t.kind}`)).toEqual([
      "gp:github",
      "gp:repos",
      "gp:secret",
      "gp:view",
      "autre:repos",
    ]);
    expect(taskTitle(tasks[0]!)).toBe("Compte GitHub");
    expect(taskTitle(tasks[3]!)).toBe("Dépôt par défaut");
  });

  it("formate la taille des archives", () => {
    expect(formatSize(31_200_000)).toBe("31,2 Mo");
    expect(formatSize(420_000)).toBe("420 ko");
  });
});
