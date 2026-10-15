import { Suspense, type ComponentType } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { LoadingState } from "@/components/layout/EmptyState";
import { useNavigation, type View } from "@/stores/navigation";
import { registry } from "@/views/registry";

/** Rend la vue courante dans son cadre (rail latéral pour l'app, plein écran pour l'onboarding). */
export function ViewOutlet() {
  const view = useNavigation((s) => s.view);
  const def = registry[view.id];
  const Component = def.component as ComponentType<{ view: View }>;

  const screen = (
    <Suspense fallback={<LoadingState />}>
      {/* La clé remonte l'écran quand ses paramètres changent (ex. une autre issue). */}
      <Component key={JSON.stringify(view)} view={view} />
    </Suspense>
  );

  if (def.layout === "onboarding") return screen;

  return (
    <div className="flex h-full">
      <Sidebar />
      {screen}
    </div>
  );
}
