/**
 * `version_detail` mocké : la version en cours reprend `currentVersion` (burndown des
 * maquettes), les autres sont dérivées des compteurs de leur milestone.
 */
import type { BurndownPoint, Version, VersionDetail, VersionReport } from "@/lib/types";
import { compareVersions } from "@/lib/domain/versions";
import * as data from "./fixtures";

const DAY = 86_400_000;

function days(start: string, end: string): number {
  return Math.round((Date.parse(end.slice(0, 10)) - Date.parse(start.slice(0, 10))) / DAY);
}

/** Burndown régulier d'un sprint livré : la portée descend jusqu'à zéro à l'échéance. */
function shippedBurndown(version: Version, length: number): BurndownPoint[] {
  const scope = version.openIssues + version.closedIssues;
  const start = Date.parse(version.startOn!.slice(0, 10));
  return Array.from({ length: length + 1 }, (_, i) => ({
    date: new Date(start + i * DAY).toISOString().slice(0, 10),
    remaining: Math.round(scope * (1 - i / length)),
  }));
}

function detailOf(version: Version): VersionDetail {
  if (version.title === data.currentVersion.version.title) return data.currentVersion;
  const total = version.openIssues + version.closedIssues;
  const progress = total === 0 ? 0 : Math.round((version.closedIssues / total) * 100);
  const length = version.startOn && version.dueOn ? days(version.startOn, version.dueOn) : null;
  const shipped = version.state === "closed" && length !== null;
  return {
    version,
    daysLeft: version.dueOn ? days(new Date().toISOString(), version.dueOn) : null,
    progress,
    velocity: shipped ? Math.round((version.closedIssues / length) * 10) / 10 : 0,
    delayDays: 0,
    burndown: shipped ? shippedBurndown(version, length) : [],
  };
}

export function versionReport(title: string | null): VersionReport | null {
  const sorted = [...data.versions].sort((a, b) => compareVersions(a.title, b.title));
  const index = sorted.findIndex((v) => v.title === title);
  const version = sorted[index];
  if (!version) return null;
  const issues = data.issues
    .filter((i) => i.version === version.title)
    .sort(
      (a, b) => Number(a.status === "done") - Number(b.status === "done") || b.number - a.number,
    );
  const numbers = new Set(issues.map((i) => i.number));
  // Avancement cohérent avec les issues mockées (comme le calcule Rust).
  const closed = issues.filter((i) => i.status === "done").length;
  const detail = issues.length
    ? { ...detailOf(version), progress: Math.round((closed / issues.length) * 100) }
    : detailOf(version);
  const sprintDays =
    version.kind === "sprint" && version.startOn && version.dueOn
      ? days(version.startOn, version.dueOn)
      : null;
  return {
    detail,
    current: version.title === data.project.currentVersion,
    issues,
    pulls: data.pullRequests.filter(
      (p) => p.version === version.title || (p.linkedIssue !== null && numbers.has(p.linkedIssue)),
    ),
    previous: sorted[index - 1]?.title ?? null,
    next: sorted[index + 1]?.title ?? null,
    sprintDays,
    tag: version.state === "closed" ? `v${version.title}` : null,
  };
}
