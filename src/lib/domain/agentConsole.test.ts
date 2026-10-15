import { describe, expect, it } from "vitest";
import type { AgentOverview, AgentRunDetail, AgentRunEntry, CtoItem, DevTask } from "@/lib/types";
import {
  agentFooter,
  agentNote,
  agentStatus,
  consoleSubtitle,
  formatSpan,
  journalTime,
  latestByAgent,
  overviewOf,
  runHeadline,
  runLine,
  runProgress,
  runStatus,
  runningAgents,
  summarizeRun,
  usageOver,
  withReview,
} from "./agentConsole";
import { keys } from "@/lib/query/keys";

const detail = (patch: Partial<AgentRunDetail> = {}): AgentRunDetail => ({
  run: "cto-1",
  kind: "cto",
  state: "running",
  autonomy: "apply-after-review",
  version: "0.1.2",
  model: null,
  startedAt: "2026-10-15T08:40:00Z",
  endedAt: null,
  items: [],
  tasks: [],
  log: [],
  error: null,
  paused: false,
  ...patch,
});

const item = (number: number, state: CtoItem["state"], unanswered = 0): CtoItem => ({
  number,
  title: `Issue ${number}`,
  version: "0.1.2",
  issueType: null,
  state,
  before: "",
  after: null,
  questions: [
    ...Array.from({ length: unanswered }, (_, i) => ({ id: `q${i}`, question: "?", answer: null })),
    { id: "ok", question: "?", answer: "oui" },
  ],
  estimate: null,
  estimateHours: null,
  labels: [],
  risk: null,
  related: [],
  error: null,
});

const task = (number: number, state: DevTask["state"], step: number, pr: number | null): DevTask => ({
  number,
  title: `Tâche ${number}`,
  issueType: null,
  estimate: null,
  state,
  step,
  stepNotes: [],
  branch: `feat/${number}-x`,
  worktree: null,
  pr,
  prUrl: null,
  files: [],
  additions: 0,
  deletions: 0,
  summary: null,
  error: null,
  startedAt: "2026-10-15T09:00:00Z",
});

const entry = (patch: Partial<AgentRunEntry> = {}): AgentRunEntry => ({
  ...summarizeRun(detail()),
  ...patch,
});

const overview = (patch: Partial<AgentOverview> = {}): AgentOverview => ({
  kind: "cto",
  autonomy: "apply-after-review",
  activity: "idle",
  pendingActions: 0,
  currentRun: null,
  lastRunAt: null,
  ...patch,
});

describe("résumé d'exécution (miroir de Rust)", () => {
  it("compte la file de l'agent CTO", () => {
    const e = summarizeRun(
      detail({
        items: [
          item(151, "applied"),
          item(153, "ready"),
          item(156, "questions", 2),
          item(158, "running"),
          item(160, "queued"),
          item(161, "skipped"),
          item(162, "failed"),
        ],
      }),
    );
    expect([e.total, e.processed, e.pending, e.questions]).toEqual([7, 5, 2, 2]);
    expect([e.applied, e.skipped, e.failed]).toEqual([1, 1, 1]);
    expect(e.focus).toBe(158);
  });

  it("suit l'étape et la PR de l'agent développeur", () => {
    const d = detail({
      run: "dev-1",
      kind: "dev",
      tasks: [task(140, "done", 6, 87), task(142, "running", 3, 88), task(145, "queued", 0, null)],
    });
    const e = summarizeRun(d);
    expect([e.total, e.processed, e.applied, e.focus, e.step, e.pr]).toEqual([3, 1, 1, 142, 3, 88]);

    const o = overviewOf("dev", "apply-after-review", d);
    expect(o.activity).toBe("running");
    expect(o.currentRun).toMatchObject({ issue: 142, step: 3, steps: 6, startedAt: "2026-10-15T09:00:00Z" });
  });

  it("dérive l'activité : attente de validation, puis repos", () => {
    const waiting = overviewOf("cto", "apply-after-review", detail({ state: "review", items: [item(153, "ready")] }));
    expect([waiting.activity, waiting.pendingActions]).toEqual(["waiting", 1]);
    expect(waiting.currentRun?.step).toBe(1);

    const done = overviewOf("cto", "auto", detail({ state: "done", endedAt: "2026-10-15T09:10:00Z" }));
    expect([done.activity, done.currentRun, done.lastRunAt]).toEqual(["idle", null, "2026-10-15T09:10:00Z"]);
    expect(overviewOf("ceo", "propose", null).activity).toBe("idle");
  });
});

describe("barre latérale", () => {
  const running = overview({
    kind: "dev",
    activity: "running",
    currentRun: { run: "dev-1", state: "running", issue: 142, step: 3, steps: 6, startedAt: "" },
  });

  it("ne garde que les agents qui travaillent, dans l'ordre du rail", () => {
    const cto = overview({
      activity: "running",
      currentRun: { run: "cto-1", state: "running", issue: 156, step: 0, steps: 4, startedAt: "" },
    });
    const list = runningAgents([running, overview({ kind: "ceo", activity: "waiting" }), cto]);
    expect(list.map((a) => a.kind)).toEqual(["cto", "dev"]);
    expect(runningAgents([overview({ activity: "running" })])).toEqual([]);
  });

  it("écrit l'étape et le minuteur sans afficher de zéro", () => {
    expect(runLine("dev", running.currentRun!, 252)).toBe("#142 · étape 3/6 · 04:12");
    expect(runProgress("dev", { step: 3, steps: 6 })?.percent).toBeCloseTo(41.67, 1);
    expect(runLine("cto", { ...running.currentRun!, issue: 156, step: 0, steps: 4 }, 5)).toBe(
      "#156 · issue 1/4 · 00:05",
    );
    expect(runLine("dev", { ...running.currentRun!, issue: null, step: 0, steps: 0 }, 61)).toBe("01:01");
  });
});

describe("cartes de la console", () => {
  it("donne la pastille d'état", () => {
    const run = { run: "dev-1", state: "running" as const, issue: 142, step: 3, steps: 6, startedAt: "" };
    expect(agentStatus(overview({ activity: "running", currentRun: run }), 252)).toEqual({
      label: "#142 · 04:12",
      tone: "green",
      dot: true,
    });
    expect(agentStatus(overview({ activity: "running", currentRun: run }), 9, true).label).toBe("en pause");
    expect(agentStatus(overview({ activity: "waiting", pendingActions: 5 }), 0).label).toBe("5 à valider");
    expect(agentStatus(overview(), 0)).toEqual({ label: "inactif", tone: "neutral", dot: false });
  });

  it("résume ce qui attend", () => {
    expect(agentNote(overview(), entry({ questions: 2, pending: 1, focus: 156 }))).toBe(
      "2 questions sans réponse sur #156",
    );
    expect(agentNote(overview(), entry({ kind: "ceo", pending: 5, version: "0.2.0" }))).toBe(
      "5 issues proposées pour 0.2.0",
    );
    expect(
      agentNote(overview({ kind: "dev", activity: "running" }), entry({ kind: "dev", pr: 88 })),
    ).toBe("PR #88 ouverte en brouillon");
    expect(agentNote(overview(), entry({ state: "done" }))).toBeNull();
    expect(agentNote(overview(), null)).toBeNull();
  });

  it("écrit le pied de carte", () => {
    const now = new Date("2026-10-15T12:00:00Z");
    const dev = overview({
      kind: "dev",
      activity: "running",
      currentRun: { run: "dev-1", state: "running", issue: 142, step: 3, steps: 6, startedAt: "" },
    });
    expect(agentFooter(dev, null, now)).toBe("Étape 3/6 : implémentation");
    const last = entry({ state: "done", total: 18, processed: 12, endedAt: "2026-10-15T09:00:00Z" });
    // Intl sépare « 3 h » par une espace insécable.
    expect(agentFooter(overview(), last, now)).toMatch(/^Dernier passage : il y a 3\sh · 12\/18 issues$/);
    expect(agentFooter(overview(), null, now)).toBe("Jamais lancé sur ce dépôt");
  });

  it("compte agents en cours et actions en attente", () => {
    expect(
      consoleSubtitle([
        overview({ activity: "running" }),
        overview({ kind: "ceo", activity: "waiting", pendingActions: 5 }),
      ]),
    ).toBe("1 agent en cours · 5 actions en attente de validation");
    expect(consoleSubtitle([])).toBe("0 agent en cours · 0 action en attente de validation");
  });

  it("garde la dernière exécution de chaque agent", () => {
    const latest = latestByAgent([
      entry({ run: "cto-2" }),
      entry({ run: "dev-1", kind: "dev" }),
      entry({ run: "cto-1" }),
    ]);
    expect([latest.cto?.run, latest.dev?.run, latest.ceo]).toEqual(["cto-2", "dev-1", undefined]);
  });
});

describe("journal d'exécution", () => {
  it("donne l'état de chaque ligne", () => {
    expect(runStatus(entry())).toEqual({ label: "en cours", tone: "blue" });
    expect(runStatus(entry({ paused: true })).label).toBe("en pause");
    expect(runStatus(entry({ state: "review", pending: 2 }))).toEqual({ label: "à valider", tone: "amber" });
    expect(runStatus(entry({ state: "done", applied: 3 }))).toEqual({ label: "appliqué", tone: "green" });
    expect(runStatus(entry({ state: "done" })).label).toBe("terminé");
    expect(runStatus(entry({ state: "discarded" })).label).toBe("arrêté");
    expect(runStatus(entry({ state: "failed" }))).toEqual({ label: "échec", tone: "red" });
  });

  it("résume chaque exécution en une ligne", () => {
    expect(
      runHeadline(entry({ total: 18, processed: 12, applied: 10, questions: 2, pending: 3 })),
    ).toBe("Passage sur la version 0.1.2 · 12/18 traitées · 10 appliquées · 2 questions posées");
    expect(
      runHeadline(entry({ kind: "dev", version: null, total: 1, focus: 142, step: 3, pr: 88 })),
    ).toBe("Tâche #142 · étape 3/6 · PR #88");
    expect(runHeadline(entry({ kind: "dev", version: null, total: 2, processed: 2, applied: 2 }))).toBe(
      "2 tâches lancées · 2/2 traitées · 2 poussées",
    );
    expect(runHeadline(entry({ kind: "deploy", version: null, state: "failed", error: "tag pris" }))).toBe(
      "Exécution · tag pris",
    );
  });

  it("date les lignes comme la maquette", () => {
    const now = new Date(2026, 9, 15, 12, 0);
    expect(journalTime(new Date(2026, 9, 15, 11, 24).toISOString(), now)).toBe("11:24");
    expect(journalTime(new Date(2026, 9, 14, 18, 0).toISOString(), now)).toBe("hier");
    expect(journalTime(new Date(2026, 8, 12, 9, 0).toISOString(), now)).toBe("12 sept.");
  });
});

describe("consommation et garde-fous", () => {
  it("répartit le temps d'exécution des 7 derniers jours", () => {
    const now = new Date("2026-10-15T12:00:00Z");
    const usage = usageOver(
      [
        entry({ kind: "dev", startedAt: "2026-10-15T11:00:00Z", endedAt: null }),
        entry({ kind: "cto", startedAt: "2026-10-14T10:00:00Z", endedAt: "2026-10-14T10:20:00Z" }),
        entry({ kind: "cto", startedAt: "2026-10-01T10:00:00Z", endedAt: "2026-10-01T12:00:00Z" }),
      ],
      now,
    );
    expect(usage.runs).toBe(2);
    expect(usage.agents.map((a) => [a.kind, Math.round(a.percent)])).toEqual([
      ["dev", 75],
      ["cto", 25],
      ["ceo", 0],
      ["deploy", 0],
    ]);
    expect(formatSpan(usage.averageSeconds)).toBe("40 min");
    expect(usageOver([], now).agents.every((a) => a.percent === 0)).toBe(true);
  });

  it("écrit les durées en clair", () => {
    expect(formatSpan(42)).toBe("42 s");
    expect(formatSpan(400)).toBe("6 min 40 s");
    expect(formatSpan(3900)).toBe("1 h 05 min");
  });

  it("bascule la validation avant écriture sans toucher à l'agent CEO", () => {
    const current = { cto: "auto", ceo: "propose", dev: "apply-after-review", deploy: "propose" } as const;
    expect(withReview(current, true)).toEqual({
      cto: "apply-after-review",
      ceo: "propose",
      dev: "apply-after-review",
      deploy: "propose",
    });
    expect(withReview(current, false)).toEqual({ cto: "auto", ceo: "propose", dev: "auto", deploy: "propose" });
  });

  it("range le journal sous les clés d'agent, rafraîchies par agent://state", () => {
    expect(keys.agentRunsList(30).slice(0, 1)).toEqual([...keys.agents]);
    expect(keys.agentsOverview().slice(0, 1)).toEqual([...keys.agents]);
  });
});
