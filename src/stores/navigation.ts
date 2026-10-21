/**
 * Navigation par état, sans routeur d'URL (rien à exposer dans un webview local).
 * Chaque écran du canvas est une `View` typée : ses paramètres sont vérifiés à la compilation.
 *
 * La vue courante est gardée dans le `sessionStorage` du webview : un rechargement de la page
 * (rechargement du serveur de dev, du webview) rouvre le même écran, la conversation ouverte
 * comprise, au lieu de retomber sur une nouvelle conversation (recette du 21/10, P13).
 * Un nouveau lancement de l'app repart du chat.
 */
import { create } from "zustand";

/** Sections des réglages (V2-Settings.dc.html) ; `plugin` ouvre les réglages d'un plugin. */
export type SettingsSection = "general" | "ai" | "tools" | "repos" | "plugins" | "plugin" | "about";

/** Étapes de l'onboarding de Cadence 2 (V2-Onboarding-*.dc.html). */
export type OnboardingStep = "welcome" | "ai" | "plugins" | "tools" | "setup" | "done";

export type View =
  // Cadence 2 : onboarding plein écran, puis coque (rail, barre contextuelle, contenu)
  | { id: "onboarding"; step?: OnboardingStep }
  // `plugin` : nouvelle conversation ouverte depuis la barre de ce plugin.
  | { id: "chat"; session?: string; plugin?: string }
  | { id: "catalog"; plugin?: string }
  | { id: "plugin"; plugin: string; view: string; params?: Record<string, string> }
  | { id: "settings"; section?: SettingsSection; plugin?: string }
  // Cadence 1, écrans de gestion de projet (futur plugin gestion-projet)
  | { id: "onboarding-github" }
  | { id: "sprint" }
  | { id: "issues" }
  | { id: "issue"; number: number }
  | { id: "pull-requests" }
  | { id: "reviews"; pr?: number }
  | { id: "roadmap" }
  | { id: "version"; version?: string }
  // Outils de dev
  | { id: "design-system" };

export type ViewId = View["id"];
export type ViewOf<K extends ViewId> = Extract<View, { id: K }>;

interface NavigationState {
  view: View;
  history: View[];
  navigate: (view: View) => void;
  replace: (view: View) => void;
  back: () => void;
}

const MAX_HISTORY = 50;
const STORED_VIEW = "cadence.view";

/** Vue gardée pour un rechargement ; `null` si elle est illisible ou de l'onboarding. */
export function parseStoredView(
  raw: string | null,
  isViewId: (id: string) => boolean,
): View | null {
  if (!raw) return null;
  try {
    const view = JSON.parse(raw) as unknown;
    if (!view || typeof view !== "object" || Array.isArray(view)) return null;
    const id = (view as { id?: unknown }).id;
    if (typeof id !== "string" || !isViewId(id) || id === "onboarding") return null;
    return view as View;
  } catch {
    return null;
  }
}

/** Vue gardée par le dernier écran de cette fenêtre (avant un rechargement). */
export function storedView(isViewId: (id: string) => boolean): View | null {
  try {
    return parseStoredView(sessionStorage.getItem(STORED_VIEW), isViewId);
  } catch {
    return null;
  }
}

function keep(view: View) {
  try {
    sessionStorage.setItem(STORED_VIEW, JSON.stringify(view));
  } catch {
    // Stockage indisponible : un rechargement repartira du chat.
  }
}

export const useNavigation = create<NavigationState>((set) => ({
  view: { id: "chat" },
  history: [],
  navigate: (view) => {
    keep(view);
    set((s) => ({ view, history: [...s.history, s.view].slice(-MAX_HISTORY) }));
  },
  replace: (view) => {
    keep(view);
    set({ view });
  },
  back: () =>
    set((s) => {
      const previous = s.history.at(-1);
      if (previous) keep(previous);
      return previous ? { view: previous, history: s.history.slice(0, -1) } : s;
    }),
}));

/** Raccourci hors composant : `navigate({ id: "plugin", plugin: "documents", view: "bibliotheque" })`. */
export const navigate = (view: View) => useNavigation.getState().navigate(view);
