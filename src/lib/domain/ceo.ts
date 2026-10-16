/** Agent CEO : palier visé, couverture des thèmes, résumé des propositions. */
import type { AgentRunDetail, CeoProposal, Version } from "@/lib/types";
import { compareVersions } from "./versions";
import { themesOf } from "./roadmap";

/**
 * Palier visé par défaut (même règle que Rust, `agents::ceo::next_target`) : le premier
 * palier ouvert après la version en cours, sinon la première version ouverte après elle.
 */
export function ceoTarget(versions: Version[], current: string | null): string | null {
  const after = [...versions]
    .sort((a, b) => compareVersions(a.title, b.title))
    .filter(
      (v) => v.state === "open" && (current === null || compareVersions(v.title, current) > 0),
    );
  return (after.find((v) => v.kind === "palier") ?? after[0])?.title ?? null;
}

/** Propositions encore à relire (création possible). */
export function pendingProposals(run: AgentRunDetail | null | undefined): CeoProposal[] {
  return run?.kind === "ceo"
    ? run.proposals.filter((p) => p.state === "ready" || p.state === "failed")
    : [];
}

/**
 * Cochée d'office : à créer, dans le palier visé, sans recouvrement signalé. Les autres
 * (autre version, recouvrement à trancher) attendent un choix explicite.
 */
export function isDefaultPick(p: CeoProposal, target: string | null): boolean {
  return (
    p.state === "ready" && p.milestone === target && !p.overlap && p.overlapIssues.length === 0
  );
}

export interface ThemeCoverage {
  theme: string;
  /** Au moins une proposition (encore active) en découle. */
  covered: boolean;
}

/** Thèmes du palier, marqués selon qu'une proposition en découle. */
export function themeCoverage(
  version: Version | undefined,
  proposals: CeoProposal[],
): ThemeCoverage[] {
  const norm = (s: string) => s.trim().toLowerCase();
  const used = new Set(
    proposals.filter((p) => p.state !== "skipped" && p.theme).map((p) => norm(p.theme!)),
  );
  const themes = themesOf(version?.description ?? null);
  // Thèmes cités par les propositions mais absents de la description (ex. « Notifications »).
  const extra = [
    ...new Set(proposals.map((p) => p.theme).filter((t): t is string => Boolean(t))),
  ].filter((t) => !themes.some((x) => norm(x) === norm(t)));
  return [
    ...themes.map((theme) => ({ theme, covered: used.has(norm(theme)) })),
    ...extra.map((theme) => ({ theme, covered: false })),
  ];
}

/** Libellé des thèmes d'où viennent les propositions : « Rôles et permissions », « Multi-dépôts ». */
export function themesLabel(proposals: CeoProposal[]): string {
  const themes = [...new Set(proposals.map((p) => p.theme).filter((t): t is string => Boolean(t)))];
  return themes.map((t) => `« ${t} »`).join(", ");
}

/** Pied de l'écran : « 3 issues seront créées … avec le milestone 0.2.0 ». */
export function milestonesLabel(proposals: CeoProposal[]): string {
  const milestones = [...new Set(proposals.map((p) => p.milestone))].sort(compareVersions);
  if (milestones.length === 0) return "";
  if (milestones.length === 1) return `le milestone ${milestones[0]}`;
  return `les milestones ${milestones.slice(0, -1).join(", ")} et ${milestones.at(-1)}`;
}

export const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;
