import { describe, expect, it } from "vitest";
import { mockPlugins, mockSettingsSchema, validateMockSettings } from "@/lib/ipc/mock/plugins";
import type { InstalledPluginView } from "@/lib/types";
import { mockCatalogView } from "@/lib/ipc/mock/catalog";
import {
  applyReposChanged,
  catalogCards,
  catalogTools,
  homeView,
  installedVersionNote,
  kitIcon,
  moveInRail,
  pendingSetup,
  pendingSetupLabel,
  railGroups,
  viewSections,
} from "./plugins";

const byId = (id: string) => mockPlugins.find((p) => p.id === id)!;

describe("railGroups", () => {
  it("groupe les plugins activés par catégorie, dans l'ordre fixe du rail", () => {
    const groups = railGroups([...mockPlugins].reverse());
    expect(groups.map((g) => g.label)).toEqual(["Projet", "Études", "Données", "Outils"]);
    // Ordre reçu gardé dans la catégorie ; le minuteur désactivé n'apparaît pas.
    expect(groups[1]!.plugins.map((p) => p.id)).toEqual(["bibliographie", "documents"]);
    expect(groups[3]!.plugins.map((p) => p.id)).toEqual(["exemple", "journal"]);
  });

  it("écarte un plugin invalide et n'affiche pas de catégorie vide", () => {
    const broken: InstalledPluginView = { ...byId("journal"), status: "invalid" };
    expect(railGroups([broken]).length).toBe(0);
  });
});

describe("viewSections", () => {
  it("regroupe les vues visibles par section, dans l'ordre du manifeste", () => {
    const sections = viewSections(byId("gestion-projet").views);
    expect(sections.map((s) => s.title)).toEqual(["Pilotage", "Travail"]);
    expect(sections[1]!.views.map((v) => v.id)).toEqual(["issues", "pull-requests", "reviews"]);
    expect(viewSections(byId("journal").views)).toEqual([
      { title: null, views: byId("journal").views },
    ]);
  });
});

describe("homeView", () => {
  it("prend la vue d'accueil, sinon la première vue visible", () => {
    expect(homeView(byId("gestion-projet"))).toBe("sprint");
    const noHome = {
      ...byId("documents"),
      views: byId("documents").views.map((v) => ({ ...v, home: false })),
    };
    expect(homeView(noHome)).toBe("bibliotheque");
    expect(homeView({ ...byId("journal"), views: [] })).toBeNull();
  });
});

describe("moveInRail", () => {
  it("échange avec le voisin de la même catégorie", () => {
    expect(moveInRail(mockPlugins, "bibliographie", -1)).toEqual([
      "gestion-projet",
      "bibliographie",
      "documents",
      "explorateur-sql",
      "journal",
      "exemple",
      "minuteur",
    ]);
    expect(moveInRail(mockPlugins, "documents", -1)).toBeNull();
    expect(moveInRail(mockPlugins, "gestion-projet", 1)).toBeNull();
    expect(moveInRail(mockPlugins, "absent", 1)).toBeNull();
  });
});

describe("kitIcon", () => {
  it("garde une icône du kit et remplace une icône inconnue", () => {
    expect(kitIcon("board")).toBe("board");
    expect(kitIcon("licorne")).toBe("layers");
  });
});

describe("validateMockSettings", () => {
  it("applique le même sous-ensemble de schéma que le cœur", () => {
    const schema = mockSettingsSchema["gestion-projet"]!;
    expect(validateMockSettings(schema, { sprintDays: 7, tagPrefix: "v" })).toEqual([]);
    expect(validateMockSettings(schema, { sprintDays: 99, autre: 1 }).map((e) => e.field)).toEqual([
      "autre",
      "sprintDays",
    ]);
    expect(validateMockSettings(schema, {})).toEqual([
      { field: "sprintDays", message: "Réglage obligatoire." },
    ]);
  });
});

describe("schemaFields", () => {
  it("rend les types du sous-ensemble, dans l'ordre demandé", async () => {
    const { schemaFields, parseSettingInput } = await import("./plugins");
    const fields = schemaFields({
      type: "object",
      required: ["sprintDays"],
      properties: {
        notes: { type: "string", format: "multiline", "x-cadence-order": 3 },
        sprintDays: {
          type: "integer",
          title: "Durée",
          minimum: 1,
          maximum: 60,
          "x-cadence-order": 1,
        },
        scheme: { type: "string", enum: ["semver", "calver"] },
        auto: { type: "boolean" },
        labels: { type: "array", items: { type: "string" } },
      },
    });
    expect(fields.map((f) => [f.key, f.kind])).toEqual([
      ["sprintDays", "integer"],
      ["notes", "multiline"],
      ["scheme", "enum"],
      ["auto", "boolean"],
    ]);
    expect(fields[0]).toMatchObject({ title: "Durée", required: true, min: 1, max: 60 });
    expect(fields[2]!.options).toEqual(["semver", "calver"]);
    expect(schemaFields(null)).toEqual([]);
    expect(parseSettingInput(fields[0]!, "7")).toBe(7);
    expect(parseSettingInput(fields[0]!, "")).toBeUndefined();
    expect(parseSettingInput(fields[0]!, "deux")).toBe("deux");
    expect(parseSettingInput(fields[3]!, true)).toBe(true);
  });
});

describe("catalogCards", () => {
  it("fusionne le catalogue et les plugins installés hors catalogue", () => {
    const view = mockCatalogView(mockPlugins, false, "2026-10-17T08:00:00Z");
    const cards = catalogCards(view.plugins, mockPlugins);
    const docs = cards.find((c) => c.id === "documents")!;
    expect(docs.entry?.updateAvailable).toBe(true);
    expect(docs.installed?.version).toBe("0.1.0");
    const exemple = cards.find((c) => c.id === "exemple")!;
    expect(exemple.entry).toBeNull();
    expect(cards.find((c) => c.id === "tests-couverture")!.installed).toBeNull();
    expect(cards.find((c) => c.id === "mobile-preview")!.entry?.compatible).toBe(false);
  });
});

describe("installedVersionNote (N03)", () => {
  it("se tait quand la version installée est celle du catalogue", () => {
    expect(installedVersionNote("0.6.0", { version: "0.6.0", source: "catalog" })).toBeNull();
  });

  it("signale une version plus récente installée en développement ou par archive", () => {
    expect(installedVersionNote("0.6.0", { version: "0.7.0", source: "dev" })).toBe(
      "v0.7.0 installée en développement, plus récente que le catalogue (v0.6.0)",
    );
    expect(installedVersionNote("0.6.0", { version: "0.7.0", source: "file" })).toContain(
      "depuis une archive locale, plus récente",
    );
    // Tri semver, pas lexical.
    expect(installedVersionNote("0.10.0", { version: "0.9.0", source: "catalog" })).toBe(
      "v0.9.0 installée depuis le catalogue · v0.10.0 au catalogue",
    );
  });
});

describe("pendingSetup (N03)", () => {
  const gp = byId("gestion-projet");

  it("propose le service Dépôts, les secrets puis les étapes [[setup]]", () => {
    const plugin = {
      ...gp,
      status: "needs-setup" as const,
      setup: [{ id: "depots", title: "Dépôts du plugin", view: "setup-repos", required: true }],
    };
    const pending = pendingSetup(plugin, { githubConnected: false, trackedRepos: [] });
    expect(pending.map((p) => p.kind)).toEqual(["github", "repos", "secrets", "view"]);
    expect(pending.map(pendingSetupLabel)).toEqual([
      "Connecter le compte GitHub",
      "Choisir les dépôts suivis",
      "Renseigner les secrets requis",
      "Dépôts du plugin",
    ]);
  });

  it("n'a rien à proposer quand tout est prêt, ni pour un plugin désactivé", () => {
    const ready = { ...gp, status: "ok" as const, setup: [] };
    expect(pendingSetup(ready, { githubConnected: true, trackedRepos: ["o/n"] })).toEqual([]);
    expect(
      pendingSetup({ ...ready, enabled: false }, { githubConnected: false, trackedRepos: [] }),
    ).toEqual([]);
  });
});

describe("catalogTools (P02)", () => {
  const catalog = { version: "0.6.0", tools: ["a", "b"] };

  it("montre les outils du serveur installé quand il tourne, et l'écart avec le catalogue", () => {
    const tools = catalogTools(
      catalog,
      { version: "0.7.0" },
      { state: "ready", toolNames: ["a", "b", "c"], uiTools: 3 },
    );
    expect(tools.names).toEqual(["a", "b", "c"]);
    expect(tools.uiTools).toBe(3);
    expect(tools.note).toBe(
      "Outils de la v0.7.0 installée ; le catalogue en annonce 2 pour la v0.6.0.",
    );
  });

  it("garde la liste du catalogue quand le serveur ne tourne pas", () => {
    const stopped = catalogTools(catalog, { version: "0.7.0" }, { state: "stopped" });
    expect(stopped.names).toEqual(["a", "b"]);
    expect(stopped.note).toContain("Liste du catalogue (v0.6.0)");
    expect(catalogTools(catalog, null, null)).toEqual({ names: ["a", "b"], uiTools: 0, note: null });
    const same = catalogTools(catalog, { version: "0.6.0" }, { state: "ready", toolNames: ["a", "b"] });
    expect(same.note).toBeNull();
  });
});

describe("applyReposChanged (N05)", () => {
  const settings = {
    trackedRepos: { autre: ["o/a"] } as Record<string, string[]>,
    repoSelection: {} as Record<string, string>,
    onboarded: true,
  };

  it("reporte un dépôt suivi depuis l'iframe du plugin dans les réglages en cache", () => {
    const next = applyReposChanged(settings, {
      plugin: "gestion-projet",
      repos: ["Remisdv/linear-test"],
      selection: "Remisdv/linear-test",
    });
    expect(next.trackedRepos).toEqual({ autre: ["o/a"], "gestion-projet": ["Remisdv/linear-test"] });
    expect(next.repoSelection["gestion-projet"]).toBe("Remisdv/linear-test");
    expect(next.onboarded).toBe(true);
    expect(settings.trackedRepos["gestion-projet"]).toBeUndefined();
  });

  it("retire la liste et la sélection vidées", () => {
    const next = applyReposChanged(
      { ...settings, repoSelection: { autre: "o/a" } },
      { plugin: "autre", repos: [], selection: null },
    );
    expect(next.trackedRepos).toEqual({});
    expect(next.repoSelection).toEqual({});
  });
});
