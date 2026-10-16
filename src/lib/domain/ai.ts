/** IA de code pilotées par Cadence 2 : libellés affichés (plan § 4). */
import type { AiId } from "@/lib/types";

export const aiLabels: Record<AiId, { name: string }> = {
  "claude-code": { name: "Claude Code" },
  codex: { name: "Codex CLI" },
  opencode: { name: "OpenCode" },
};

export const aiOrder: AiId[] = ["claude-code", "codex", "opencode"];
