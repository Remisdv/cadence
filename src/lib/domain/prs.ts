/** Pull requests : filtres de la vue, libellés de revue, lecture des patchs. */
import type { DiffLine } from "@/components/domain/Diff";
import type { PrRow, PrState } from "@/lib/types";

export type PrStateFilter = "open" | "merged" | "closed" | "all";

export interface PrViewFilter {
  /** Titres de version ; `""` = sans version. Vide = toutes. */
  versions: string[];
  state: PrStateFilter;
  author: "all" | "me";
}

export const defaultPrFilter: PrViewFilter = {
  versions: [],
  state: "open",
  author: "all",
};

const statesFor: Record<PrStateFilter, PrState[]> = {
  open: ["draft", "open"],
  merged: ["merged"],
  closed: ["closed"],
  all: ["draft", "open", "merged", "closed"],
};

export const isOpenPr = (pr: PrRow) => pr.state === "open" || pr.state === "draft";

export function matchesPr(pr: PrRow, f: PrViewFilter): boolean {
  return (
    statesFor[f.state].includes(pr.state) &&
    (f.versions.length === 0 || f.versions.includes(pr.version ?? "")) &&
    (f.author === "all" || pr.author.isSelf)
  );
}

/** Durée d'attente : « 6 h », « 2 j » ; rouge au-delà d'un jour. */
export function waitingFor(iso: string, now: Date = new Date()) {
  const hours = Math.max(0, (now.getTime() - new Date(iso).getTime()) / 3_600_000);
  const label = hours < 1 ? "< 1 h" : hours < 24 ? `${Math.floor(hours)} h` : `${Math.floor(hours / 24)} j`;
  return { label, tone: hours >= 24 ? ("red" as const) : ("amber" as const) };
}

export type ReviewTone = "tx2" | "amber" | "red";

const plural = (n: number, word: string) => `${n} ${word}${n > 1 ? "s" : ""}`;

/** Colonne « Revues » : changements demandés, approbations, attente, ou rien. */
export function reviewSummary(pr: PrRow, now: Date = new Date()): { text: string; tone: ReviewTone } | null {
  if (pr.changesRequested > 0) {
    return {
      text: `${pr.changesRequested} changement${pr.changesRequested > 1 ? "s" : ""} demandé${pr.changesRequested > 1 ? "s" : ""}`,
      tone: "amber",
    };
  }
  if (pr.approvals > 0) return { text: plural(pr.approvals, "approbation"), tone: "tx2" };
  if (pr.reviewRequestedAt && isOpenPr(pr)) {
    const wait = waitingFor(pr.reviewRequestedAt, now);
    return { text: `en attente · ${wait.label}`, tone: wait.tone };
  }
  return null;
}

/**
 * Patch unifié GitHub → lignes du `DiffViewer`. Les en-têtes de bloc (`@@ … @@`) après le
 * premier deviennent une ligne « ⋯ » ; « \ No newline at end of file » est ignoré.
 */
export function parsePatch(patch: string): DiffLine[] {
  const lines: DiffLine[] = [];
  let hunks = 0;
  for (const raw of patch.split("\n")) {
    if (raw.startsWith("@@")) {
      if (hunks++ > 0) lines.push({ type: "ctx", text: "⋯" });
    } else if (raw.startsWith("\\")) {
      continue;
    } else if (raw.startsWith("+")) {
      lines.push({ type: "add", text: raw.slice(1) });
    } else if (raw.startsWith("-")) {
      lines.push({ type: "del", text: raw.slice(1) });
    } else {
      lines.push({ type: "ctx", text: raw.slice(1) });
    }
  }
  return lines;
}
