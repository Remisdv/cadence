/**
 * Console des agents et statut du rail : résumés d'exécution, libellés d'état, durées.
 * Logique pure, partagée par l'écran, la barre latérale et le mock (même calcul que Rust :
 * `domain/agent_history.rs` et `commands::agents::agents_overview`).
 */
import type {
  AgentKind,
  AgentOverview,
  AgentRunDetail,
  AgentRunEntry,
  Autonomy,
  RunState,
  RunSummary,
} from "@/lib/types";
import type { BadgeTone } from "@/components/ui/Badge";
import { agentOrder, devSteps } from "./agents";
import { formatAgo, formatDay, formatDuration } from "./format";

/** Ce que fait chaque agent (paragraphe des cartes de la console). */
export const agentDescriptions: Record<AgentKind, string> = {
  cto: "Parcourt les issues d'une version, complète les descriptions à partir du titre et du code, et pose les questions qui manquent pour rendre la tâche exécutable.",
  ceo: "Lit la roadmap et les versions livrées, puis dérive les issues probables des paliers à venir. Rien n'est créé sans votre accord.",
  dev: "Vous demande quelles tâches lancer, crée la branche et la PR en brouillon, puis implémente dans un worktree isolé.",
  deploy:
    "Publie la version, rédige la documentation et rejoue un test d'acceptation par issue pour vérifier que la version est bien déployée.",
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n > 1 ? many : one}`;

/** Exécution qui travaille encore (la file avance). */
export const isActive = (state: RunState) => state === "prepare" || state === "running";

const isTerminal = (state: RunState) =>
  state === "done" || state === "discarded" || state === "failed";

/* ---------- Résumés (mêmes règles que Rust) ---------- */

/** Ligne du journal pour une exécution (miroir de `AgentRunEntry::of`). */
export function summarizeRun(d: AgentRunDetail): AgentRunEntry {
  const { items, tasks } = d;
  const awaits = (s: string) => s === "ready" || s === "questions";
  const runningItem = items.find((i) => i.state === "running");
  const runningTask = tasks.find((t) => t.state === "running");
  const focus =
    runningTask?.number ??
    runningItem?.number ??
    tasks.find((t) => t.state === "review")?.number ??
    items.find((i) => awaits(i.state))?.number ??
    null;
  const count = <T>(list: T[], pred: (x: T) => boolean) => list.filter(pred).length;
  return {
    run: d.run,
    kind: d.kind,
    state: d.state,
    autonomy: d.autonomy,
    version: d.version,
    startedAt: d.startedAt,
    endedAt: d.endedAt,
    paused: d.paused,
    total: items.length + tasks.length,
    processed:
      count(items, (i) => i.state !== "queued" && i.state !== "running") +
      count(tasks, (t) => t.state !== "queued" && t.state !== "running"),
    pending: count(items, (i) => awaits(i.state)) + count(tasks, (t) => t.state === "review"),
    questions: items
      .filter((i) => i.state === "questions")
      .reduce((n, i) => n + count(i.questions, (q) => q.answer === null), 0),
    applied: count(items, (i) => i.state === "applied") + count(tasks, (t) => t.state === "done"),
    skipped: count(items, (i) => i.state === "skipped") + count(tasks, (t) => t.state === "skipped"),
    failed: count(items, (i) => i.state === "failed") + count(tasks, (t) => t.state === "failed"),
    focus,
    step: runningTask?.step ?? null,
    pr: [...tasks].reverse().find((t) => t.pr !== null)?.pr ?? null,
    error: d.error,
  };
}

/** Statut d'un agent d'après sa dernière exécution (miroir de `agents_overview`). */
export function overviewOf(
  kind: AgentKind,
  autonomy: Autonomy,
  d: AgentRunDetail | null,
): AgentOverview {
  if (!d) {
    return { kind, autonomy, activity: "idle", pendingActions: 0, currentRun: null, lastRunAt: null };
  }
  const e = summarizeRun(d);
  const devTask = d.tasks.find((t) => t.state === "running");
  const running =
    isActive(d.state) ||
    d.items.some((i) => i.state === "running") ||
    devTask !== undefined;
  const done = d.items.filter((i) => i.state !== "queued" && i.state !== "running").length;
  const currentRun: RunSummary | null = isTerminal(d.state)
    ? null
    : {
        run: d.run,
        state: d.state,
        issue: devTask?.number ?? d.items.find((i) => i.state === "running")?.number ?? null,
        step: devTask ? devTask.step : done,
        steps: devTask ? devSteps.length : d.items.length,
        startedAt: devTask?.startedAt ?? d.startedAt,
      };
  return {
    kind,
    autonomy,
    activity: running ? "running" : e.pending > 0 ? "waiting" : "idle",
    pendingActions: e.pending,
    currentRun,
    lastRunAt: d.endedAt ?? d.startedAt,
  };
}

/* ---------- Barre latérale ---------- */

/** Agents qui travaillent, dans l'ordre du rail. */
export function runningAgents(overview: AgentOverview[]): AgentOverview[] {
  return agentOrder.flatMap((kind) =>
    overview.filter((a) => a.kind === kind && a.activity === "running" && a.currentRun !== null),
  );
}

/**
 * Avancement d'une exécution en cours : étape de la tâche (développeur) ou issue examinée
 * (CTO, `step` = issues déjà traitées). `null` sans compteur exploitable.
 */
export function runProgress(
  kind: AgentKind,
  run: Pick<RunSummary, "step" | "steps">,
): { label: string; percent: number } | null {
  const { step, steps } = run;
  if (!steps || step === null) return null;
  if (kind === "dev") {
    if (step < 1) return null;
    return { label: `étape ${step}/${steps}`, percent: ((step - 0.5) / steps) * 100 };
  }
  const current = Math.min(step + 1, steps);
  return { label: `issue ${current}/${steps}`, percent: (step / steps) * 100 };
}

/** « #142 · étape 3/6 · 04:12 » sous la carte « Agent … en cours ». */
export function runLine(kind: AgentKind, run: RunSummary, elapsedSeconds: number): string {
  const parts: string[] = [];
  if (run.issue !== null) parts.push(`#${run.issue}`);
  const progress = runProgress(kind, run);
  if (progress) parts.push(progress.label);
  parts.push(formatDuration(elapsedSeconds));
  return parts.join(" · ");
}

/* ---------- Cartes de la console ---------- */

export interface StatusLabel {
  label: string;
  tone: BadgeTone;
  dot: boolean;
}

/** Pastille d'une carte d'agent : « inactif », « 5 à valider », « #142 · 04:12 ». */
export function agentStatus(
  agent: AgentOverview,
  elapsedSeconds: number,
  paused = false,
): StatusLabel {
  if (agent.activity === "running") {
    if (paused) return { label: "en pause", tone: "neutral", dot: true };
    const issue = agent.currentRun?.issue;
    const time = formatDuration(elapsedSeconds);
    return { label: issue ? `#${issue} · ${time}` : time, tone: "green", dot: true };
  }
  if (agent.activity === "waiting") {
    return { label: `${agent.pendingActions} à valider`, tone: "amber", dot: true };
  }
  return { label: "inactif", tone: "neutral", dot: false };
}

/** Bordure de la carte : ambre quand des actions attendent, verte quand l'agent travaille. */
export function agentCardTone(agent: AgentOverview): "amber" | "green" | null {
  return agent.activity === "running" ? "green" : agent.activity === "waiting" ? "amber" : null;
}

/** Encadré de la carte : ce qui attend ou ce qui se passe. `null` : rien à signaler. */
export function agentNote(agent: AgentOverview, last: AgentRunEntry | null): string | null {
  if (!last) return null;
  if (last.questions > 0) {
    const on = last.focus !== null ? ` sur #${last.focus}` : "";
    return `${plural(last.questions, "question")} sans réponse${on}`;
  }
  if (last.pending > 0) {
    const what =
      last.kind === "dev"
        ? plural(last.pending, "push en attente", "pushs en attente")
        : last.kind === "ceo"
          ? plural(last.pending, "issue proposée", "issues proposées")
          : plural(last.pending, "proposition à relire", "propositions à relire");
    return last.version ? `${what} pour ${last.version}` : what;
  }
  if (agent.activity === "running") {
    if (last.kind === "dev" && last.pr !== null) return `PR #${last.pr} ouverte en brouillon`;
    if (last.focus !== null) return `#${last.focus} en cours d'examen`;
  }
  if (last.state === "failed" && last.error) return last.error;
  return null;
}

const unitOf: Partial<Record<AgentKind, [string, string]>> = {
  cto: ["issue", "issues"],
  dev: ["tâche", "tâches"],
};

/** Pied de carte : étape en cours, sinon dernier passage. */
export function agentFooter(
  agent: AgentOverview,
  last: AgentRunEntry | null,
  now: Date = new Date(),
): string {
  const run = agent.currentRun;
  if (agent.activity === "running" && run) {
    if (agent.kind === "dev" && run.step && run.steps) {
      const name = devSteps[run.step - 1];
      return `Étape ${run.step}/${run.steps}${name ? ` : ${name.charAt(0).toLowerCase()}${name.slice(1)}` : ""}`;
    }
    const progress = runProgress(agent.kind, run);
    if (progress) return progress.label.charAt(0).toUpperCase() + progress.label.slice(1);
    return "En cours";
  }
  const at = last ? (last.endedAt ?? last.startedAt) : agent.lastRunAt;
  if (!at) return "Jamais lancé sur ce dépôt";
  const parts = [`Dernier passage : ${formatAgo(at, now)}`];
  if (last && last.total > 0) {
    const [one, many] = unitOf[last.kind] ?? ["élément", "éléments"];
    parts.push(`${last.processed}/${last.total} ${last.total > 1 ? many : one}`);
  } else if (last?.version) {
    parts.push(`cible ${last.version}`);
  }
  return parts.join(" · ");
}

/** Dernière exécution de chaque agent dans le journal (trié du plus récent au plus ancien). */
export function latestByAgent(entries: AgentRunEntry[]): Partial<Record<AgentKind, AgentRunEntry>> {
  const out: Partial<Record<AgentKind, AgentRunEntry>> = {};
  for (const e of entries) out[e.kind] ??= e;
  return out;
}

/** « 1 agent en cours · 5 actions en attente de validation » */
export function consoleSubtitle(overview: AgentOverview[]): string {
  const running = overview.filter((a) => a.activity === "running").length;
  const pending = overview.reduce((n, a) => n + a.pendingActions, 0);
  return [
    `${plural(running, "agent")} en cours`,
    `${plural(pending, "action")} en attente de validation`,
  ].join(" · ");
}

/* ---------- Journal d'exécution ---------- */

/** Pastille d'une ligne du journal. */
export function runStatus(e: AgentRunEntry): Omit<StatusLabel, "dot"> {
  switch (e.state) {
    case "prepare":
    case "running":
      return e.paused ? { label: "en pause", tone: "neutral" } : { label: "en cours", tone: "blue" };
    case "proposing":
    case "review":
      return e.pending > 0
        ? { label: "à valider", tone: "amber" }
        : { label: "en relecture", tone: "neutral" };
    case "applying":
      return { label: "écriture", tone: "blue" };
    case "suspended":
      return { label: "suspendu", tone: "neutral" };
    case "done":
      return e.applied > 0
        ? { label: "appliqué", tone: "green" }
        : { label: "terminé", tone: "neutral" };
    case "discarded":
      return { label: "arrêté", tone: "neutral" };
    case "failed":
      return { label: "échec", tone: "red" };
  }
}

/** Texte d'une ligne du journal : sur quoi l'agent a travaillé, puis ce qu'il en reste. */
export function runHeadline(e: AgentRunEntry): string {
  const subject =
    e.kind === "dev"
      ? e.total === 1 && e.focus !== null
        ? `Tâche #${e.focus}`
        : `${plural(e.total, "tâche")} lancée${e.total > 1 ? "s" : ""}`
      : e.version
        ? `Passage sur la version ${e.version}`
        : e.total === 1 && e.focus !== null
          ? `Passage sur #${e.focus}`
          : e.total > 0
            ? `Passage sur ${plural(e.total, "issue")}`
            : "Exécution";
  const parts = [subject];
  if (e.total > 0 && (e.kind !== "dev" || e.total > 1)) {
    parts.push(`${e.processed}/${e.total} traitée${e.processed > 1 ? "s" : ""}`);
  }
  if (e.kind === "dev" && e.step !== null) parts.push(`étape ${e.step}/${devSteps.length}`);
  if (e.applied > 0) {
    parts.push(e.kind === "dev" ? `${e.applied} poussée${e.applied > 1 ? "s" : ""}` : `${e.applied} appliquée${e.applied > 1 ? "s" : ""}`);
  }
  if (e.questions > 0) parts.push(`${plural(e.questions, "question")} posée${e.questions > 1 ? "s" : ""}`);
  else if (e.pending > 0) parts.push(`${e.pending} à valider`);
  if (e.kind === "dev" && e.pr !== null) parts.push(`PR #${e.pr}`);
  if (e.failed > 0) parts.push(`${e.failed} en échec`);
  if (e.state === "failed" && e.error) parts.push(e.error);
  return parts.join(" · ");
}

/** Heure d'une ligne du journal : « 11:24 » aujourd'hui, « hier », sinon « 12 sept. ». */
export function journalTime(iso: string, now: Date = new Date()): string {
  const date = new Date(iso);
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return "hier";
  return formatDay(iso);
}

/* ---------- Consommation ---------- */

/** Durée lisible : « 42 s », « 6 min 40 s », « 1 h 05 min ». */
export function formatSpan(totalSeconds: number): string {
  const s = Math.max(0, Math.round(totalSeconds));
  if (s < 60) return `${s} s`;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h} h ${String(m).padStart(2, "0")} min`;
  const rest = s % 60;
  return rest > 0 ? `${m} min ${rest} s` : `${m} min`;
}

/** Durée d'une exécution, jusqu'à maintenant si elle tourne encore. */
export function runSeconds(e: Pick<AgentRunEntry, "startedAt" | "endedAt">, now: Date): number {
  const start = new Date(e.startedAt).getTime();
  const end = e.endedAt ? new Date(e.endedAt).getTime() : now.getTime();
  return Number.isFinite(start) && Number.isFinite(end) ? Math.max(0, (end - start) / 1000) : 0;
}

export interface AgentUsage {
  kind: AgentKind;
  seconds: number;
  /** Part du temps d'exécution de la période, 0 → 100. */
  percent: number;
}

export interface UsageSummary {
  /** Agents triés du plus au moins actif. */
  agents: AgentUsage[];
  runs: number;
  averageSeconds: number;
}

/** Temps passé par agent sur les `days` derniers jours (exécutions commencées dans la période). */
export function usageOver(entries: AgentRunEntry[], now: Date = new Date(), days = 7): UsageSummary {
  const since = now.getTime() - days * 86_400_000;
  const recent = entries.filter((e) => new Date(e.startedAt).getTime() >= since);
  const seconds = new Map<AgentKind, number>(agentOrder.map((k) => [k, 0]));
  for (const e of recent) seconds.set(e.kind, (seconds.get(e.kind) ?? 0) + runSeconds(e, now));
  const total = [...seconds.values()].reduce((a, b) => a + b, 0);
  const list = agentOrder
    .map((kind) => {
      const s = seconds.get(kind) ?? 0;
      return { kind, seconds: s, percent: total > 0 ? (s / total) * 100 : 0 };
    })
    .sort((a, b) => b.seconds - a.seconds);
  return {
    agents: list,
    runs: recent.length,
    averageSeconds: recent.length > 0 ? total / recent.length : 0,
  };
}

/* ---------- Garde-fous ---------- */

/** Agents qui écrivent sans validation (autonomie `auto`). */
export function autoAgents(autonomies: Record<AgentKind, Autonomy>): AgentKind[] {
  return agentOrder.filter((k) => autonomies[k] === "auto");
}

/**
 * Autonomies après le basculement de « Validation avant écriture GitHub » : activée, plus
 * aucun agent en `auto` ; désactivée, ceux qui validaient passent en `auto`. L'agent CEO
 * reste en « propose seulement », comme tout agent réglé ainsi.
 */
export function withReview(
  autonomies: Record<AgentKind, Autonomy>,
  review: boolean,
): Record<AgentKind, Autonomy> {
  const out = { ...autonomies };
  for (const kind of agentOrder) {
    if (review && out[kind] === "auto") out[kind] = "apply-after-review";
    if (!review && kind !== "ceo" && out[kind] === "apply-after-review") out[kind] = "auto";
  }
  return out;
}
