import type { AgentKind, Autonomy } from "@/lib/types";
import type { BadgeTone } from "@/components/ui/Badge";

export interface AgentMeta {
  kind: AgentKind;
  /** Lettre du carré d'agent (C, D, V, L). */
  letter: string;
  name: string;
  short: string;
  tone: Extract<BadgeTone, "blue" | "purple" | "green" | "amber">;
  role: string;
}

export const agents: Record<AgentKind, AgentMeta> = {
  cto: {
    kind: "cto",
    letter: "C",
    name: "Agent CTO",
    short: "CTO",
    tone: "blue",
    role: "Complète les descriptions, pose les questions manquantes",
  },
  ceo: {
    kind: "ceo",
    letter: "D",
    name: "Agent CEO",
    short: "CEO",
    tone: "purple",
    role: "Dérive des issues candidates depuis la roadmap",
  },
  dev: {
    kind: "dev",
    letter: "V",
    name: "Agent développeur",
    short: "Développeur",
    tone: "green",
    role: "Branche, PR, implémentation",
  },
  deploy: {
    kind: "deploy",
    letter: "L",
    name: "Agent déploiement",
    short: "Déploiement",
    tone: "amber",
    role: "Release, documentation, tests de vérification",
  },
};

export const agentOrder: AgentKind[] = ["cto", "ceo", "dev", "deploy"];

export const autonomyLabels: Record<Autonomy, string> = {
  propose: "Propose seulement",
  "apply-after-review": "Applique après validation",
  auto: "Automatique",
};

/** Étapes de l'agent développeur (écran « Agent développeur »). */
export const devSteps = [
  "Analyse de l'issue et du dépôt",
  "Branche et PR en brouillon",
  "Implémentation",
  "Tests",
  "Push et mise à jour de la PR",
  "Sortie du brouillon et demande de revue",
] as const;

/** Étapes de l'agent déploiement. */
export const deploySteps = [
  "Pré-vol",
  "Build",
  "Tests de vérification",
  "Documentation",
  "Tag et release",
] as const;
