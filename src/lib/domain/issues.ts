import type { CiState, IssueStatus, IssueType, PrState } from "@/lib/types";
import type { BadgeTone } from "@/components/ui/Badge";
import type { DotTone } from "@/components/ui/StatusDot";

export const issueTypeTone: Record<IssueType, BadgeTone> = {
  feat: "blue",
  fix: "amber",
  perf: "purple",
  chore: "neutral",
  docs: "muted",
};

export const issueTypes: IssueType[] = ["feat", "fix", "chore", "perf", "docs"];

export interface StatusMeta {
  label: string;
  dot: DotTone;
}

/** Colonnes du tableau de sprint, dans l'ordre. */
export const issueStatuses: Record<IssueStatus, StatusMeta> = {
  todo: { label: "À faire", dot: "neutral" },
  doing: { label: "En cours", dot: "clay" },
  review: { label: "En revue", dot: "blue" },
  done: { label: "Fait", dot: "green" },
};

export const statusOrder: IssueStatus[] = ["todo", "doing", "review", "done"];

export const prStateMeta: Record<PrState, { label: string; tone: BadgeTone; iconClass: string }> = {
  draft: { label: "brouillon", tone: "neutral", iconClass: "text-tx3" },
  open: { label: "ouverte", tone: "green", iconClass: "text-green" },
  merged: { label: "fusionnée", tone: "purple", iconClass: "text-purple" },
  closed: { label: "fermée", tone: "red", iconClass: "text-red" },
};

export const ciMeta: Record<CiState, { label: string; iconClass: string } | null> = {
  none: null,
  pending: { label: "en cours", iconClass: "text-amber" },
  success: { label: "verte", iconClass: "text-green" },
  failure: { label: "en échec", iconClass: "text-red" },
};

/** Regroupe des éléments par clé en conservant l'ordre des clés fourni. */
export function groupBy<T, K extends string>(items: T[], keyOf: (item: T) => K, order: K[]) {
  const groups = new Map<K, T[]>(order.map((k) => [k, []]));
  for (const item of items) groups.get(keyOf(item))?.push(item);
  return groups;
}
