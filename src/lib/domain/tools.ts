/**
 * Registre d'outils (révision 1.2 du plan) : libellés et tons des états, résumé de l'en-tête,
 * journal d'une installation ou d'une connexion reconstruit depuis `tool://install`.
 */
import type { BadgeTone } from "@/components/ui/Badge";
import type { IconName } from "@/components/icons/Icon";
import type {
  AiId,
  ToolId,
  ToolInfo,
  ToolRunEvent,
  ToolRunKind,
  ToolRunResult,
  ToolStatus,
} from "@/lib/types";

export const toolStatusLabels: Record<ToolStatus, { label: string; tone: BadgeTone }> = {
  ready: { label: "Installé", tone: "green" },
  missing: { label: "Manquant", tone: "amber" },
  outdated: { label: "À mettre à jour", tone: "amber" },
  "not-connected": { label: "Non connecté", tone: "amber" },
  error: { label: "En erreur", tone: "red" },
};

/** Libellé d'état ; un outil prêt qui a un compte s'affiche « Connecté ». */
export function toolStatus(tool: ToolInfo): { label: string; tone: BadgeTone } {
  if (tool.status === "ready" && tool.login) return { label: "Connecté", tone: "green" };
  if (tool.status === "missing" && aiOfTool(tool.id))
    return { label: "Introuvable", tone: "neutral" };
  return toolStatusLabels[tool.status];
}

export const toolIcons: Record<ToolId, IconName> = {
  git: "branch",
  gh: "github",
  claude: "sparkle",
  codex: "terminal",
  opencode: "code",
  node: "package",
};

const AI_OF_TOOL: Partial<Record<ToolId, AiId>> = {
  claude: "claude-code",
  codex: "codex",
  opencode: "opencode",
};

export const toolOfAi: Record<AiId, ToolId> = {
  "claude-code": "claude",
  codex: "codex",
  opencode: "opencode",
};

export const aiOfTool = (tool: ToolId): AiId | undefined => AI_OF_TOOL[tool];

/** Outil demandé par un service, un plugin ou une IA choisie, et non écarté. */
export const isRequested = (tool: ToolInfo) => tool.requestedBy.length > 0;

/** Bloque « Continuer » : demandé à titre requis, pas prêt (une IA non connectée bloque aussi). */
export const blocksContinue = (tool: ToolInfo) =>
  tool.required && !tool.ignored && tool.status !== "ready";

export interface ToolsSummary {
  ready: number;
  toConnect: number;
  missingRequired: number;
  missingOptional: number;
}

export function toolsSummary(tools: ToolInfo[]): ToolsSummary {
  const summary: ToolsSummary = { ready: 0, toConnect: 0, missingRequired: 0, missingOptional: 0 };
  for (const tool of tools) {
    if (tool.status === "ready") summary.ready += 1;
    else if (tool.status === "not-connected") summary.toConnect += 1;
    else if (tool.required && !tool.ignored) summary.missingRequired += 1;
    else summary.missingOptional += 1;
  }
  return summary;
}

/** Recette proposée : la première dont le gestionnaire est présent, sinon la première. */
export function preferredRecipe(tool: ToolInfo) {
  return tool.recipes.find((r) => r.available) ?? tool.recipes[0] ?? null;
}

/**
 * Outils à installer d'un coup (« Tout installer ») : manquants, non ignorés, installables ;
 * Node.js en premier, puisque les recettes npm en dépendent.
 */
export const installable = (tools: ToolInfo[]) =>
  tools
    .filter(
      (t) =>
        (t.status === "missing" || t.status === "outdated") && !t.ignored && preferredRecipe(t),
    )
    .sort((a, b) => Number(b.id === "node") - Number(a.id === "node"));

/** « Demandé par » : requis s'il y en a un, sinon optionnel. */
export function requestLabel(tool: ToolInfo): { label: string; tone: BadgeTone } {
  if (tool.requestedBy.some((r) => r.kind === "ai" && !r.optional))
    return { label: "par défaut", tone: "clay" };
  if (tool.required) return { label: "requis", tone: "clay" };
  return { label: "optionnel", tone: "neutral" };
}

/* ---------- Journal ---------- */

export interface ToolRunLog {
  kind: ToolRunKind;
  /** Commande lancée, affichée en tête du journal. */
  command: string;
  /** Journal de la commande précédente (l'installation, au-dessus de la connexion). */
  previous: { command: string; lines: string[] } | null;
  lines: string[];
  running: boolean;
  result: ToolRunResult | null;
}

export function startLog(
  kind: ToolRunKind,
  command: string,
  before: ToolRunLog | null = null,
): ToolRunLog {
  const previous =
    before && before.kind === "install" && kind === "login"
      ? { command: before.command, lines: before.lines }
      : null;
  return { kind, command, previous, lines: [], running: true, result: null };
}

export function applyRunEvent(log: ToolRunLog | undefined, event: ToolRunEvent): ToolRunLog {
  const base = log && log.kind === event.kind ? log : startLog(event.kind, "");
  if (event.done) return { ...base, running: false, result: event.done };
  if (event.line === null) return base;
  const lines =
    event.replace && base.lines.length > 0
      ? [...base.lines.slice(0, -1), event.line]
      : [...base.lines, event.line];
  return { ...base, lines: lines.slice(-500) };
}

/** Code d'appairage affiché par `gh auth login --web` (« one-time code: A1B2-C3D4 »). */
export function pairingCode(lines: string[]): string | null {
  for (const line of lines) {
    const match = /one-time code:\s*([A-Z0-9]{4}-[A-Z0-9]{4})/i.exec(line);
    if (match) return match[1]!.toUpperCase();
  }
  return null;
}

/** Adresse à ouvrir pendant une connexion (« https://github.com/login/device »). */
export function loginUrl(lines: string[]): string | null {
  for (const line of lines) {
    const match = /(https:\/\/[^\s]+)/.exec(line);
    if (match) return match[1]!.replace(/[.,;]+$/, "");
  }
  return null;
}

/** Paquet d'une recette (« winget install --id GitHub.cli … » → « GitHub.cli »). */
export function recipePackage(command: string): string {
  const parts = command.split(" ");
  const id = parts.indexOf("--id");
  if (id >= 0 && parts[id + 1]) return parts[id + 1]!;
  const install = parts.findIndex((p) => p === "install" || p === "-S");
  const after = parts.slice(install + 1).find((p) => !p.startsWith("-"));
  return after ?? parts[0] ?? command;
}

/** Durée lisible (« 38 s », « 1 min 12 s »). */
export function formatRunDuration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  return `${Math.floor(s / 60)} min ${s % 60} s`;
}

/** Dernier pourcentage du journal (barre de winget, npm…) ; `null` s'il n'y en a pas. */
export function runProgress(lines: string[]): number | null {
  const num = (text: string) => Number(text.replace(",", "."));
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i]!;
    const percent = /(\d{1,3}(?:[.,]\d+)?)\s?%/.exec(line);
    if (percent) return Math.min(100, Math.round(num(percent[1]!)));
    // « 9,4 Mo / 13,8 Mo », « 12.1 MB / 40.0 MB »
    const sizes = /([\d.,]+)\s?(?:[kKMG]o|[kKMG]B)\s*\/\s*([\d.,]+)\s?(?:[kKMG]o|[kKMG]B)/.exec(
      line,
    );
    if (sizes && num(sizes[2]!) > 0)
      return Math.min(100, Math.round((num(sizes[1]!) / num(sizes[2]!)) * 100));
  }
  return null;
}

/** « Demandé par le service Dépôts GitHub (optionnel) » pour l'en-tête d'installation. */
export function requestSentence(tool: ToolInfo): string | null {
  const first = tool.requestedBy[0];
  if (!first) return null;
  const who =
    first.kind === "service"
      ? `le service ${first.name}`
      : first.kind === "ai"
        ? "l'étape IA de code"
        : first.kind === "tool"
          ? `${first.name} (installation par npm)`
          : `le plugin ${first.name}`;
  const others = tool.requestedBy.length - 1;
  return `Demandé par ${who}${first.optional ? " (optionnel)" : ""}${others > 0 ? ` et ${others} autre${others > 1 ? "s" : ""}` : ""}`;
}
