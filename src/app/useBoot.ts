import { useEffect, useState } from "react";
import { useAuthStatus, useCurrentProject } from "@/lib/query/queries";
import { useNavigation, type View } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";
import { isViewId } from "@/views/registry";

/**
 * Choisit l'écran de départ : onboarding tant que GitHub n'est pas connecté, sinon le sprint
 * du dépôt mémorisé (ou le sélecteur de dépôt ouvert, s'il n'y en a pas encore).
 *
 * En dev, `?view=<id>&<param>=…` force un écran (ex. `?view=agent&kind=dev`, `?view=issue&number=156`).
 */
export function useBoot(): { ready: boolean; error: unknown } {
  const auth = useAuthStatus();
  const connected = auth.data?.github.status === "connected";
  const project = useCurrentProject(connected);
  const replace = useNavigation((s) => s.replace);
  const openPicker = useUiStore((s) => s.setProjectPickerOpen);
  const [ready, setReady] = useState(false);

  const settled = auth.isSuccess && (!connected || project.isSuccess || project.isError);

  useEffect(() => {
    if (ready || !settled) return;
    const forced = import.meta.env.DEV ? forcedView() : null;
    if (forced) replace(forced);
    else if (!connected) replace({ id: "onboarding-github" });
    else {
      replace({ id: "sprint" });
      // Aucun dépôt mémorisé : le sélecteur de la barre latérale s'ouvre de lui-même.
      if (!project.data) openPicker(true);
    }
    setReady(true);
  }, [ready, settled, connected, project.data, replace, openPicker]);

  return { ready, error: auth.error };
}

function forcedView(): View | null {
  const params = new URLSearchParams(window.location.search);
  const id = params.get("view");
  if (!id || !isViewId(id)) return null;
  const view: Record<string, string | number> = { id };
  params.forEach((value, key) => {
    if (key === "view" || key === "scenario") return;
    view[key] = /^\d+$/.test(value) ? Number(value) : value;
  });
  return view as unknown as View;
}
