import { describe, expect, it } from "vitest";
import { mockPlugins, mockSettingsSchema, validateMockSettings } from "@/lib/ipc/mock/plugins";
import type { InstalledPluginView } from "@/lib/types";
import { mockCatalogView } from "@/lib/ipc/mock/catalog";
import { catalogCards, homeView, kitIcon, moveInRail, railGroups, viewSections } from "./plugins";

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
