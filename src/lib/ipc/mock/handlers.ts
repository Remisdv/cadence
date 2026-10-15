/**
 * Implémentation mock des commandes. Chaque commande du contrat DOIT avoir un handler :
 * le type `Handlers` le vérifie à la compilation.
 *
 * Scénario de démarrage via l'URL :
 * - `?scenario=app` (défaut) : connecté, projet ouvert ;
 * - `?scenario=onboarding` : compte GitHub non connecté ;
 * - `?scenario=no-project` : connecté, aucun projet choisi.
 *
 * Issue du device flow simulé : `&flow=ok` (défaut, autorisé après 6 s), `denied`, `expired`,
 * `config` (client ID absent). Session de test de Claude Code : `&cli=ok` (défaut) ou `fail`.
 */
import type { AuthChanged, CliAgentInfo, ProjectState } from "@/lib/types";
import type { CommandArgs, CommandName, CommandResult } from "../commands";
import { CommandError } from "../errors";
import { mockBus } from "./bus";
import * as data from "./fixtures";

type Handlers = {
  [C in CommandName]: (args: CommandArgs<C>) => CommandResult<C> | Promise<CommandResult<C>>;
};

const params = new URLSearchParams(window.location.search);
const scenario = params.get("scenario") ?? "app";
const flow = params.get("flow") ?? "ok";
const cliOutcome = params.get("cli") ?? "ok";

const state = {
  github: scenario === "onboarding" ? data.githubDisconnected : data.githubConnected,
  claudeCode: scenario === "onboarding" ? data.claudeDetected : data.claudeConnected,
  codex: data.cliNotFound,
  engines: data.defaultEngines,
  project: scenario === "app" ? data.project : (null as ProjectState | null),
};
let flowTimer: number | undefined;

const LATENCY_MS = 120;
const delay = (ms = LATENCY_MS) => new Promise((r) => setTimeout(r, ms));

function fail(
  command: string,
  message: string,
  kind: "validation" | "internal" | "config" = "validation",
): never {
  throw new CommandError(command, { kind, message });
}

function emitAuth(event: Omit<AuthChanged, "provider">) {
  mockBus.emit<AuthChanged>("auth://changed", { provider: "github", ...event });
}

function cliKey(provider: string): "claudeCode" | "codex" {
  if (provider === "github") fail("cli", "GitHub n'est pas un agent de code.");
  return provider === "codex" ? "codex" : "claudeCode";
}

const handlers: Handlers = {
  auth_start_device_flow: () => {
    if (flow === "config") {
      fail(
        "auth_start_device_flow",
        "Client ID GitHub absent : créer une OAuth App avec le device flow activé, puis définir CADENCE_GITHUB_CLIENT_ID. En attendant, un jeton personnel fonctionne.",
        "config",
      );
    }
    window.clearTimeout(flowTimer);
    flowTimer = window.setTimeout(() => {
      if (flow === "denied") return emitAuth({ status: "error", reason: "denied", message: null });
      if (flow === "expired")
        return emitAuth({ status: "error", reason: "expired", message: null });
      state.github = data.githubConnected;
      emitAuth({ status: "connected", reason: null, message: null });
    }, 6000);
    return {
      userCode: "WDJB-MJHT",
      verificationUri: "https://github.com/login/device",
      expiresIn: flow === "expired" ? 8 : 900,
    };
  },
  auth_status: () => ({ github: state.github, claudeCode: state.claudeCode, codex: state.codex }),
  auth_set_token: ({ token }) => {
    if (!token.trim()) fail("auth_set_token", "Le jeton est vide.");
    if (!/^(ghp_|github_pat_|gho_)/.test(token.trim()))
      fail("auth_set_token", "GitHub refuse ce jeton.");
    window.clearTimeout(flowTimer);
    state.github = data.githubConnected;
    emitAuth({ status: "connected", reason: null, message: null });
    return state.github;
  },
  auth_sign_out: () => {
    state.github = data.githubDisconnected;
    emitAuth({ status: "disconnected", reason: null, message: null });
    return null;
  },
  cli_agent_test: async ({ provider }) => {
    const key = cliKey(provider);
    const current = state[key];
    if (!current.binaryPath)
      fail("cli_agent_test", "Binaire introuvable : impossible de le tester.");
    await delay(1200);
    const ok = cliOutcome !== "fail";
    const next: CliAgentInfo = {
      ...current,
      status: current.status === "disconnected" ? "disconnected" : ok ? "connected" : "error",
      authMethod: "Compte Claude",
      test: {
        ok,
        latencyMs: 1240,
        message: ok
          ? null
          : "Failed to authenticate: OAuth session expired and could not be refreshed",
      },
    };
    state[key] = next;
    return next;
  },
  cli_agent_set_path: ({ provider, path }) => {
    const key = cliKey(provider);
    state[key] = {
      status: "connected",
      binaryPath: path,
      version: "0.46.0",
      authMethod: null,
      test: null,
    };
    return state[key];
  },
  cli_agent_set_enabled: ({ provider, enabled }) => {
    const key = cliKey(provider);
    const current = state[key];
    const status = !enabled
      ? "disconnected"
      : !current.binaryPath
        ? "not-found"
        : current.test?.ok
          ? "connected"
          : "pending";
    state[key] = { ...current, status };
    return state[key];
  },

  project_current: () => state.project,
  project_list_repos: ({ query }) => {
    const q = query.trim().toLowerCase();
    return data.repos.filter(
      (r) => r.fullName.toLowerCase().includes(q) || r.description?.toLowerCase().includes(q),
    );
  },
  project_preview: async ({ repo }) => {
    await delay(300);
    return data.projectPreview(repo);
  },
  project_open: async ({ repo }) => {
    await delay(400);
    const preview = data.projectPreview(repo);
    state.project = {
      repo: preview.repo,
      currentVersion: preview.currentVersion,
      hasConfig: preview.hasConfig,
    };
    return state.project;
  },

  versions_list: () => data.versions,
  version_current: () => data.currentVersion,
  version_close: ({ input }) => ({
    closed: data.currentVersion.version.title,
    opened: input.next,
    movedIssues: input.carryOver === "move-to-next" ? 7 : 0,
    tag: `v${data.currentVersion.version.title}`,
    deployRun: "run-deploy-mock",
  }),

  issues_list: ({ filter }) =>
    data.issues.filter(
      (i) =>
        (!filter.version || i.version === filter.version) &&
        (!filter.status || filter.status.includes(i.status)) &&
        (!filter.issueType || i.issueType === filter.issueType) &&
        (!filter.query || i.title.toLowerCase().includes(filter.query.toLowerCase())),
    ),
  issue_get: ({ number }) => data.issueDetail(number),
  issue_create: ({ input }) => {
    if (!input.title?.trim()) fail("issue_create", "Une issue doit avoir un titre.");
    if (!input.version?.trim())
      fail("issue_create", "Une issue doit être rattachée à une version.");
    return {
      number: 160,
      title: input.title,
      version: input.version,
      issueType: input.issueType ?? null,
      status: "todo",
      labels: input.labels ?? [],
      assignee: data.me,
      estimate: null,
      activeAgent: input.sendToCto ? "cto" : null,
      linkedPr: null,
      flags: input.body ? [] : ["incomplete-description"],
      updatedAt: new Date().toISOString(),
    };
  },
  issue_answer_cto: () => null,

  prs_list: ({ filter }) =>
    data.pullRequests.filter((p) => !filter.state || filter.state.includes(p.state)),
  reviews_pending: () => data.reviews,

  agent_engines_get: () => state.engines,
  agent_engines_set: ({ engines }) => {
    state.engines = engines;
    return engines;
  },
  agents_overview: () => data.agentsOverview,
  agent_start: ({ kind }) => `run-${kind}-${Date.now()}`,
  agent_control: () => null,
  agent_actions: () => [],
  agent_apply: ({ ids }) => ({ applied: ids, failed: [] }),

  settings_get: () => data.config,
  settings_set: ({ config }) => config,
};

export async function handleMock<C extends CommandName>(
  command: C,
  args: CommandArgs<C>,
): Promise<CommandResult<C>> {
  await delay();
  const handler = handlers[command] as Handlers[C];
  return structuredClone(await handler(args));
}
