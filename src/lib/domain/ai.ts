/** IA de code pilotées par Cadence 2 : libellés affichés (plan § 4). */
import type { AiId, ToolInfo } from "@/lib/types";

export const aiLabels: Record<AiId, { name: string }> = {
  "claude-code": { name: "Claude Code" },
  codex: { name: "Codex CLI" },
  opencode: { name: "OpenCode" },
};

export const aiOrder: AiId[] = ["claude-code", "codex", "opencode"];

/**
 * État de l'IA par défaut montré dans le rail, lu dans le même rapport d'outils que l'onboarding
 * et les réglages (`tools_list`) : « connectée » dès que la CLI est détectée et connectée, même
 * si la session de test n'a pas été lancée (recette du 21/10, P02).
 */
export function aiRailState(
  ai: AiId | null,
  tool: Pick<ToolInfo, "status"> | undefined,
): { label: string; tone: "green" | "amber" | "neutral" } {
  if (!ai) return { label: "à choisir", tone: "neutral" };
  if (!tool) return { label: "détection…", tone: "neutral" };
  switch (tool.status) {
    case "ready":
      return { label: "connectée", tone: "green" };
    case "not-connected":
      return { label: "à connecter", tone: "amber" };
    case "missing":
      return { label: "non installée", tone: "amber" };
    case "outdated":
      return { label: "à mettre à jour", tone: "amber" };
    case "error":
      return { label: "détection en échec", tone: "amber" };
  }
}
