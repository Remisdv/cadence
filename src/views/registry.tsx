/**
 * Registre des écrans : une entrée par vue du canvas « Cadence — Écrans de l'app ».
 *
 * `layout` décide du cadre : `app` = rail de Cadence 2 + écran (l'écran pose lui-même sa barre
 * contextuelle : conversations, barre du plugin, sections des réglages), `onboarding` = plein
 * écran.
 * Les vues sont chargées à la demande (`lazy`) pour garder un démarrage rapide ; un écran pas
 * encore fait passe par `placeholder()` (`./Placeholder`).
 */
import { lazy, type ComponentType } from "react";
import type { ViewId, ViewOf } from "@/stores/navigation";

export type ViewProps<K extends ViewId> = { view: ViewOf<K> };

export interface ViewDef<K extends ViewId> {
  title: string;
  /** Fichier de la maquette dans le canvas. */
  mockup: string;
  layout: "app" | "onboarding";
  component: ComponentType<ViewProps<K>>;
}

type Registry = { [K in ViewId]: ViewDef<K> };

export const registry: Registry = {
  // Cadence 2 : coque
  chat: {
    title: "Chat",
    mockup: "V2-Chat.dc.html",
    layout: "app",
    component: lazy(() => import("./chat/ChatView")),
  },
  plugin: {
    title: "Vue d'un plugin",
    mockup: "V2-Project-Issue.dc.html (barre du plugin)",
    layout: "app",
    component: lazy(() => import("./plugin/PluginView")),
  },
  catalog: {
    title: "Catalogue de plugins",
    mockup: "V2-Catalog.dc.html",
    layout: "app",
    component: lazy(() => import("./catalog/CatalogView")),
  },
  settings: {
    title: "Réglages",
    mockup: "V2-Settings.dc.html",
    layout: "app",
    component: lazy(() => import("./settings/SettingsView")),
  },

  // Cadence 1 : écrans de gestion de projet, compilés jusqu'au plugin gestion-projet
  "onboarding-github": {
    title: "Connecter votre compte GitHub",
    mockup: "Onboarding-Welcome.dc.html",
    layout: "onboarding",
    component: lazy(() => import("./onboarding/GithubView")),
  },

  // 2 · Travail quotidien — sprint et issues
  sprint: {
    title: "Sprint en cours",
    mockup: "Main.dc.html",
    layout: "app",
    component: lazy(() => import("./sprint/SprintView")),
  },
  issues: {
    title: "Issues",
    mockup: "Issues.dc.html",
    layout: "app",
    component: lazy(() => import("./issues/IssuesView")),
  },
  issue: {
    title: "Détail d'une issue",
    mockup: "Issue-Detail.dc.html",
    layout: "app",
    component: lazy(() => import("./issues/IssueDetailView")),
  },

  // 3 · Code, revues et planification
  "pull-requests": {
    title: "Pull requests",
    mockup: "Pull-Requests.dc.html",
    layout: "app",
    component: lazy(() => import("./pulls/PullRequestsView")),
  },
  reviews: {
    title: "Revues à faire",
    mockup: "Reviews.dc.html",
    layout: "app",
    component: lazy(() => import("./pulls/ReviewsView")),
  },
  roadmap: {
    title: "Roadmap",
    mockup: "Roadmap.dc.html",
    layout: "app",
    component: lazy(() => import("./roadmap/RoadmapView")),
  },

  // 4 · Versions
  version: {
    title: "Détail d'une version",
    mockup: "Version-Detail.dc.html",
    layout: "app",
    component: lazy(() => import("./versions/VersionView")),
  },

  // Outils de dev
  "design-system": {
    title: "Galerie de composants",
    mockup: "—",
    layout: "app",
    component: lazy(() => import("./dev/DesignSystemView")),
  },
};

export const isViewId = (id: string): id is ViewId => id in registry;

/** Précharge les écrans de la coque, pour que le premier clic du rail n'attende pas. */
export function preloadShellViews() {
  void import("./chat/ChatView");
  void import("./plugin/PluginView");
  void import("./catalog/CatalogView");
  void import("./settings/SettingsView");
}
