/**
 * Statut des agents et journal des exécutions en mode mock, calculés à partir des simulations
 * (`cto.ts`, `dev.ts`) avec les mêmes règles que Rust. Un agent qui n'a encore rien lancé
 * dans la session garde les fixtures (statut et historique des maquettes).
 */
import type { AgentKind, AgentOverview, AgentRunDetail, AgentRunEntry } from "@/lib/types";
import { agentOrder } from "@/lib/domain/agents";
import { overviewOf, summarizeRun } from "@/lib/domain/agentConsole";
import * as data from "./fixtures";
import { ctoMock } from "./cto";
import { devMock } from "./dev";

/** Simulation d'un agent : dernière exécution et historique de la session. */
interface RunSource {
  latest: () => AgentRunDetail | null;
  runs: () => AgentRunDetail[];
}

const sources: Partial<Record<AgentKind, RunSource>> = { cto: ctoMock, dev: devMock };

const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

const entry = (e: Partial<AgentRunEntry> & Pick<AgentRunEntry, "run" | "kind" | "state" | "startedAt">): AgentRunEntry => ({
  autonomy: data.config.agents[e.kind].autonomy,
  version: null,
  endedAt: null,
  paused: false,
  total: 0,
  processed: 0,
  pending: 0,
  questions: 0,
  applied: 0,
  skipped: 0,
  failed: 0,
  focus: null,
  step: null,
  pr: null,
  error: null,
  ...e,
});

/** Historique des maquettes (console des agents), relatif à maintenant. */
const fixtureRuns: AgentRunEntry[] = [
  // Cohérent avec la fixture du statut : #142, étape 3/6, démarrée il y a 4 min 12 s.
  entry({
    run: "run-dev-142",
    kind: "dev",
    state: "running",
    startedAt: data.agentsOverview.find((a) => a.kind === "dev")?.currentRun?.startedAt ?? ago(4),
    total: 1,
    focus: 142,
    step: 3,
    pr: 88,
  }),
  entry({
    run: "run-ceo-020",
    kind: "ceo",
    state: "review",
    version: "0.2.0",
    startedAt: ago(62),
    endedAt: ago(58),
    total: 0,
    pending: 5,
  }),
  entry({
    run: "run-cto-012",
    kind: "cto",
    state: "done",
    version: "0.1.2",
    startedAt: ago(185),
    endedAt: ago(170),
    total: 18,
    processed: 18,
    applied: 12,
    skipped: 6,
  }),
  entry({
    run: "run-deploy-011",
    kind: "deploy",
    state: "done",
    version: "0.1.1",
    startedAt: ago(26 * 60 + 9),
    endedAt: ago(26 * 60),
    total: 0,
    applied: 1,
  }),
  entry({
    run: "run-cto-011",
    kind: "cto",
    state: "done",
    version: "0.1.1",
    startedAt: ago(3 * 24 * 60),
    endedAt: ago(3 * 24 * 60 - 11),
    total: 9,
    processed: 9,
    applied: 9,
  }),
  entry({
    run: "run-dev-131",
    kind: "dev",
    state: "done",
    startedAt: ago(4 * 24 * 60),
    endedAt: ago(4 * 24 * 60 - 38),
    total: 2,
    processed: 2,
    applied: 2,
    pr: 84,
  }),
];

/** L'agent a-t-il déjà tourné dans la session ? Sinon, ses fixtures font foi. */
const live = (kind: AgentKind) => (sources[kind]?.runs().length ?? 0) > 0;

export const agentsMock = {
  overview(): AgentOverview[] {
    return agentOrder.map((kind) => {
      const autonomy = data.config.agents[kind].autonomy;
      const source = sources[kind];
      if (source && live(kind)) return overviewOf(kind, autonomy, source.latest());
      const fixture = data.agentsOverview.find((a) => a.kind === kind);
      const last = fixtureRuns.find((r) => r.kind === kind);
      return {
        ...(fixture ?? overviewOf(kind, autonomy, null)),
        autonomy,
        lastRunAt: last ? (last.endedAt ?? last.startedAt) : (fixture?.lastRunAt ?? null),
      };
    });
  },

  runs(limit: number): AgentRunEntry[] {
    const all = agentOrder.flatMap((kind) =>
      live(kind)
        ? sources[kind]!.runs().map(summarizeRun)
        : fixtureRuns.filter((r) => r.kind === kind),
    );
    return all
      .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
      .slice(0, Math.max(1, Math.min(500, limit)));
  },
};
