import { useEffect, useState } from "react";
import { useAppStatus } from "@/lib/query/queries";
import { storedView, useNavigation, type View } from "@/stores/navigation";
import { isViewId, preloadShellViews } from "@/views/registry";

/**
 * Choisit l'écran de départ : l'onboarding de Cadence 2 (Bienvenue → IA → plugins → outils →
 * configuration) tant que `onboarded` est faux, le chat global ensuite. Après un rechargement
 * du webview, l'écran d'avant (la conversation ouverte comprise) est rouvert.
 *
 * En dev, `?view=<id>&<param>=…` force un écran : `?view=settings&section=plugins`,
 * `?view=plugin&plugin=documents&page=flashcards`, `?view=issue&number=156`.
 */
export function useBoot(): { ready: boolean; error: unknown } {
  const status = useAppStatus();
  const replace = useNavigation((s) => s.replace);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (ready || !status.isSuccess) return;
    const forced = import.meta.env.DEV ? forcedView() : null;
    const reloaded = status.data.onboarded ? storedView(isViewId) : null;
    replace(forced ?? reloaded ?? (status.data.onboarded ? { id: "chat" } : { id: "onboarding" }));
    setReady(true);
    preloadShellViews();
  }, [ready, status.isSuccess, status.data, replace]);

  return { ready, error: status.error };
}

function forcedView(): View | null {
  const params = new URLSearchParams(window.location.search);
  const id = params.get("view");
  if (!id || !isViewId(id)) return null;
  const view: Record<string, string | number> = { id };
  params.forEach((value, key) => {
    if (key === "view" || key === "scenario") return;
    // `?view=plugin&plugin=documents&page=flashcards` : `page` donne la vue du plugin.
    const name = id === "plugin" && key === "page" ? "view" : key;
    view[name] = /^\d+$/.test(value) ? Number(value) : value;
  });
  return view as unknown as View;
}
