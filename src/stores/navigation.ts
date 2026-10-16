/**
 * Navigation par état, sans routeur d'URL (rien à exposer dans un webview local).
 * Chaque écran du canvas est une `View` typée : ses paramètres sont vérifiés à la compilation.
 */
import { create } from "zustand";

export type SettingsSection =
  "accounts" | "machine" | "appearance" | "shortcuts" | "about";

export type View =
  // 1 · Connexion et configuration
  | { id: "onboarding-github" }
  // 2 · Travail quotidien
  | { id: "sprint" }
  | { id: "issues" }
  | { id: "issue"; number: number }
  // 3 · Code, revues et planification
  | { id: "pull-requests" }
  | { id: "reviews"; pr?: number }
  | { id: "roadmap" }
  // 4 · Versions
  | { id: "version"; version?: string }
  // 5 · Réglages
  | { id: "settings"; section?: SettingsSection }
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
  view: { id: "sprint" },
  history: [],
  navigate: (view) => set((s) => ({ view, history: [...s.history, s.view].slice(-MAX_HISTORY) })),
  replace: (view) => set({ view }),
  back: () =>
    set((s) => {
      const previous = s.history.at(-1);
      return previous ? { view: previous, history: s.history.slice(0, -1) } : s;
    }),
}));

/** Raccourci hors composant : `navigate({ id: "issue", number: 156 })`. */
export const navigate = (view: View) => useNavigation.getState().navigate(view);
