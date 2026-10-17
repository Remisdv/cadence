import type { IssueFilter, PrFilter, SyncResource } from "@/lib/types";

/**
 * Clés de cache TanStack Query, regroupées par ressource. La première clé de chaque groupe
 * correspond à une `SyncResource` pour que `sync://updated` invalide le bon groupe.
 */
export const keys = {
  // Cadence 2 : app, préférences de la machine, plugins installés.
  appStatus: () => ["app", "status"] as const,
  appSettings: () => ["app", "settings"] as const,
  plugins: ["plugins"] as const,
  pluginsInstalled: () => ["plugins", "installed"] as const,
  pluginSettings: (plugin: string) => ["plugins", "settings", plugin] as const,
  pluginSecret: (plugin: string, secret: string) => ["plugins", "secret", plugin, secret] as const,
  pluginStatus: () => ["plugins", "status"] as const,
  catalog: () => ["plugins", "catalog"] as const,
  chat: ["chat"] as const,
  chatSessions: () => ["chat", "sessions"] as const,
  chatSession: (id: string) => ["chat", "session", id] as const,
  /** Textes en cours de diffusion d'une conversation (deltas), par message. */
  chatDrafts: (id: string) => ["chat", "drafts", id] as const,

  auth: ["auth"] as const,
  project: ["project"] as const,
  projectCurrent: () => ["project", "current"] as const,
  repos: (query: string) => ["project", "repos", query] as const,
  projectOwners: () => ["project", "owners"] as const,

  versions: ["milestones"] as const,
  versionList: () => ["milestones", "list"] as const,
  versionCurrent: () => ["milestones", "current"] as const,
  versionDetail: (title: string | null) => ["milestones", "detail", title ?? ""] as const,

  issues: ["issues"] as const,
  issueList: (filter: IssueFilter) => ["issues", "list", filter] as const,
  issue: (number: number) => ["issues", "detail", number] as const,

  prs: ["pull-requests"] as const,
  prList: (filter: PrFilter) => ["pull-requests", "list", filter] as const,

  reviews: ["reviews"] as const,
  reviewsPending: () => ["reviews", "pending"] as const,
  prDetail: (number: number) => ["pull-requests", "detail", number] as const,

  // Hors GitHub : version de l'app et mise à jour automatique.
  appVersion: () => ["app", "version"] as const,
  appUpdate: () => ["app", "update"] as const,
};

/** Groupe de requêtes à invalider quand une ressource GitHub change. */
export const keysForResource: Record<SyncResource, readonly (readonly string[])[]> = {
  milestones: [keys.versions],
  issues: [keys.issues, keys.versions],
  "pull-requests": [keys.prs, keys.issues, keys.versions],
  reviews: [keys.reviews, keys.prs],
  labels: [keys.issues],
  releases: [keys.versions],
};
