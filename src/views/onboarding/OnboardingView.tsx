/**
 * Onboarding de Cadence 2 (V2-Onboarding-*.dc.html) : Bienvenue → IA de code → Plugins →
 * Outils nécessaires → Configuration → Terminé. Chaque étape lit l'état réel (détection,
 * catalogue, plugins installés) : revenir en arrière ne perd rien.
 */
import type { ViewProps } from "@/views/registry";
import type { OnboardingStep } from "@/stores/navigation";
import { useNavigation } from "@/stores/navigation";
import { nextStep, previousStep } from "@/lib/domain/onboarding";
import { WelcomeStep } from "./steps/WelcomeStep";
import { AiStep } from "./steps/AiStep";
import { PluginsStep } from "./steps/PluginsStep";
import { ToolsStep } from "./steps/ToolsStep";
import { SetupStep } from "./steps/SetupStep";
import { DoneStep } from "./steps/DoneStep";

export interface StepProps {
  onNext: () => void;
  onBack: () => void;
}

export default function OnboardingView({ view }: ViewProps<"onboarding">) {
  const replace = useNavigation((s) => s.replace);
  const step: OnboardingStep = view.step ?? "welcome";
  const go = (to: OnboardingStep) => replace({ id: "onboarding", step: to });
  const props: StepProps = {
    onNext: () => go(nextStep(step)),
    onBack: () => go(previousStep(step)),
  };

  switch (step) {
    case "welcome":
      return <WelcomeStep {...props} />;
    case "ai":
      return <AiStep {...props} />;
    case "plugins":
      return <PluginsStep {...props} />;
    case "tools":
      return <ToolsStep {...props} />;
    case "setup":
      return <SetupStep {...props} />;
    case "done":
      return <DoneStep {...props} />;
  }
}
