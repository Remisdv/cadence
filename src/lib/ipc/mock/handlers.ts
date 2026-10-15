/**
 * Implémentation mock des commandes. Chaque commande du contrat DOIT avoir un handler :
 * le type `Handlers` le vérifie à la compilation.
 *
 * Scénario de démarrage via l'URL :
 * - `?scenario=app` (défaut) : connecté, projet ouvert ;
 * - `?scenario=onboarding` : compte GitHub non connecté ;
 * - `?scenario=no-project` : connecté, aucun projet choisi.
 *
 * Issue de la connexion GitHub simulée (navigateur ou code) : `&flow=ok` (défaut, autorisée
 * après quelques secondes), `denied`, `expired`, `config` (client ID / secret absent). Session de test de Claude Code : `&cli=ok` (défaut) ou `fail`.
 */
import type { AuthChanged, CliAgentInfo, IssueRow, ProjectState } from "@/lib/types";
import type { CommandArgs, CommandName, CommandResult } from "../commands";
import { CommandError } from "../errors";
import { mockBus } from "./bus";
import * as data from "./fixtures";
import { ctoMock } from "./cto";
import { devMock } from "./dev";
import { agentsMock } from "./agents";
import { versionReport } from "./versions";

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
  auth_start_web_flow: () => {
    if (flow === "config") {
      fail(
        "auth_start_web_flow",
        "Client secret GitHub absent : définir CADENCE_GITHUB_CLIENT_SECRET pour la connexion par navigateur. Le code d'appairage et le jeton personnel restent disponibles.",
        "config",
      );
    }
    // Sans OAuth App propre (cas par défaut), c'est Git Credential Manager qui pilote le navigateur.
    const authorizeUrl = data.githubSources.oauthApp
      ? "https://github.com/login/oauth/authorize?client_id=mock"
      : null;
    window.clearTimeout(flowTimer);
    flowTimer = window.setTimeout(() => {
      if (flow === "denied") return emitAuth({ status: "error", reason: "denied", message: null });
      if (flow === "expired")
        return emitAuth({ status: "error", reason: "expired", message: null });
      state.github = data.githubConnected;
      emitAuth({ status: "connected", reason: null, message: null });
    }, 4000);
    return { authorizeUrl };
  },
  auth_use_gh_cli: async () => {
    await delay(500);
    window.clearTimeout(flowTimer);
    state.github = data.githubConnected;
    emitAuth({ status: "connected", reason: null, message: null });
    return state.github;
  },
  auth_status: () => ({
    github: state.github,
    githubSources: data.githubSources,
    claudeCode: state.claudeCode,
    codex: state.codex,
  }),
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
  project_owners: () => data.repoOwners,
  project_create: async ({ input }) => {
    await delay(700);
    const name = input.name.trim();
    if (!/^[A-Za-z0-9._-]{1,100}$/.test(name) || name === "." || name === "..") {
      fail(
        "project_create",
        "Nom de dépôt invalide : lettres, chiffres, « - », « _ » et « . » uniquement.",
      );
    }
    const fullName = `${input.owner}/${name}`;
    if (data.repos.some((r) => r.fullName.toLowerCase() === fullName.toLowerCase())) {
      fail("project_create", "Un dépôt porte déjà ce nom sur ce compte.");
    }
    data.repos.unshift({
      fullName,
      owner: input.owner,
      name,
      description: input.description?.trim() || null,
      private: input.private ?? false,
      defaultBranch: "main",
      openIssues: 0,
      milestones: 0,
      pushedAt: new Date().toISOString(),
    });
    state.project = data.projectFor(fullName);
    return state.project;
  },
  project_open: async ({ repo }) => {
    await delay(400);
    state.project = data.projectFor(repo);
    return state.project;
  },

  versions_list: () => data.versions,
  version_current: () => data.currentVersion,
  version_detail: ({ title }) => {
    const report = versionReport(title ?? state.project?.currentVersion ?? null);
    if (!report) fail("version_detail", `La version ${title ?? ""} n'est pas un milestone du dépôt.`);
    return report;
  },
  version_close: ({ input }) => {
    const closed = data.project.currentVersion ?? "0.1.2";
    if (data.versions.some((v) => v.title === input.next && v.state === "closed"))
      fail("version_close", `La version ${input.next} est déjà close sur GitHub.`);
    const open = data.issues.filter((i) => i.version === closed && i.status !== "done").length;
    return {
      closed,
      opened: input.next,
      movedIssues: open,
      tag: `v${closed}`,
      releaseUrl: input.generateNotes
        ? `https://github.com/cadence-app/desktop/releases/tag/untagged-${closed}`
        : null,
      deployRun: null,
    };
  },

  issues_list: ({ filter }) =>
    data.issues.filter(
      (i) =>
        (!filter.version || i.version === filter.version) &&
        (!filter.status || filter.status.includes(i.status)) &&
        (!filter.issueType || i.issueType === filter.issueType) &&
        (!filter.query || i.title.toLowerCase().includes(filter.query.toLowerCase())),
    ),
  issue_get: ({ number }) => {
    const detail = data.issueDetail(number);
    const item = ctoMock.item(number);
    const questions = item?.state === "questions" ? structuredClone(item.questions) : [];
    return {
      ...detail,
      body: ctoMock.appliedBody(number) ?? detail.body,
      questions,
      activeAgent: item?.state === "running" ? "cto" : null,
      flags: [
        ...detail.flags.filter((f) => f !== "cto-questions"),
        ...(questions.length > 0 ? (["cto-questions"] as const) : []),
      ],
    };
  },
  issue_create: ({ input }) => {
    if (!input.title?.trim()) fail("issue_create", "Une issue doit avoir un titre.");
    if (!input.version?.trim())
      fail("issue_create", "Une issue doit être rattachée à une version.");
    if (!data.versions.some((v) => v.title === input.version && v.state === "open"))
      fail("issue_create", `La version ${input.version} n'est pas un milestone ouvert du dépôt.`);
    const row: IssueRow = {
      number: Math.max(...data.issues.map((i) => i.number)) + 1,
      title: input.title,
      version: input.version,
      issueType: input.issueType ?? null,
      status: "todo",
      labels: input.labels ?? [],
      assignee: input.assignee ? data.me : null,
      estimate: null,
      activeAgent: input.sendToCto ? "cto" : null,
      linkedPr: null,
      flags: input.body ? [] : ["incomplete-description"],
      updatedAt: new Date().toISOString(),
    };
    data.issues.unshift(row);
    if (input.sendToCto) {
      ctoMock.start([row.number]);
      row.activeAgent = "cto";
    }
    return row;
  },
  issue_answer_cto: ({ number, answers }) => {
    try {
      ctoMock.answer(number, answers);
    } catch (e) {
      fail("issue_answer_cto", (e as Error).message);
    }
    return null;
  },

  prs_list: ({ filter }) =>
    data.pullRequests.filter(
      (p) =>
        (!filter.state || filter.state.includes(p.state)) &&
        (!filter.author || p.author.login === filter.author) &&
        (!filter.version || p.version === filter.version),
    ),
  reviews_pending: () => data.reviews,
  pr_detail: ({ number }) => {
    const detail = data.prDetailFor(number);
    if (!detail) fail("pr_detail", `PR #${number} absente du cache.`);
    return detail;
  },

  agent_engines_get: () => state.engines,
  agent_engines_set: ({ engines }) => {
    state.engines = engines;
    return engines;
  },
  agents_overview: () => agentsMock.overview(),
  agent_start: ({ kind, input }) => {
    if (kind === "dev") {
      if (!input.issues?.length) fail("agent_start", "Choisissez au moins une tâche.");
      try {
        return devMock.start(input.issues);
      } catch (e) {
        fail("agent_start", (e as Error).message);
      }
    }
    if (kind !== "cto") fail("agent_start", "Cet agent arrive plus tard.");
    return ctoMock.start(input.issues?.length ? input.issues : undefined);
  },
  agent_control: ({ run, cmd }) => {
    const dev = run.startsWith("dev-");
    if (cmd.cmd === "stop") (dev ? devMock : ctoMock).stop();
    else if (!dev && cmd.cmd !== "message") ctoMock.pause(cmd.cmd === "pause");
    else if (!dev) fail("agent_control", "L'agent CTO ne prend pas d'intervention en cours de route.");
    else if (cmd.cmd === "pause") devMock.pause(true);
    else if (cmd.cmd === "resume") devMock.pause(false);
    else devMock.message(cmd.text);
    return null;
  },
  agent_actions: () => [],
  agent_apply: ({ run, ids }) => (run.startsWith("dev-") ? devMock.push(ids) : ctoMock.apply(ids)),
  agent_latest_run: ({ kind }) =>
    structuredClone(kind === "cto" ? ctoMock.latest() : kind === "dev" ? devMock.latest() : null),
  agent_run_get: ({ run }) => {
    const found = run.startsWith("dev-") ? devMock.latest() : ctoMock.latest();
    if (!found) fail("agent_run_get", "Exécution introuvable.");
    return structuredClone(found);
  },
  agent_reject: ({ run, numbers }) => {
    (run.startsWith("dev-") ? devMock.skip : ctoMock.reject)(numbers);
    return null;
  },
  agent_edit_body: ({ number, body }) => {
    ctoMock.edit(number, body);
    return null;
  },
  agent_runs_list: ({ limit }) => agentsMock.runs(limit),

  settings_get: () => data.config,
  settings_set: ({ config }) => {
    if (!(config.versioning.sprintDays >= 1 && config.versioning.sprintDays <= 60))
      fail("settings_set", "Un sprint dure entre 1 et 60 jours.");
    Object.assign(data.config, config);
    return config;
  },
};

export async function handleMock<C extends CommandName>(
  command: C,
  args: CommandArgs<C>,
): Promise<CommandResult<C>> {
  await delay();
  const handler = handlers[command] as Handlers[C];
  return structuredClone(await handler(args));
}
