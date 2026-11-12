import { describe, expect, it } from "vitest";
import type { Repo } from "@/lib/types";
import { RECENT_REPOS, activateRepo, activeRepo, mergeRepoRows } from "./repos";

describe("dépôt actif d'un plugin", () => {
  it("met le dépôt activé en tête, sans doublon", () => {
    expect(activateRepo(["me/a", "me/b"], "me/b")).toEqual(["me/b", "me/a"]);
    expect(activateRepo(["me/a"], "ME/A")).toEqual(["ME/A"]);
    expect(activateRepo([], "me/a")).toEqual(["me/a"]);
  });

  it("ne retient que les dépôts récents", () => {
    const many = Array.from({ length: RECENT_REPOS + 3 }, (_, i) => `me/r${i}`);
    const next = activateRepo(many, "me/neuf");
    expect(next).toHaveLength(RECENT_REPOS);
    expect(next[0]).toBe("me/neuf");
  });

  it("retombe sur le premier dépôt suivi, jamais sur « tous »", () => {
    expect(activeRepo(["me/a", "me/b"], "me/b")).toBe("me/b");
    expect(activeRepo(["me/a", "me/b"], null)).toBe("me/a");
    expect(activeRepo(["me/a"], "me/retire")).toBe("me/a");
    expect(activeRepo([], null)).toBeNull();
  });
});

describe("lignes du sélecteur de dépôt", () => {
  const repo = (fullName: string): Repo => ({
    fullName,
    owner: fullName.split("/")[0]!,
    name: fullName.split("/")[1]!,
    description: `desc ${fullName}`,
    private: true,
    defaultBranch: "main",
    openIssues: 0,
    milestones: 0,
    pushedAt: null,
  });

  it("met les récents en tête, puis le reste du compte sans doublon", () => {
    const rows = mergeRepoRows(["me/b"], [repo("me/a"), repo("ME/B")], "");
    expect(rows.map((r) => [r.fullName, r.recent])).toEqual([
      ["me/b", true],
      ["me/a", false],
    ]);
    expect(rows[0]!.description).toBe("desc ME/B");
  });

  it("filtre les récents par la recherche", () => {
    expect(mergeRepoRows(["me/site", "me/app"], [], "app").map((r) => r.fullName)).toEqual(["me/app"]);
  });
});
