import { Suspense, type ComponentType } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { EmptyState, LoadingState } from "@/components/layout/EmptyState";
import { Page } from "@/components/layout/Page";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { useCurrentProject } from "@/lib/query/queries";
import { useNavigation, type View, type ViewId } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";
import { registry } from "@/views/registry";

/** Écrans utilisables sans dépôt choisi. */
const projectFree: ViewId[] = ["settings", "design-system"];

/** Rend la vue courante dans son cadre (rail latéral pour l'app, plein écran pour l'onboarding). */
export function ViewOutlet() {
  const view = useNavigation((s) => s.view);
  const project = useCurrentProject();
  const def = registry[view.id];
  const Component = def.component as ComponentType<{ view: View }>;

  if (def.layout === "onboarding") {
    return (
      <Suspense fallback={<LoadingState />}>
        <Component key={JSON.stringify(view)} view={view} />
      </Suspense>
    );
  }

  const needsProject = !projectFree.includes(view.id) && project.isSuccess && !project.data;

  return (
    <div className="flex h-full">
      <Sidebar />
      {needsProject ? (
        <NoProject title={def.title} />
      ) : (
        <Suspense fallback={<LoadingState />}>
          {/* La clé remonte l'écran quand ses paramètres changent (ex. une autre issue). */}
          <Component key={JSON.stringify(view)} view={view} />
        </Suspense>
      )}
    </div>
  );
}

function NoProject({ title }: { title: string }) {
  const openPicker = useUiStore((s) => s.setProjectPickerOpen);
  return (
    <Page title={title} search={false}>
      <Card className="h-full">
        <EmptyState
          icon="folder"
          title="Choisissez un dépôt"
          action={
            <Button variant="primary" icon="folder" onClick={() => openPicker(true)}>
              Choisir un dépôt
            </Button>
          }
        >
          Cadence travaille sur un dépôt à la fois, n'importe lequel de vos dépôts GitHub. Il se
          choisit dans la barre latérale et reste ouvert au prochain lancement.
        </EmptyState>
      </Card>
    </Page>
  );
}
