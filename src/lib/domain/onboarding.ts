/**
 * Onboarding de Cadence 2 (V2-Onboarding-*.dc.html) : étapes de la colonne de gauche, et
 * écrans de l'étape « Configuration », dans l'ordre du rail. Les services de Cadence
 * s'intercalent à l'étape du premier plugin qui les utilise (connexion GitHub, puis dépôts
 * suivis), avant les secrets et les écrans `[[setup]]` du plugin.
 */
import type { InstalledPluginView, PluginSecretInfo, PluginSetupStep } from "@/lib/types";
import type { OnboardingStep } from "@/stores/navigation";

export const onboardingSteps: { id: Exclude<OnboardingStep, "welcome">; label: string }[] = [
  { id: "ai", label: "IA de code" },
  { id: "plugins", label: "Plugins" },
  { id: "tools", label: "Outils nécessaires" },
  { id: "setup", label: "Configuration" },
  { id: "done", label: "Terminé" },
];

/** Index de l'étape dans la colonne de gauche (−1 pour la bienvenue). */
export const stepIndex = (step: OnboardingStep) => onboardingSteps.findIndex((s) => s.id === step);

export const nextStep = (step: OnboardingStep): OnboardingStep =>
  step === "welcome" ? "ai" : (onboardingSteps[stepIndex(step) + 1]?.id ?? "done");

export const previousStep = (step: OnboardingStep): OnboardingStep =>
  stepIndex(step) <= 0 ? "welcome" : onboardingSteps[stepIndex(step) - 1]!.id;

export type SetupTask =
  | { kind: "github"; plugin: InstalledPluginView }
  | { kind: "repos"; plugin: InstalledPluginView }
  | { kind: "secret"; plugin: InstalledPluginView; secret: PluginSecretInfo }
  | { kind: "view"; plugin: InstalledPluginView; step: PluginSetupStep };

/** Écrans de configuration des plugins activés, dans l'ordre du rail. */
export function setupTasks(plugins: InstalledPluginView[]): SetupTask[] {
  const tasks: SetupTask[] = [];
  let github = false;
  for (const plugin of plugins) {
    if (!plugin.enabled || plugin.status === "invalid") continue;
    if (plugin.services.includes("repos")) {
      if (!github) tasks.push({ kind: "github", plugin });
      github = true;
      tasks.push({ kind: "repos", plugin });
    }
    const secrets = [...plugin.secrets].sort((a, b) => Number(b.required) - Number(a.required));
    for (const secret of secrets) tasks.push({ kind: "secret", plugin, secret });
    for (const step of plugin.setup) tasks.push({ kind: "view", plugin, step });
  }
  return tasks;
}

/** Titre de l'écran dans le fil d'Ariane (« Gestion de projet › Compte GitHub »). */
export function taskTitle(task: SetupTask): string {
  switch (task.kind) {
    case "github":
      return "Compte GitHub";
    case "repos":
      return "Dépôts suivis";
    case "secret":
      return task.secret.label;
    case "view":
      return task.step.title;
  }
}

/** Clé stable d'un écran (pour retrouver sa position quand la liste change). */
export const taskKey = (task: SetupTask) =>
  `${task.plugin.id}:${task.kind}:${task.kind === "secret" ? task.secret.id : task.kind === "view" ? task.step.id : ""}`;

/** Taille lisible d'un ensemble d'archives (« 31,2 Mo »). */
export function formatSize(bytes: number): string {
  if (bytes < 1_000_000) return `${Math.max(1, Math.round(bytes / 1000))} ko`;
  return `${(bytes / 1_000_000).toFixed(1).replace(".", ",")} Mo`;
}
