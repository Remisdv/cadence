/**
 * Données de démonstration, reprises des maquettes (dépôt cadence-app/desktop, sprint 0.1.2).
 * Elles servent au mode mock et à la galerie de composants.
 */
import type {
  AgentEngines,
  ProjectPreview,
  AgentOverview,
  AuthStatus,
  CliAgentInfo,
  Config,
  GithubAuth,
  IssueDetail,
  IssueRow,
  PrRow,
  ProjectState,
  Repo,
  ReviewRow,
  UserRef,
  Version,
  VersionDetail,
} from "@/lib/types";

export const me: UserRef = {
  login: "rlhuillier",
  name: "Rémi Lhuillier",
  avatarUrl: null,
  isSelf: true,
};

export const marc: UserRef = { login: "marcb", name: "Marc B.", avatarUrl: null, isSelf: false };

export const githubConnected: GithubAuth = {
  status: "connected",
  login: me.login,
  name: me.name,
  avatarUrl: null,
  scopes: ["repo", "read:org", "workflow", "read:user"],
};

export const githubDisconnected: GithubAuth = {
  status: "disconnected",
  login: null,
  name: null,
  avatarUrl: null,
  scopes: [],
};

/** Claude Code détecté et testé (état de la maquette). */
export const claudeConnected: CliAgentInfo = {
  status: "connected",
  binaryPath: "/opt/homebrew/bin/claude",
  version: "2.4.1",
  authMethod: "Compte Claude",
  test: { ok: true, latencyMs: 1200, message: null },
};

/** Claude Code détecté, session de test pas encore lancée (premier passage à l'onboarding). */
export const claudeDetected: CliAgentInfo = {
  ...claudeConnected,
  status: "pending",
  authMethod: null,
  test: null,
};

export const cliNotFound: CliAgentInfo = {
  status: "not-found",
  binaryPath: null,
  version: null,
  authMethod: null,
  test: null,
};

export const authConnected: AuthStatus = {
  github: githubConnected,
  claudeCode: claudeConnected,
  codex: cliNotFound,
};

export const authDisconnected: AuthStatus = {
  github: githubDisconnected,
  claudeCode: claudeDetected,
  codex: cliNotFound,
};

export const defaultEngines: AgentEngines = {
  cto: { engine: "claude-code", model: "opus" },
  ceo: { engine: "claude-code", model: "sonnet" },
  dev: { engine: "claude-code", model: "opus" },
  deploy: { engine: "claude-code", model: "sonnet" },
};

export const repos: Repo[] = [
  {
    fullName: "cadence-app/desktop",
    owner: "cadence-app",
    name: "desktop",
    description: "Client Tauri — gestion de projet sur GitHub",
    private: false,
    defaultBranch: "main",
    openIssues: 18,
    milestones: 6,
    pushedAt: "2026-09-17T08:12:00Z",
  },
  {
    fullName: "cadence-app/docs",
    owner: "cadence-app",
    name: "docs",
    description: "Documentation publique et changelog",
    private: false,
    defaultBranch: "main",
    openIssues: 4,
    milestones: 0,
    pushedAt: "2026-09-12T16:40:00Z",
  },
  {
    fullName: "cadence-app/actions",
    owner: "cadence-app",
    name: "actions",
    description: "Workflows de build et de release",
    private: false,
    defaultBranch: "main",
    openIssues: 1,
    milestones: 2,
    pushedAt: "2026-09-02T10:00:00Z",
  },
  {
    fullName: "rlhuillier/dotfiles",
    owner: "rlhuillier",
    name: "dotfiles",
    description: null,
    private: true,
    defaultBranch: "main",
    openIssues: 0,
    milestones: 0,
    pushedAt: "2026-08-30T10:00:00Z",
  },
];

export const project: ProjectState = {
  repo: repos[0]!,
  currentVersion: "0.1.2",
  hasConfig: true,
};

export const versions: Version[] = [
  {
    title: "0.1.0",
    kind: "sprint",
    state: "closed",
    milestoneNumber: 1,
    name: "Voir son projet",
    startOn: "2026-08-12",
    dueOn: "2026-08-26",
    openIssues: 0,
    closedIssues: 9,
    description: null,
  },
  {
    title: "0.1.1",
    kind: "sprint",
    state: "closed",
    milestoneNumber: 3,
    name: "Lecture du projet",
    startOn: "2026-08-26",
    dueOn: "2026-09-09",
    openIssues: 0,
    closedIssues: 14,
    description: null,
  },
  {
    title: "0.1.2",
    kind: "sprint",
    state: "open",
    milestoneNumber: 4,
    name: "Cockpit Tauri",
    startOn: "2026-09-09",
    dueOn: "2026-09-23",
    openIssues: 7,
    closedIssues: 11,
    description: null,
  },
  {
    title: "0.1.3",
    kind: "sprint",
    state: "open",
    milestoneNumber: 5,
    name: "Fiabilité",
    startOn: "2026-09-23",
    dueOn: "2026-10-07",
    openIssues: 5,
    closedIssues: 0,
    description: null,
  },
  {
    title: "0.2.0",
    kind: "palier",
    state: "open",
    milestoneNumber: 6,
    name: "Travail à plusieurs",
    startOn: null,
    dueOn: null,
    openIssues: 0,
    closedIssues: 0,
    description: "- Rôles et permissions\n- Revue assistée\n- Multi-dépôts",
  },
  {
    title: "1.0.0",
    kind: "palier",
    state: "open",
    milestoneNumber: 7,
    name: "Ouverture publique",
    startOn: null,
    dueOn: null,
    openIssues: 0,
    closedIssues: 0,
    description: "- Signature et notarisation\n- Mise à jour automatique",
  },
];

export const currentVersion: VersionDetail = {
  version: versions[2]!,
  daysLeft: 6,
  progress: 61,
  velocity: 2.4,
  delayDays: 1.5,
  burndown: [
    { date: "2026-09-09", remaining: 18 },
    { date: "2026-09-10", remaining: 18 },
    { date: "2026-09-11", remaining: 15 },
    { date: "2026-09-12", remaining: 15 },
    { date: "2026-09-13", remaining: 12 },
    { date: "2026-09-14", remaining: 12 },
    { date: "2026-09-15", remaining: 9 },
    { date: "2026-09-16", remaining: 7 },
  ],
};

const row = (r: Partial<IssueRow> & Pick<IssueRow, "number" | "title" | "status">): IssueRow => ({
  version: "0.1.2",
  issueType: "feat",
  labels: [],
  assignee: null,
  estimate: null,
  activeAgent: null,
  linkedPr: null,
  flags: [],
  updatedAt: "2026-09-16T10:00:00Z",
  ...r,
});

export const issues: IssueRow[] = [
  row({
    number: 151,
    title: "Persister la sélection de projet entre deux lancements",
    status: "todo",
    assignee: me,
    estimate: "S",
  }),
  row({ number: 153, title: "Raccourci global pour la palette de commandes", status: "todo" }),
  row({
    number: 156,
    title: "Message d'erreur lisible quand le quota API est atteint",
    status: "todo",
    issueType: "fix",
    labels: ["area/api"],
    flags: ["incomplete-description", "cto-questions"],
    assignee: me,
  }),
  row({
    number: 157,
    title: "Signature et notarisation macOS",
    status: "todo",
    issueType: "chore",
  }),
  row({
    number: 142,
    title: "Device flow OAuth GitHub + stockage trousseau",
    status: "doing",
    activeAgent: "dev",
    linkedPr: 88,
    estimate: "M",
  }),
  row({
    number: 147,
    title: "Cache local des issues avec invalidation par ETag",
    status: "doing",
    issueType: "perf",
    assignee: me,
  }),
  row({
    number: 149,
    title: "Bandeau de quota daté en régime gel",
    status: "doing",
    issueType: "fix",
    assignee: marc,
  }),
  row({
    number: 138,
    title: "Vue roadmap : lanes par palier de version",
    status: "review",
    linkedPr: 86,
    assignee: marc,
  }),
  row({
    number: 140,
    title: "Synchronisation incrémentale des milestones",
    status: "review",
    linkedPr: 84,
    flags: ["ci-failed"],
  }),
  row({
    number: 133,
    title: "Hauteur du rail latéral sur petits écrans",
    status: "review",
    issueType: "fix",
    linkedPr: 85,
    assignee: marc,
  }),
  row({ number: 145, title: "Journal des agents en local", status: "review", linkedPr: 87 }),
  row({
    number: 131,
    title: "Squelette Tauri 2 + routeur de vues",
    status: "done",
    issueType: "chore",
    linkedPr: 79,
  }),
  row({
    number: 134,
    title: "Client GitHub typé (octocrab) côté Rust",
    status: "done",
    linkedPr: 80,
  }),
  row({
    number: 135,
    title: "Pagination et ETag sur le client GitHub",
    status: "done",
    issueType: "perf",
  }),
  row({ number: 136, title: "Sélection de dépôt à l'onboarding", status: "done" }),
  row({ number: 137, title: "Migrations SQLite du cache", status: "done", issueType: "chore" }),
  row({ number: 139, title: "Tri semver des milestones", status: "done", issueType: "fix" }),
  row({
    number: 141,
    title: "Documentation du modèle de données",
    status: "done",
    issueType: "docs",
  }),
];

export function issueDetail(number: number): IssueDetail {
  const base = issues.find((i) => i.number === number) ?? issues[2]!;
  return {
    ...base,
    body: "Quand on dépasse le quota, l'app affiche une erreur brute de l'API.",
    htmlUrl: `https://github.com/cadence-app/desktop/issues/${base.number}`,
    author: me,
    createdAt: "2026-09-15T09:30:00Z",
    questions:
      base.number === 156
        ? [
            {
              id: "q1",
              question: "Faut-il un compte à rebours jusqu'à la réinitialisation du quota ?",
              answer: null,
            },
            {
              id: "q2",
              question: "Le message doit-il proposer de passer en mode économe ?",
              answer: null,
            },
          ]
        : [],
  };
}

const pr = (p: Partial<PrRow> & Pick<PrRow, "number" | "title" | "branch">): PrRow => ({
  base: "main",
  author: marc,
  agent: null,
  state: "open",
  ci: "success",
  review: null,
  linkedIssue: null,
  version: "0.1.2",
  additions: 0,
  deletions: 0,
  comments: 0,
  updatedAt: "2026-09-16T10:00:00Z",
  ...p,
});

export const pullRequests: PrRow[] = [
  pr({
    number: 88,
    title: "Device flow OAuth GitHub",
    branch: "feat/142-github-device-flow",
    author: me,
    agent: "dev",
    state: "draft",
    ci: "pending",
    linkedIssue: 142,
    additions: 412,
    deletions: 87,
  }),
  pr({
    number: 86,
    title: "Vue roadmap : lanes par palier",
    branch: "feat/138-roadmap-lanes",
    review: "approved",
    linkedIssue: 138,
    additions: 268,
    deletions: 41,
  }),
  pr({
    number: 85,
    title: "Hauteur du rail latéral",
    branch: "fix/133-rail-height",
    review: "pending",
    linkedIssue: 133,
    additions: 18,
    deletions: 6,
  }),
  pr({
    number: 84,
    title: "Synchronisation incrémentale des milestones",
    branch: "feat/140-incremental-sync",
    author: me,
    ci: "failure",
    review: "changes-requested",
    linkedIssue: 140,
    additions: 233,
    deletions: 58,
  }),
  pr({
    number: 80,
    title: "Client GitHub typé (octocrab)",
    branch: "chore/134-octocrab-client",
    state: "merged",
    review: "approved",
    linkedIssue: 134,
    additions: 540,
    deletions: 12,
  }),
];

export const reviews: ReviewRow[] = [
  { pr: pullRequests[3]!, audience: "you", requestedAt: "2026-09-15T09:00:00Z" },
  { pr: pullRequests[2]!, audience: "you", requestedAt: "2026-09-17T04:00:00Z" },
  { pr: pullRequests[1]!, audience: "others", requestedAt: "2026-09-16T12:00:00Z" },
];

export const config: Config = {
  project: { repo: "cadence-app/desktop", tagPrefix: "v" },
  versioning: { scheme: "semver", confirmMajorBump: true, sprintDays: 14 },
  issues: {
    requireMilestone: true,
    sendToCtoOnCreate: true,
    template: "contexte-attendu-criteres",
  },
  agents: {
    cto: {
      engine: "claude-code",
      model: "opus",
      autonomy: "apply-after-review",
      maxSessionMinutes: 30,
    },
    ceo: { engine: "claude-code", model: "sonnet", autonomy: "propose", maxSessionMinutes: 30 },
    dev: {
      engine: "claude-code",
      model: "opus",
      autonomy: "apply-after-review",
      maxSessionMinutes: 30,
    },
    deploy: {
      engine: "claude-code",
      model: "sonnet",
      autonomy: "apply-after-review",
      maxSessionMinutes: 30,
    },
    worktreeRoot: "~/.cadence/wt",
  },
};

export const agentsOverview: AgentOverview[] = [
  {
    kind: "cto",
    autonomy: "apply-after-review",
    activity: "idle",
    pendingActions: 0,
    currentRun: null,
    lastRunAt: "2026-09-17T07:00:00Z",
  },
  {
    kind: "ceo",
    autonomy: "propose",
    activity: "waiting",
    pendingActions: 5,
    currentRun: null,
    lastRunAt: "2026-09-17T09:00:00Z",
  },
  {
    kind: "dev",
    autonomy: "apply-after-review",
    activity: "running",
    pendingActions: 0,
    // Démarré il y a 4 min 12 s, pour que le minuteur du rail affiche une durée réaliste.
    currentRun: {
      run: "run-dev-142",
      state: "running",
      issue: 142,
      step: 3,
      steps: 6,
      startedAt: new Date(Date.now() - 252_000).toISOString(),
    },
    lastRunAt: "2026-09-17T09:55:48Z",
  },
  {
    kind: "deploy",
    autonomy: "apply-after-review",
    activity: "idle",
    pendingActions: 0,
    currentRun: null,
    lastRunAt: "2026-09-16T18:24:00Z",
  },
];

/** Prévisualisation d'un dépôt avant ouverture (onboarding 3). */
export function projectPreview(fullName: string): ProjectPreview {
  const repo = repos.find((r) => r.fullName === fullName) ?? repos[0]!;
  if (repo.fullName === "cadence-app/desktop") {
    return { repo, versions, currentVersion: "0.1.2", ignoredMilestones: [], hasConfig: true };
  }
  if (repo.fullName === "cadence-app/actions") {
    return {
      repo,
      versions: [{ ...versions[4]!, milestoneNumber: 1 }],
      currentVersion: "0.2.0",
      ignoredMilestones: ["Backlog"],
      hasConfig: false,
    };
  }
  return { repo, versions: [], currentVersion: null, ignoredMilestones: [], hasConfig: false };
}
