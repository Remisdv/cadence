/**
 * Navigation par état, sans routeur d'URL (rien à exposer dans un webview local).
 * Chaque écran du canvas est une `View` typée : ses paramètres sont vérifiés à la compilation.
 */
import { create } from "zustand";

/** Sections des réglages (V2-Settings.dc.html) ; `plugin` ouvre les réglages d'un plugin. */
export type SettingsSection = "general" | "ai" | "repos" | "plugins" | "plugin" | "about";

/** Étapes de l'onboarding de Cadence 2 (V2-Onboarding-*.dc.html). */
export type OnboardingStep = "welcome" | "ai" | "plugins" | "tools" | "setup" | "done";

export type View =
  // Cadence 2 : onboarding plein écran, puis coque (rail, barre contextuelle, contenu)
  | { id: "onboarding"; step?: OnboardingStep }
  | { id: "chat"; session?: string }
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

export const useNavigation = create<NavigationState>((set) => ({
  view: { id: "chat" },
  history: [],
  navigate: (view) => set((s) => ({ view, history: [...s.history, s.view].slice(-MAX_HISTORY) })),
  replace: (view) => set({ view }),
  back: () =>
    set((s) => {
      const previous = s.history.at(-1);
      return previous ? { view: previous, history: s.history.slice(0, -1) } : s;
    }),
}));

/** Raccourci hors composant : `navigate({ id: "plugin", plugin: "documents", view: "bibliotheque" })`. */
export const navigate = (view: View) => useNavigation.getState().navigate(view);
