import { describe, expect, it } from "vitest";
import type { DeployState, DeployTest } from "@/lib/types";
import { releaseNotes } from "@/lib/ipc/mock/deploy";
import {
  deploySubtitle,
  formatSize,
  formatTestDuration,
  inNotes,
  needsDecision,
  noteLines,
  pipelineStages,
  testCounts,
} from "./deploy";

const test = (
  number: number,
  status: DeployTest["status"],
  extra: Partial<DeployTest> = {},
): DeployTest => ({
  number,
  title: `Issue ${number}`,
  issueType: "feat",
  criteria: ["ça marche"],
  status,
  detail: null,
  durationMs: null,
  decision: null,
  ...extra,
});

const state = (extra: Partial<DeployState> = {}): DeployState => ({
  tag: "v0.1.1",
  name: "Lecture du projet",
  nextVersion: "0.1.2",
  stages: [
    { state: "success", detail: "14 issues fermées" },
    { state: "success", detail: "macOS arm64" },
    { state: "failure", detail: "13 / 14 · 1 échec sur #128" },
    { state: "running", detail: "rédaction…" },
    { state: "pending", detail: null },
  ],
  step: 4,
  tests: [test(128, "failed"), test(127, "passed"), test(125, "passed")],
  artifacts: [],
  notes: null,
  notesEdited: false,
  releaseUrl: null,
  published: false,
  worktree: null,
  ...extra,
});

describe("pipelineStages", () => {
  it("garde ✓ / ✗ pour une étape finie et une icône métier sinon", () => {
    const stages = pipelineStages(state());
    expect(stages.map((s) => s.state)).toEqual([
      "success",
      "success",
      "failure",
      "running",
      "pending",
    ]);
    expect(stages.map((s) => s.icon)).toEqual([undefined, undefined, undefined, "book", "tag"]);
    expect(stages[2]!.title).toBe("Tests de vérification");
  });
});

describe("verdicts", () => {
  it("compte les tests et repère les échecs à trancher", () => {
    const tests = [
      test(1, "passed"),
      test(2, "failed"),
      test(3, "failed", { decision: "ignored" }),
      test(4, "unverifiable"),
    ];
    expect(testCounts(tests)).toEqual({ total: 4, passed: 1, failed: 2, unverifiable: 1 });
    expect(tests.filter(needsDecision).map((t) => t.number)).toEqual([2]);
    expect(tests.filter(inNotes).map((t) => t.number)).toEqual([1, 3, 4]);
  });

  it("résume l'exécution dans l'en-tête", () => {
    expect(deploySubtitle(state())).toBe("Étape 4/5 · Documentation · 2 tests sur 3");
    expect(deploySubtitle(state({ step: 5, published: true, notes: "## v0.1.1" }))).toBe(
      "Release publiée · documentation à jour · 2 tests sur 3",
    );
  });
});

describe("notes de version", () => {
  it("rédige comme Rust : sections par type, sans les échecs non ignorés", () => {
    const notes = releaseNotes("v0.1.1", "Lecture du projet", [
      test(128, "failed"),
      test(127, "passed"),
      test(120, "passed", { issueType: "fix" }),
      test(110, "passed", { issueType: null }),
    ]);
    expect(notes).toBe(
      "## v0.1.1 — Lecture du projet\n\n### Nouveautés\n- Issue 127 (#127)\n\n### Corrections\n- Issue 120 (#120)\n\n### Maintenance\n- Issue 110 (#110)\n",
    );
    expect(noteLines(notes)).toEqual([
      { kind: "title", text: "## v0.1.1 — Lecture du projet" },
      { kind: "section", text: "Nouveautés" },
      { kind: "item", text: "Issue 127 (#127)" },
      { kind: "section", text: "Corrections" },
      { kind: "item", text: "Issue 120 (#120)" },
      { kind: "section", text: "Maintenance" },
      { kind: "item", text: "Issue 110 (#110)" },
    ]);
  });
});

describe("formats", () => {
  it("écrit durées et tailles à la française", () => {
    expect(formatTestDuration(800)).toBe("0,8 s");
    expect(formatTestDuration(125_000)).toBe("2 min 05 s");
    expect(formatTestDuration(null)).toBe("");
    expect(formatSize(18_842)).toBe("18,4 Mo");
    expect(formatSize(512)).toBe("512 Ko");
  });
});
