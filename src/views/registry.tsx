/**
 * Registre des écrans : une entrée par vue du canvas « Cadence — Écrans de l'app ».
 *
 * `layout` décide du cadre : `app` = rail latéral + écran, `onboarding` = plein écran.
 * Les écrans non implémentés passent par `placeholder()` ; les vraies vues sont chargées
 * à la demande (`lazy`) pour garder un démarrage rapide.
 */
import { lazy, type ComponentType } from "react";
import type { ViewId, ViewOf } from "@/stores/navigation";
import { placeholder } from "./Placeholder";

export type ViewProps<K extends ViewId> = { view: ViewOf<K> };

export interface ViewDef<K extends ViewId> {
  title: string;
  /** Fichier de la maquette dans le canvas. */
  mockup: string;
  layout: "app" | "onboarding";
  component: ComponentType<ViewProps<K>>;
}

type Registry = { [K in ViewId]: ViewDef<K> };

const app = <K extends ViewId>(title: string, mockup: string): ViewDef<K> => ({
  title,
  mockup,
  layout: "app",
  component: placeholder(title, mockup),
});

export const registry: Registry = {
  // 1 · Connexion et configuration
  "onboarding-github": {
    title: "Connecter votre compte GitHub",
    mockup: "Onboarding-Welcome.dc.html",
    layout: "onboarding",
    component: lazy(() => import("./onboarding/GithubView")),
  },
  "onboarding-agents": {
    title: "Brancher vos agents de code",
    mockup: "Onboarding-Agents.dc.html",
    layout: "onboarding",
    component: lazy(() => import("./onboarding/AgentsView")),
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

  // 4 · Versions et pilotage des agents
  version: {
    title: "Détail d'une version",
    mockup: "Version-Detail.dc.html",
    layout: "app",
    component: lazy(() => import("./versions/VersionView")),
  },
  agents: {
    title: "Console des agents",
    mockup: "Agents-Hub.dc.html",
    layout: "app",
    component: lazy(() => import("./agents/AgentsConsoleView")),
  },
  agent: {
    title: "Agent",
    mockup: "Agent-CTO.dc.html · Agent-CEO · Agent-Dev · Agent-Deploy",
    layout: "app",
    component: lazy(() => import("./agents/AgentView")),
  },

  // 5 · Réglages
  settings: {
    title: "Réglages",
    mockup: "Settings.dc.html",
    layout: "app",
    component: lazy(() => import("./settings/SettingsView")),
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
