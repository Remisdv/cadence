import { describe, expect, it } from "vitest";
import type { IssueRow } from "@/lib/types";
import { issues, versions } from "@/lib/ipc/mock/fixtures";
import { groupByVersion, matchesView, type IssueViewFilter } from "./issueGroups";

const all: IssueViewFilter = { query: "", versions: [], state: "all", assignee: "all" };

const extra: IssueRow[] = [
  { ...issues[0]!, number: 900, version: "0.1.3", status: "todo" },
  { ...issues[0]!, number: 901, version: "0.1.0", status: "done" },
  { ...issues[0]!, number: 902, version: null, status: "todo" },
];
const rows = [...issues, ...extra];

describe("groupByVersion", () => {
  it("met le sprint en cours d'abord, les versions livrées ensuite, puis sans version", () => {
    const groups = groupByVersion(rows, rows, versions, "0.1.2");
    expect(groups.map((g) => g.version)).toEqual(["0.1.2", "0.1.3", "0.1.0", null]);
    expect(groups.map((g) => g.label)).toEqual([
      "Sprint en cours",
      "Sprint suivant",
      "Livrée",
      "Sans version",
    ]);
  });

  it("compte l'avancement sur toute la version, filtres ignorés", () => {
    const openOnly = rows.filter((r) => matchesView(r, { ...all, state: "open" }));
    const [current] = groupByVersion(rows, openOnly, versions, "0.1.2");
    expect(current!.total).toBe(18);
    expect(current!.open).toBe(11);
    expect(Math.round(current!.progress)).toBe(39);
    expect(current!.rows.every((r) => r.status !== "done")).toBe(true);
  });

  it("masque les groupes vidés par les filtres", () => {
    const visible = rows.filter((r) => matchesView(r, { ...all, versions: ["0.1.3"] }));
    expect(groupByVersion(rows, visible, versions, "0.1.2").map((g) => g.version)).toEqual([
      "0.1.3",
    ]);
  });
});

describe("matchesView", () => {
  it("cherche par titre, numéro, label ou assigné", () => {
    const quota = issues.find((i) => i.number === 156)!;
    expect(matchesView(quota, { ...all, query: "quota" })).toBe(true);
    expect(matchesView(quota, { ...all, query: "#156" })).toBe(true);
    expect(matchesView(quota, { ...all, query: "area/api" })).toBe(true);
    expect(matchesView(quota, { ...all, query: "roadmap" })).toBe(false);
  });

  it("filtre sur mes issues et sur « sans version »", () => {
    const mine = issues.filter((i) => matchesView(i, { ...all, assignee: "me" }));
    expect(mine.every((i) => i.assignee?.isSelf)).toBe(true);
    expect(matchesView(extra[2]!, { ...all, versions: [""] })).toBe(true);
  });
});
