import type { Version } from "@/lib/types";
import type { BadgeTone } from "@/components/ui/Badge";

/** Statut d'affichage d'une version dans la roadmap. */
export type VersionStage = "shipped" | "current" | "planned" | "milestone" | "vision";

export const versionStageMeta: Record<VersionStage, { label: string; tone: BadgeTone }> = {
  shipped: { label: "livrée", tone: "green" },
  current: { label: "en cours", tone: "clay" },
  planned: { label: "planifiée", tone: "muted" },
  milestone: { label: "planifiée", tone: "blue" },
  vision: { label: "vision", tone: "purple" },
};

type Semver = [number, number, number];

export function parseSemver(title: string): Semver | null {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(title.trim());
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

/** Tri semver (jamais lexical : 0.11.0 vient après 0.2.0). Non-semver en dernier. */
export function compareVersions(a: string, b: string): number {
  const x = parseSemver(a);
  const y = parseSemver(b);
  if (!x || !y) return x ? -1 : y ? 1 : a.localeCompare(b);
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
}

/** Sprint suivant, nouveau palier, version majeure. */
export function nextVersions(current: string): [string, string, string] | null {
  const v = parseSemver(current);
  if (!v) return null;
  const [major, minor, patch] = v;
  return [`${major}.${minor}.${patch + 1}`, `${major}.${minor + 1}.0`, `${major + 1}.0.0`];
}

export function versionStage(version: Version, current: string | null): VersionStage {
  if (version.state === "closed") return "shipped";
  if (version.title === current) return "current";
  if (version.kind === "sprint") return "planned";
  const v = parseSemver(version.title);
  return v && v[1] === 0 && v[2] === 0 && v[0] >= 1 ? "vision" : "milestone";
}

/** Durée d'un sprint par défaut (`versioning.sprint_days` de `.cadence/config.toml`). */
export const SPRINT_DAYS = 14;

/**
 * Dates du sprint qui suit une clôture : il commence à l'échéance de la version close (ou
 * aujourd'hui si elle est dépassée) et dure `days` jours (même règle que `plan_close` côté
 * Rust). Dates `YYYY-MM-DD`.
 */
export function nextSprintRange(currentDue: string | null, now: Date = new Date(), days = SPRINT_DAYS) {
  const today = now.toISOString().slice(0, 10);
  const due = currentDue?.slice(0, 10) ?? null;
  const start = due && due > today ? due : today;
  const end = new Date(`${start}T12:00:00Z`);
  end.setUTCDate(end.getUTCDate() + days);
  return { start, end: end.toISOString().slice(0, 10) };
}
