/**
 * Regroupement des issues par version (vue Issues) : ordre des groupes, libellés, avancement.
 */
import type { IssueRow, IssueStatus, Version } from "@/lib/types";
import { compareVersions, versionStage, type VersionStage } from "./versions";

export interface IssueGroup {
  /** Titre de la version, ou `null` pour les issues sans milestone. */
  version: string | null;
  stage: VersionStage | null;
  label: string;
  /** Issues affichées (après filtres). */
  rows: IssueRow[];
  /** Comptes sur toutes les issues de la version, filtres ignorés. */
  open: number;
  total: number;
  progress: number;
}

export type StateFilter = "open" | "closed" | "all";

export interface IssueViewFilter {
  query: string;
  /** Versions retenues ; vide = toutes. `""` désigne « sans version ». */
  versions: string[];
  state: StateFilter;
  /** `me` : seulement les issues qui me sont assignées. */
  assignee: "all" | "me";
}

const openStatuses: IssueStatus[] = ["todo", "doing", "review"];

export function matchesView(row: IssueRow, f: IssueViewFilter): boolean {
  const q = f.query.trim().toLowerCase();
  const isOpen = openStatuses.includes(row.status);
  return (
    (f.state === "all" || (f.state === "open" ? isOpen : !isOpen)) &&
    (f.versions.length === 0 || f.versions.includes(row.version ?? "")) &&
    (f.assignee === "all" || row.assignee?.isSelf === true) &&
    (q === "" ||
      row.title.toLowerCase().includes(q) ||
      `#${row.number}` === q ||
      String(row.number) === q ||
      row.labels.some((l) => l.toLowerCase().includes(q)) ||
      (row.assignee?.login.toLowerCase().includes(q) ?? false))
  );
}

/**
 * Groupes dans l'ordre de lecture : versions ouvertes (semver croissant, donc le sprint en
 * cours d'abord), puis versions livrées (les plus récentes d'abord), puis « Sans version ».
 * Un groupe sans issue après filtres n'est pas affiché.
 */
export function groupByVersion(
  all: IssueRow[],
  visible: IssueRow[],
  versions: Version[],
  current: string | null,
): IssueGroup[] {
  const byTitle = new Map(versions.map((v) => [v.title, v]));
  const titles = [...new Set(all.map((i) => i.version ?? ""))];

  const rank = (title: string) => {
    if (title === "") return 3;
    return byTitle.get(title)?.state === "closed" ? 2 : 1;
  };
  titles.sort((a, b) => {
    const ra = rank(a);
    const rb = rank(b);
    if (ra !== rb) return ra - rb;
    return ra === 2 ? compareVersions(b, a) : compareVersions(a, b);
  });

  let nextSprintSeen = false;
  return titles
    .map((title): IssueGroup => {
      const inVersion = all.filter((i) => (i.version ?? "") === title);
      const open = inVersion.filter((i) => openStatuses.includes(i.status)).length;
      const total = inVersion.length;
      const version = byTitle.get(title);
      const stage = version ? versionStage(version, current) : null;
      let label = "Sans version";
      if (title !== "") {
        if (stage === "current") label = "Sprint en cours";
        else if (stage === "shipped") label = "Livrée";
        else if (stage === "planned") {
          label = nextSprintSeen ? "Sprint planifié" : "Sprint suivant";
          nextSprintSeen = true;
        } else if (stage === "milestone" || stage === "vision") label = "Palier";
        else label = "Version";
      }
      return {
        version: title === "" ? null : title,
        stage,
        label,
        rows: visible.filter((i) => (i.version ?? "") === title),
        open,
        total,
        progress: total === 0 ? 0 : ((total - open) / total) * 100,
      };
    })
    .filter((g) => g.rows.length > 0);
}
