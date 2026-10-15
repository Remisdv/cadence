/**
 * Données de démonstration, reprises des maquettes (dépôt cadence-app/desktop, sprint 0.1.2).
 * Elles servent au mode mock et à la galerie de composants.
 */
import type {
  AgentEngines,
  AgentOverview,
  AuthStatus,
  CliAgentInfo,
  Config,
  GithubAuth,
  GithubSources,
  IssueDetail,
  IssueRow,
  PrDetail,
  PrFile,
  PrRow,
  ProjectState,
  RepoOwner,
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

/** Moyens de connexion détectés sur la machine (comme sur le poste de dev). */
export const githubSources: GithubSources = {
  ghCli: "rlhuillier",
  credentialManager: true,
  oauthApp: false,
  deviceFlow: false,
};

export const authConnected: AuthStatus = {
  github: githubConnected,
  githubSources,
  claudeCode: claudeConnected,
  codex: cliNotFound,
};

export const authDisconnected: AuthStatus = {
  github: githubDisconnected,
  githubSources,
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

/** Il y a `hours` heures : les attentes de revue (« 2 j », « 6 h ») restent vraies. */
const hoursAgo = (hours: number) => new Date(Date.now() - hours * 3_600_000).toISOString();

const pr = (p: Partial<PrRow> & Pick<PrRow, "number" | "title" | "branch">): PrRow => ({
  base: "main",
  author: me,
  agent: null,
  state: "open",
  ci: "success",
  ciPassed: 4,
  ciTotal: 4,
  review: null,
  approvals: 0,
  changesRequested: 0,
  reviewRequestedAt: null,
  linkedIssue: null,
  version: "0.1.2",
  additions: 0,
  deletions: 0,
  changedFiles: 1,
  comments: 0,
  createdAt: hoursAgo(72),
  updatedAt: hoursAgo(5),
  ...p,
});

export const pullRequests: PrRow[] = [
  pr({
    number: 88,
    title: "feat(auth) : device flow GitHub + stockage trousseau",
    branch: "feat/142-github-device-flow",
    agent: "dev",
    state: "draft",
    ci: "pending",
    ciPassed: 1,
    linkedIssue: 142,
    additions: 286,
    deletions: 31,
    changedFiles: 7,
    updatedAt: hoursAgo(0.1),
  }),
  pr({
    number: 86,
    title: "feat(roadmap) : lanes par palier de version",
    branch: "feat/138-roadmap-lanes",
    author: marc,
    review: "pending",
    approvals: 1,
    reviewRequestedAt: hoursAgo(6),
    linkedIssue: 138,
    additions: 412,
    deletions: 87,
    changedFiles: 11,
    updatedAt: hoursAgo(1),
  }),
  pr({
    number: 85,
    title: "feat(sync) : synchronisation incrémentale des milestones",
    branch: "feat/140-milestone-sync",
    ci: "failure",
    ciPassed: 3,
    review: "changes-requested",
    changesRequested: 1,
    reviewRequestedAt: hoursAgo(30),
    linkedIssue: 140,
    additions: 238,
    deletions: 44,
    changedFiles: 6,
    updatedAt: hoursAgo(3),
  }),
  pr({
    number: 84,
    title: "fix(ui) : hauteur du rail sur petits écrans",
    branch: "fix/133-rail-height",
    author: marc,
    review: "pending",
    reviewRequestedAt: hoursAgo(50),
    additions: 18,
    deletions: 6,
    changedFiles: 2,
    createdAt: hoursAgo(50),
    updatedAt: hoursAgo(26),
  }),
  pr({
    number: 79,
    title: "chore(api) : client GitHub typé avec octocrab",
    branch: "chore/134-octocrab-client",
    state: "merged",
    review: "approved",
    approvals: 2,
    linkedIssue: 134,
    additions: 906,
    deletions: 212,
    changedFiles: 23,
    updatedAt: hoursAgo(80),
  }),
  pr({
    number: 76,
    title: "feat(core) : squelette Tauri 2 et routeur de vues",
    branch: "feat/131-tauri-shell",
    state: "merged",
    review: "approved",
    approvals: 1,
    version: "0.1.1",
    additions: 1204,
    deletions: 0,
    changedFiles: 41,
    updatedAt: hoursAgo(400),
  }),
];

const prByNumber = (n: number) => pullRequests.find((p) => p.number === n)!;

export const reviews: ReviewRow[] = [
  { pr: prByNumber(84), audience: "you", requestedAt: prByNumber(84).reviewRequestedAt! },
  { pr: prByNumber(85), audience: "others", requestedAt: prByNumber(85).reviewRequestedAt! },
  { pr: prByNumber(86), audience: "you", requestedAt: prByNumber(86).reviewRequestedAt! },
];

const railPatch = [
  "@@ -1,9 +1,10 @@",
  " .rail {",
  "   display: flex;",
  "-  height: 100vh;",
  "+  height: 100%;",
  "+  min-height: 0;",
  "   flex-direction: column;",
  " }",
  " ",
  " .rail__nav {",
  "-  overflow: visible;",
  "+  overflow-y: auto;",
  "+  scrollbar-width: thin;",
  " }",
].join("\n");

/** Détail d'une PR : description et fichiers (patchs réalistes pour #84, génériques sinon). */
export function prDetailFor(number: number): PrDetail | undefined {
  const pr = pullRequests.find((p) => p.number === number);
  if (!pr) return undefined;
  const files: PrFile[] =
    number === 84
      ? [
          { path: "src/views/rail.css", status: "modified", additions: 12, deletions: 4, patch: railPatch },
          {
            path: "src/views/rail.tsx",
            status: "modified",
            additions: 6,
            deletions: 2,
            patch: "@@ -12,4 +12,8 @@\n export function Rail() {\n-  return <nav className=\"rail\">\n+  return (\n+    <nav className=\"rail\" aria-label=\"Navigation\">\n }",
          },
        ]
      : [
          {
            path: `src/${pr.branch.split("/")[1] ?? "change"}.ts`,
            status: "modified",
            additions: pr.additions,
            deletions: pr.deletions,
            patch: "@@ -1,3 +1,4 @@\n import { invoke } from \"@tauri-apps/api/core\";\n+import { call } from \"@/lib/ipc/transport\";\n \n-export const run = () => invoke(\"run\");\n+export const run = () => call(\"run\");",
          },
          { path: "assets/logo.png", status: "added", additions: 0, deletions: 0, patch: null },
        ];
  return {
    pr,
    body:
      number === 84
        ? "Corrige le débordement du rail sous 720 px de hauteur : passage de `100vh` à une hauteur fluide et défilement interne de la zone de navigation.\n\nAucun changement de comportement au-dessus de 720 px."
        : null,
    url: `https://github.com/cadence-app/desktop/pull/${number}`,
    files,
  };
}

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

/** Comptes sous lesquels créer un dépôt (mock). */
export const repoOwners: RepoOwner[] = [
  { login: me.login, kind: "user", avatarUrl: null },
  { login: "cadence-app", kind: "organization", avatarUrl: null },
];

/** Projet ouvert pour un dépôt donné (sélecteur de dépôt de la barre latérale). */
export function projectFor(fullName: string): ProjectState {
  const repo = repos.find((r) => r.fullName === fullName) ?? repos[0]!;
  const currentVersion =
    repo.fullName === "cadence-app/desktop" ? "0.1.2" : repo.milestones > 0 ? "0.2.0" : null;
  return { repo, currentVersion, hasConfig: repo.fullName === "cadence-app/desktop" };
}
