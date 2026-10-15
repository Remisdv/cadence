import type { IssueFilter, PrFilter, SyncResource } from "@/lib/types";

/**
 * Clés de cache TanStack Query, regroupées par ressource. La première clé de chaque groupe
 * correspond à une `SyncResource` pour que `sync://updated` invalide le bon groupe.
 */
export const keys = {
  auth: ["auth"] as const,
  project: ["project"] as const,
  projectCurrent: () => ["project", "current"] as const,
  repos: (query: string) => ["project", "repos", query] as const,
  projectPreview: (repo: string) => ["project", "preview", repo] as const,

  versions: ["milestones"] as const,
  versionList: () => ["milestones", "list"] as const,
  versionCurrent: () => ["milestones", "current"] as const,

  issues: ["issues"] as const,
  issueList: (filter: IssueFilter) => ["issues", "list", filter] as const,
  issue: (number: number) => ["issues", "detail", number] as const,

  prs: ["pull-requests"] as const,
  prList: (filter: PrFilter) => ["pull-requests", "list", filter] as const,

  reviews: ["reviews"] as const,
  reviewsPending: () => ["reviews", "pending"] as const,

  agents: ["agents"] as const,
  agentsOverview: () => ["agents", "overview"] as const,
  agentEngines: () => ["agents", "engines"] as const,
  agentActions: (run: string) => ["agents", "actions", run] as const,

  settings: ["settings"] as const,
};

/** Groupe de requêtes à invalider quand une ressource GitHub change. */
export const keysForResource: Record<SyncResource, readonly (readonly string[])[]> = {
  milestones: [keys.versions],
  issues: [keys.issues, keys.versions],
  "pull-requests": [keys.prs, keys.issues],
  reviews: [keys.reviews, keys.prs],
  labels: [keys.issues],
  releases: [keys.versions],
};
