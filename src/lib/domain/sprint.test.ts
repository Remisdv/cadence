import { describe, expect, it } from "vitest";
import { issues, pullRequests } from "@/lib/ipc/mock/fixtures";
import type { IssueRow, ReviewRow } from "@/lib/types";
import {
  closeChecks,
  daysLeftLabel,
  delayLabel,
  gainedToday,
  pullStats,
  releaseNotes,
  requiredPace,
  reviewsToDo,
  sprintBoard,
  sprintTodos,
  statusMeters,
} from "./sprint";

const now = new Date("2026-10-15T12:00:00Z");
const hoursAgo = (h: number) => new Date(now.getTime() - h * 3_600_000).toISOString();
const issue = (p: Partial<IssueRow>): IssueRow => ({ ...issues[0]!, ...p });

describe("sprintBoard", () => {
  it("range par état et ne garde que les dernières issues faites", () => {
    const board = sprintBoard([...issues].reverse(), 2);
    expect(board.map((c) => c.status)).toEqual(["todo", "doing", "review", "done"]);
    expect(board.map((c) => c.total)).toEqual([4, 3, 4, 7]);
    expect(board[3]!.issues).toHaveLength(2);
    expect(board[0]!.issues.map((i) => i.number)).toEqual([151, 153, 156, 157]);
  });
});

describe("statusMeters", () => {
  it("donne fermées, en revue, en cours, à faire en part de la portée", () => {
    const meters = statusMeters(issues);
    expect(meters.map((m) => [m.label, m.value])).toEqual([
      ["Fermées", 7],
      ["En revue", 4],
      ["En cours", 3],
      ["À faire", 4],
    ]);
    expect(Math.round(meters[0]!.percent)).toBe(39);
  });
});

describe("KPI", () => {
  it("calcule le gain du jour, le retard et le rythme requis", () => {
    const points = [
      { date: "2026-09-15", remaining: 9 },
      { date: "2026-09-16", remaining: 7 },
    ];
    expect(gainedToday(points, 18)).toBe(11);
    expect(gainedToday(points.slice(0, 1), 18)).toBe(0);
    expect(delayLabel(1.5)).toEqual({ text: "1,5 j de retard", tone: "amber" });
    expect(delayLabel(-2)).toEqual({ text: "2 j d'avance", tone: "green" });
    expect(delayLabel(0.2).tone).toBe("tx3");
    expect(requiredPace(7, 6)).toBe(1.2);
    expect(requiredPace(7, 0)).toBeNull();
    expect(requiredPace(7, null)).toBeNull();
    expect(daysLeftLabel(6)).toBe("6 jours restants");
    expect(daysLeftLabel(1)).toBe("1 jour restant");
    expect(daysLeftLabel(0)).toBe("dernier jour");
    expect(daysLeftLabel(-2)).toBe("échéance dépassée de 2 j");
    expect(daysLeftLabel(null)).toBe("sans échéance");
  });

  it("compte les PR ouvertes et fusionnées", () => {
    expect(pullStats(pullRequests)).toEqual({ open: 4, merged: 2 });
    expect(pullStats([])).toEqual({ open: 0, merged: 0 });
  });

  it("garde les revues demandées à l'utilisateur et la plus ancienne", () => {
    const reviews: ReviewRow[] = [
      { pr: pullRequests[1]!, audience: "you", requestedAt: hoursAgo(6) },
      { pr: pullRequests[3]!, audience: "you", requestedAt: hoursAgo(50) },
      { pr: pullRequests[2]!, audience: "others", requestedAt: hoursAgo(80) },
    ];
    expect(reviewsToDo(reviews, now)).toEqual({ count: 2, oldest: "2 j" });
    expect(reviewsToDo([], now)).toEqual({ count: 0, oldest: null });
  });
});

describe("sprintTodos", () => {
  it("liste les revues en retard", () => {
    const reviews: ReviewRow[] = [
      { pr: pullRequests[1]!, audience: "you", requestedAt: hoursAgo(6) },
      { pr: pullRequests[3]!, audience: "you", requestedAt: hoursAgo(50) },
    ];
    expect(sprintTodos(reviews, now)).toEqual([
      { kind: "review", pr: 84, text: "PR #84 en attente de revue depuis 2 jours" },
    ]);
  });
});

describe("releaseNotes", () => {
  it("groupe les issues fermées par type", () => {
    const notes = releaseNotes(issues);
    expect(notes.map((s) => s.title)).toEqual([
      "Nouveautés",
      "Corrections",
      "Performances",
      "Documentation",
      "Maintenance",
    ]);
    expect(notes[1]!.items).toEqual(["Tri semver des milestones (#139)"]);
  });
});

describe("closeChecks", () => {
  it("bloque sur les issues ouvertes, les PR non fusionnées et la CI", () => {
    const checks = closeChecks(issues, pullRequests);
    expect(checks).toEqual([
      { ok: false, text: "11 issues encore ouvertes" },
      { ok: false, text: "4 PR non fusionnées" },
      { ok: false, text: "CI en échec sur 1 PR" },
    ]);
    expect(closeChecks([issue({ status: "done" })], [])).toEqual([
      { ok: true, text: "Toutes les issues sont fermées" },
      { ok: true, text: "Aucune PR en attente" },
    ]);
  });
});
