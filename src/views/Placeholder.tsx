import { Page } from "@/components/layout/Page";
import { OnboardingLayout } from "@/components/layout/OnboardingLayout";
import { EmptyState } from "@/components/layout/EmptyState";
import { Card } from "@/components/ui/Card";

/**
 * Écran pas encore implémenté. Pour l'implémenter : créer `src/views/<zone>/<Nom>View.tsx`
 * et remplacer l'appel à `placeholder(...)` dans `src/views/registry.tsx`.
 */
export function placeholder(title: string, mockup: string, layout: "app" | "onboarding" = "app") {
  function PlaceholderView() {
    const body = (
      <EmptyState icon="layers" title="Écran à implémenter">
        Maquette de référence : <span className="mono text-tx2">{mockup}</span> dans le canvas «
        Cadence — Écrans de l'app ».
      </EmptyState>
    );
    if (layout === "onboarding") {
      const step = mockup.includes("Agents") ? 1 : mockup.includes("Projet") ? 2 : 0;
      return (
        <OnboardingLayout step={step} title={title} intro="Écran à implémenter.">
          <Card className="max-w-[860px]">{body}</Card>
        </OnboardingLayout>
      );
    }
    return (
      <Page title={title}>
        <Card className="h-full">{body}</Card>
      </Page>
    );
  }
  PlaceholderView.displayName = `Placeholder(${mockup})`;
  return PlaceholderView;
}
