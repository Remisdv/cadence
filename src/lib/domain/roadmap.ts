/** Roadmap : versions (milestones) en cartes, avec leurs issues ou leurs thèmes. */
import type { IssueRow, Version } from "@/lib/types";
import { compareVersions, versionStage, type VersionStage } from "./versions";

export interface RoadmapItem {
  text: string;
  done: boolean;
}

export interface RoadmapCard {
  version: Version;
  stage: VersionStage;
  /** Trois issues (ouvertes d'abord), ou les thèmes du palier s'il n'en a pas. */
  items: RoadmapItem[];
  /** Vrai si `items` vient des thèmes de la description, pas des issues. */
  themesOnly: boolean;
  total: number;
  closed: number;
  /** 0 → 100 */
  progress: number;
}

export interface Roadmap {
  /** Versions livrées (les plus récentes), puis la version en cours. */
  shipped: RoadmapCard[];
  /** Versions livrées masquées faute de place. */
  olderShipped: number;
  current: RoadmapCard | null;
  /** Sprints planifiés, puis paliers. */
  planned: RoadmapCard[];
}

const ITEMS_PER_CARD = 3;

/** Thèmes d'un palier : les éléments de liste de la description du milestone. */
export function themesOf(description: string | null): string[] {
  return (description ?? "")
    .split("\n")
    .map((l) => /^\s*[-*+]\s+(.+)$/.exec(l)?.[1]?.trim())
    .filter((t): t is string => Boolean(t));
}

export function buildRoadmap(
  versions: Version[],
  issues: IssueRow[],
  current: string | null,
  maxShipped = 3,
): Roadmap {
  const cards = [...versions]
    .sort((a, b) => compareVersions(a.title, b.title))
    .map((version) => card(version, issues, current));
  const shipped = cards.filter((c) => c.stage === "shipped");
  return {
    shipped: shipped.slice(-maxShipped),
    olderShipped: Math.max(0, shipped.length - maxShipped),
    current: cards.find((c) => c.stage === "current") ?? null,
    planned: [
      ...cards.filter((c) => c.stage === "planned"),
      ...cards.filter((c) => c.stage === "milestone" || c.stage === "vision"),
    ],
  };
}

function card(version: Version, issues: IssueRow[], current: string | null): RoadmapCard {
  const scope = issues.filter((i) => i.version === version.title);
  // Les compteurs du milestone font foi (issues hors cache comprises).
  const total = Math.max(version.openIssues + version.closedIssues, scope.length);
  const closed = Math.max(version.closedIssues, scope.filter((i) => i.status === "done").length);
  const fromIssues = [...scope]
    .sort((a, b) => Number(a.status === "done") - Number(b.status === "done"))
    .slice(0, ITEMS_PER_CARD)
    .map((i) => ({ text: i.title, done: i.status === "done" }));
  const themes = themesOf(version.description).slice(0, ITEMS_PER_CARD);
  const themesOnly = fromIssues.length === 0 && themes.length > 0;
  return {
    version,
    stage: versionStage(version, current),
    items: themesOnly ? themes.map((text) => ({ text, done: false })) : fromIssues,
    themesOnly,
    total,
    closed,
    progress: total === 0 ? 0 : (closed / total) * 100,
  };
}

/** Jours restants avant l'échéance (négatif si dépassée), `null` sans échéance. */
export function daysUntil(due: string | null, now: Date = new Date()): number | null {
  if (!due) return null;
  const day = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
  return Math.round((day(new Date(due)) - day(now)) / 86_400_000);
}

const monthYear = new Intl.DateTimeFormat("fr-FR", { month: "short", year: "numeric" });

/** « sept. 2026 → oct. 2026 » : des premiers aux derniers sprints datés. */
export function periodOf(versions: Version[]): string | null {
  const dates = versions
    .flatMap((v) => [v.startOn, v.dueOn])
    .filter((d): d is string => Boolean(d))
    .sort();
  if (dates.length === 0) return null;
  const [first, last] = [monthYear.format(new Date(dates[0]!)), monthYear.format(new Date(dates.at(-1)!))];
  const label = first === last ? first : `${first} → ${last}`;
  return label.charAt(0).toUpperCase() + label.slice(1);
}
