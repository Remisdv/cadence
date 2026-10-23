/**
 * Sprint en cours et détail de version : tableau par état, KPI, burndown, points à traiter,
 * notes de version et conditions de clôture.
 */
import type {
  BurndownPoint,
  IssueRow,
  IssueStatus,
  IssueType,
  PrRow,
  ReviewRow,
} from "@/lib/types";
import type { ProgressTone } from "@/components/ui/ProgressBar";
import { formatDecimal } from "./format";
import { issueStatuses, statusOrder } from "./issues";
import { isOpenPr, waitingFor } from "./prs";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n > 1 ? many : one}`;

/* ---------- Tableau ---------- */

export interface BoardColumn {
  status: IssueStatus;
  issues: IssueRow[];
  /** Nombre total (la colonne « Fait » est tronquée). */
  total: number;
}

/**
 * Colonnes du tableau dans l'ordre des états, issues par numéro croissant ; « Fait » ne
 * garde que les plus récentes.
 */
export function sprintBoard(issues: IssueRow[], doneLimit = 3): BoardColumn[] {
  return statusOrder.map((status) => {
    const all = issues.filter((i) => i.status === status);
    const shown =
      status === "done"
        ? [...all].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0, doneLimit)
        : [...all].sort((a, b) => a.number - b.number);
    return { status, issues: shown, total: all.length };
  });
}

/** Jauges « Progression » : fermées, en revue, en cours, à faire. */
export function statusMeters(issues: IssueRow[]) {
  const total = Math.max(1, issues.length);
  const tones: Record<IssueStatus, ProgressTone> = {
    done: "green",
    review: "blue",
    doing: "clay",
    todo: "neutral",
  };
  return (["done", "review", "doing", "todo"] as const).map((status) => {
    const count = issues.filter((i) => i.status === status).length;
    return {
      label: status === "done" ? "Fermées" : issueStatuses[status].label,
      value: count,
      percent: (count / total) * 100,
      tone: tones[status],
    };
  });
}

/* ---------- KPI et burndown ---------- */

/** Points d'avancement gagnés depuis la veille (« +3 aujourd'hui »). */
export function gainedToday(burndown: BurndownPoint[], scope: number): number {
  const [prev, last] = burndown.slice(-2);
  if (!prev || !last || scope === 0) return 0;
  return Math.round((Math.max(0, prev.remaining - last.remaining) / scope) * 100);
}

/** Retard sur la droite idéale : « 1,5 j de retard » (ambre), « 2 j d'avance » (vert). */
export function delayLabel(delayDays: number): { text: string; tone: "amber" | "green" | "tx3" } {
  if (delayDays >= 0.5) return { text: `${formatDecimal(delayDays)} j de retard`, tone: "amber" };
  if (delayDays <= -0.5) return { text: `${formatDecimal(-delayDays)} j d'avance`, tone: "green" };
  return { text: "dans les temps", tone: "tx3" };
}

/** « 6 jours restants », « dernier jour », « échéance dépassée de 2 j ». */
export function daysLeftLabel(days: number | null): string {
  if (days === null) return "sans échéance";
  if (days < 0) return `échéance dépassée de ${-days} j`;
  if (days === 0) return "dernier jour";
  return `${plural(days, "jour")} restant${days > 1 ? "s" : ""}`;
}

/** Issues à fermer par jour pour finir à l'échéance ; `null` sans échéance à venir. */
export function requiredPace(remaining: number, daysLeft: number | null): number | null {
  if (daysLeft === null || daysLeft <= 0) return null;
  return Math.round((remaining / daysLeft) * 10) / 10;
}

export interface PullStats {
  open: number;
  merged: number;
}

export function pullStats(pulls: PrRow[]): PullStats {
  return {
    open: pulls.filter(isOpenPr).length,
    merged: pulls.filter((p) => p.state === "merged").length,
  };
}

/** Revues demandées à l'utilisateur et l'attente de la plus ancienne (« 2 j »). */
export function reviewsToDo(reviews: ReviewRow[], now: Date = new Date()) {
  const mine = reviews.filter((r) => r.audience === "you");
  const oldest = [...mine].sort((a, b) => a.requestedAt.localeCompare(b.requestedAt))[0];
  return { count: mine.length, oldest: oldest ? waitingFor(oldest.requestedAt, now).label : null };
}

/* ---------- Points à traiter ---------- */

export type SprintTodo = { kind: "review"; pr: number; text: string };

/** Ce qui attend l'utilisateur : revues qui attendent depuis plus d'un jour. */
export function sprintTodos(reviews: ReviewRow[], now: Date = new Date()): SprintTodo[] {
  return reviews
    .filter((r) => r.audience === "you")
    .map((r) => ({ r, hours: (now.getTime() - new Date(r.requestedAt).getTime()) / 3_600_000 }))
    .filter(({ hours }) => hours >= 24)
    .sort((a, b) => b.hours - a.hours)
    .map(({ r, hours }): SprintTodo => {
      const days = Math.floor(hours / 24);
      return {
        kind: "review",
        pr: r.pr.number,
        text: `PR #${r.pr.number} en attente de revue depuis ${plural(days, "jour")}`,
      };
    });
}

/* ---------- Notes de version ---------- */

const noteSections: { title: string; types: (IssueType | null)[] }[] = [
  { title: "Nouveautés", types: ["feat"] },
  { title: "Corrections", types: ["fix"] },
  { title: "Performances", types: ["perf"] },
  { title: "Documentation", types: ["docs"] },
  { title: "Maintenance", types: ["chore", null] },
];

/** Aperçu des notes de version : issues fermées groupées par type. */
export function releaseNotes(issues: IssueRow[]) {
  const closed = issues.filter((i) => i.status === "done");
  return noteSections
    .map(({ title, types }) => ({
      title,
      items: closed
        .filter((i) => types.includes(i.issueType))
        .map((i) => `${i.title} (#${i.number})`),
    }))
    .filter((s) => s.items.length > 0);
}

/* ---------- Clôture ---------- */

export interface CloseCheck {
  ok: boolean;
  text: string;
}

/** Conditions de clôture : issues fermées, PR fusionnées, CI des PR ouvertes. */
export function closeChecks(issues: IssueRow[], pulls: PrRow[]): CloseCheck[] {
  const open = issues.filter((i) => i.status !== "done").length;
  const unmerged = pulls.filter(isOpenPr);
  const failing = unmerged.filter((p) => p.ci === "failure").length;
  const checks: CloseCheck[] = [
    open === 0
      ? { ok: true, text: "Toutes les issues sont fermées" }
      : { ok: false, text: `${plural(open, "issue")} encore ${open > 1 ? "ouvertes" : "ouverte"}` },
    unmerged.length === 0
      ? { ok: true, text: "Aucune PR en attente" }
      : {
          ok: false,
          text: `${plural(unmerged.length, "PR", "PR")} non ${unmerged.length > 1 ? "fusionnées" : "fusionnée"}`,
        },
  ];
  if (unmerged.length > 0) {
    checks.push(
      failing === 0
        ? { ok: true, text: "CI verte sur les PR ouvertes" }
        : { ok: false, text: `CI en échec sur ${plural(failing, "PR", "PR")}` },
    );
  }
  return checks;
}
