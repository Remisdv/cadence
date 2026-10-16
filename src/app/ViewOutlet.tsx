import { Suspense, type ComponentType } from "react";
import { AppRail } from "@/components/layout/AppRail";
import { EmptyState, LoadingState } from "@/components/layout/EmptyState";
import { Page } from "@/components/layout/Page";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { TooltipProvider } from "@/components/ui/Tooltip";
import { useCurrentProject } from "@/lib/query/queries";
import { useNavigation, type View, type ViewId } from "@/stores/navigation";
import { registry } from "@/views/registry";

/** Écrans de Cadence 1 qui lisent le dépôt ouvert (futur plugin gestion-projet). */
const projectViews: ViewId[] = [
  "sprint",
  "issues",
  "issue",
  "pull-requests",
  "reviews",
  "roadmap",
  "version",
];

/**
 * Rend la vue courante dans son cadre : coque de Cadence 2 (rail 56px, puis la barre
 * contextuelle et le contenu posés par l'écran), ou plein écran pour l'onboarding.
 */
export function ViewOutlet() {
  const view = useNavigation((s) => s.view);
  const def = registry[view.id];
  const Component = def.component as ComponentType<{ view: View }>;

  if (def.layout === "onboarding") {
    return (
      <Suspense fallback={<LoadingState />}>
        <Component key={JSON.stringify(view)} view={view} />
      </Suspense>
    );
  }

  return (
    <TooltipProvider delayDuration={250}>
      <div className="flex h-full">
        <AppRail />
        {projectViews.includes(view.id) ? (
          <ProjectView view={view} Component={Component} title={def.title} />
        ) : (
          <Suspense fallback={<LoadingState />}>
            {/* La clé remonte l'écran quand ses paramètres changent (ex. une autre vue). */}
            <Component key={JSON.stringify(view)} view={view} />
          </Suspense>
        )}
      </div>
    </TooltipProvider>
  );
}

function ProjectView({
  view,
  Component,
  title,
}: {
  view: View;
  Component: ComponentType<{ view: View }>;
  title: string;
}) {
  const project = useCurrentProject();
  if (project.isSuccess && !project.data) return <NoProject title={title} />;
  return (
    <Suspense fallback={<LoadingState />}>
      <Component key={JSON.stringify(view)} view={view} />
    </Suspense>
  );
}

function NoProject({ title }: { title: string }) {
  const navigate = useNavigation((s) => s.navigate);
  return (
    <Page title={title} search={false}>
      <Card className="h-full">
        <EmptyState
          icon="folder"
          title="Aucun dépôt ouvert"
          action={
            <Button variant="primary" icon="grid" onClick={() => navigate({ id: "catalog" })}>
              Ouvrir le catalogue
            </Button>
          }
        >
          Les écrans de gestion de projet de Cadence 1 deviennent le plugin Gestion de projet, qui
          choisira ses dépôts avec le service Dépôts.
        </EmptyState>
      </Card>
    </Page>
  );
}
