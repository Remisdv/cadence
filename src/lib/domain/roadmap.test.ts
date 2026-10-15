import { describe, expect, it } from "vitest";
import { issues, versions } from "@/lib/ipc/mock/fixtures";
import type { Version } from "@/lib/types";
import { buildRoadmap, daysUntil, periodOf, themesOf } from "./roadmap";

describe("buildRoadmap", () => {
  it("sépare livré, en cours et planifié (sprints puis paliers)", () => {
    const roadmap = buildRoadmap(versions, issues, "0.1.2");
    expect(roadmap.shipped.map((c) => c.version.title)).toEqual(["0.1.0", "0.1.1"]);
    expect(roadmap.current?.version.title).toBe("0.1.2");
    expect(roadmap.planned.map((c) => [c.version.title, c.stage])).toEqual([
      ["0.1.3", "planned"],
      ["0.2.0", "milestone"],
      ["1.0.0", "vision"],
    ]);
  });

  it("montre les issues ouvertes d'abord, ou les thèmes d'un palier sans issue", () => {
    const { current, planned } = buildRoadmap(versions, issues, "0.1.2");
    expect(current!.items).toHaveLength(3);
    expect(current!.items[0]!.done).toBe(false);
    expect(Math.round(current!.progress)).toBe(61);
    const palier = planned.find((c) => c.version.title === "0.2.0")!;
    expect(palier.themesOnly).toBe(true);
    expect(palier.items.map((i) => i.text)).toEqual(["Rôles et permissions", "Revue assistée", "Multi-dépôts"]);
  });

  it("ne garde que les dernières versions livrées", () => {
    const many: Version[] = ["0.0.1", "0.0.2", "0.0.3", "0.0.4"].map((title, i) => ({
      ...versions[0]!,
      title,
      milestoneNumber: 100 + i,
    }));
    const roadmap = buildRoadmap(many, [], null, 3);
    expect(roadmap.shipped.map((c) => c.version.title)).toEqual(["0.0.2", "0.0.3", "0.0.4"]);
    expect(roadmap.olderShipped).toBe(1);
  });
});

describe("outils", () => {
  it("lit les thèmes, les jours restants et la période", () => {
    expect(themesOf("# Titre\n- Un\n* Deux\ntexte\n+ Trois")).toEqual(["Un", "Deux", "Trois"]);
    expect(themesOf(null)).toEqual([]);
    expect(daysUntil("2026-09-23T07:00:00Z", new Date("2026-09-17T12:00:00Z"))).toBe(6);
    expect(daysUntil(null)).toBeNull();
    expect(periodOf(versions)).toBe("Août 2026 → oct. 2026");
  });
});
