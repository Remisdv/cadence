/** Agent déploiement : pipeline, verdicts des tests, notes de version (écran « Agent déploiement »). */
import type { DeployState, DeployTest } from "@/lib/types";
import type { IconName } from "@/components/icons/Icon";
import type { PipelineStage } from "@/components/domain/Steps";
import { deploySteps } from "./agents";

/** Icône d'une étape en attente ou en cours (une étape finie montre ✓ ou ✗). */
const stageIcons: IconName[] = ["eye", "layers", "check", "book", "tag"];

/** Les cinq cartes du pipeline. */
export function pipelineStages(deploy: DeployState): PipelineStage[] {
  return deploySteps.map((title, i) => {
    const stage = deploy.stages[i];
    const state = stage?.state ?? "pending";
    return {
      title,
      detail: stage?.detail ?? undefined,
      state,
      icon: state === "pending" || state === "running" ? stageIcons[i] : undefined,
    };
  });
}

export interface TestCounts {
  total: number;
  passed: number;
  failed: number;
  unverifiable: number;
}

export function testCounts(tests: DeployTest[]): TestCounts {
  return {
    total: tests.length,
    passed: tests.filter((t) => t.status === "passed").length,
    failed: tests.filter((t) => t.status === "failed").length,
    unverifiable: tests.filter((t) => t.status === "unverifiable").length,
  };
}

/** Test en échec que vous n'avez pas encore tranché (rouvrir ou ignorer). */
export const needsDecision = (t: DeployTest) => t.status === "failed" && t.decision === null;

/** Même règle que Rust (`deploy::in_notes`) : une issue en échec n'est dans les notes que si son échec est ignoré. */
export function inNotes(t: DeployTest): boolean {
  if (t.status === "passed" || t.status === "unverifiable") return true;
  return t.status === "failed" && t.decision === "ignored";
}

/** Sous-titre de l'en-tête : « Release publiée · documentation à jour · 13 tests sur 14 ». */
export function deploySubtitle(deploy: DeployState): string {
  const counts = testCounts(deploy.tests);
  const parts: string[] = [];
  if (deploy.published) parts.push("Release publiée");
  else if (deploy.step >= 5) parts.push("Release à publier");
  else if (deploy.step > 0) parts.push(`Étape ${deploy.step}/5 · ${deploySteps[deploy.step - 1]}`);
  else parts.push("En préparation");
  if (deploy.notes !== null) parts.push("documentation à jour");
  const done = counts.passed + counts.failed + counts.unverifiable;
  if (done > 0)
    parts.push(`${counts.passed} test${counts.passed > 1 ? "s" : ""} sur ${counts.total}`);
  return parts.join(" · ");
}

/** Durée d'un test : « 0,8 s », « 2 min 05 s ». */
export function formatTestDuration(ms: number | null): string {
  if (ms === null) return "";
  if (ms < 60_000) return `${(ms / 1000).toFixed(1).replace(".", ",")} s`;
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")} s`;
}

/** Taille d'artefact : « 512 Ko », « 18,4 Mo ». */
export function formatSize(kb: number | null): string {
  if (kb === null) return "";
  if (kb < 1024) return `${kb} Ko`;
  return `${(kb / 1024).toFixed(1).replace(".", ",")} Mo`;
}

export type NoteLine =
  | { kind: "title"; text: string }
  | { kind: "section"; text: string }
  | { kind: "item"; text: string }
  | { kind: "text"; text: string };

/** Lignes des notes de version telles que rédigées par l'agent (titre, sections, puces). */
export function noteLines(notes: string): NoteLine[] {
  const out: NoteLine[] = [];
  for (const raw of notes.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("### ")) out.push({ kind: "section", text: line.slice(4) });
    else if (line.startsWith("## ") || line.startsWith("# "))
      out.push({ kind: "title", text: line });
    else if (/^[-*] /.test(line)) out.push({ kind: "item", text: line.slice(2) });
    else out.push({ kind: "text", text: line });
  }
  return out;
}
