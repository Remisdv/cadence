/**
 * Implémentation mock des commandes. Chaque commande du contrat DOIT avoir un handler :
 * le type `Handlers` le vérifie à la compilation.
 *
 * Scénario de démarrage via l'URL :
 * - `?scenario=app` (défaut) : connecté, projet ouvert, plugins factices installés ;
 * - `?scenario=onboarding` : onboarding à faire, compte GitHub non connecté ;
 * - `?scenario=no-project` : connecté, aucun projet choisi ;
 * - `?scenario=empty` : aucun plugin installé ;
 * - `?scenario=plugin-crash` : le serveur de Gestion de projet est en échec.
 *
 * Issue de la connexion GitHub simulée (navigateur ou code) : `&flow=ok` (défaut, autorisée
 * après quelques secondes), `denied`, `expired`, `config` (client ID / secret absent). Session de test de Claude Code : `&cli=ok` (défaut) ou `fail`.
 */
import type {
  AuthChanged,
  CliAgentInfo,
  InstalledPluginView,
  IssueRow,
  PluginEventPayload,
  PluginInstallEvent,
  PluginProcessView,
  PluginStateEvent,
  ProjectState,
} from "@/lib/types";
import type { CommandArgs, CommandName, CommandResult } from "../commands";
import { CommandError } from "../errors";
import { mockBus } from "./bus";
import * as data from "./fixtures";
import { versionReport } from "./versions";
import { mockCatalogEntries, mockCatalogView } from "./catalog";
import {
  answerApproval,
  mockChats,
  newChat,
  recordMockChatEvent,
  requestInterrupt,
  simulateTurn,
} from "./chat";
import {
  mockAppSettings,
  mockInstalledPlugin,
  mockPlugins,
  mockProcesses,
  mockSettingsSchema,
  validateMockSettings,
  withMockDefaults,
} from "./plugins";
import {
  mockCancelTool,
  mockInstallTool,
  mockLoginTool,
  mockSetToolPath,
  mockTestAi,
  mockToolsReport,
} from "./tools";

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
  project: scenario === "app" ? data.project : (null as ProjectState | null),
  settings:
    scenario === "onboarding"
      ? { ...structuredClone(mockAppSettings), onboarded: false, defaultAi: null, trackedRepos: {} }
      : structuredClone(mockAppSettings),
  plugins: scenario === "empty" || scenario === "onboarding" ? [] : structuredClone(mockPlugins),
  pluginSettings: {} as Record<string, Record<string, unknown>>,
  secrets: new Set<string>(),
  processes: mockProcesses(scenario === "plugin-crash"),
  chats: mockChats(scenario !== "app"),
  counter: 0,
  catalogFetchedAt: new Date(Date.now() - 2 * 3600_000).toISOString(),
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

function installed(command: string, id: string): InstalledPluginView {
  const plugin = state.plugins.find((p) => p.id === id);
  if (!plugin) fail(command, `Le plugin « ${id} » n'est pas installé.`);
  return plugin;
}

function declaredSecret(command: string, plugin: string, secret: string): string {
  const found = installed(command, plugin).secrets.some((s) => s.id === secret);
  if (!found) fail(command, `Le plugin « ${plugin} » ne déclare pas le secret « ${secret} ».`);
  return `plugin/${plugin}/${secret}`;
}

/** Statut recalculé après un changement de secret (`needs-setup` tant qu'un requis manque). */
function refreshStatus(plugin: InstalledPluginView) {
  if (plugin.status === "invalid") return;
  const missing = plugin.secrets.some(
    (s) => s.required && !state.secrets.has(`plugin/${plugin.id}/${s.id}`),
  );
  plugin.status = missing ? "needs-setup" : "ok";
}

function emitState(process: PluginProcessView) {
  mockBus.emit<PluginStateEvent>("plugin://state", {
    plugin: process.plugin,
    state: process.state,
    message: process.message,
  });
}

/** Le serveur d'un plugin s'arrête (désactivation, secret ou réglage changé, désinstallation). */
function stopProcess(plugin: string) {
  const process = state.processes.find((p) => p.plugin === plugin);
  if (!process || process.state === "stopped") return;
  Object.assign(process, { state: "stopped", pid: null, tools: 0, message: null });
  emitState(process);
}

/** Dernier segment d'un chemin Windows ou Unix. */
const baseName = (path: string) =>
  path
    .replace(/[\\/]+$/, "")
    .split(/[\\/]/)
    .pop() ?? "";

function addPlugin(command: string, plugin: InstalledPluginView) {
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(plugin.id))
    fail(command, `« ${plugin.id} » n'est pas un identifiant de plugin.`);
  const previous = state.plugins.find((p) => p.id === plugin.id);
  if (previous) return Object.assign(previous, { ...plugin, enabled: previous.enabled });
  state.plugins.push(plugin);
  return plugin;
}

/** Téléchargement et installation simulés d'un plugin du catalogue. */
async function installFromMockCatalog(command: string, id: string) {
  const entry = mockCatalogEntries.find((e) => e.id === id);
  if (!entry) fail(command, `« ${id} » n'est pas au catalogue.`);
  if (entry.minCadence && entry.minCadence > "0.3.0")
    fail(command, `Nécessite Cadence ${entry.minCadence} ou plus récent.`);
  const total = entry.size ?? 4_000_000;
  for (let i = 1; i <= 8; i++) {
    mockBus.emit<PluginInstallEvent>("plugin://install", {
      id,
      received: Math.round((total * i) / 8),
      total,
      phase: "downloading",
    });
    await delay(150);
  }
  mockBus.emit<PluginInstallEvent>("plugin://install", {
    id,
    received: total,
    total,
    phase: "extracting",
  });
  await delay(300);
  stopProcess(id);
  const existing = state.plugins.find((p) => p.id === id);
  const plugin = existing
    ? Object.assign(existing, { version: entry.version, source: "catalog" as const })
    : addPlugin(command, {
        ...mockInstalledPlugin(id, entry.version, "catalog"),
        name: entry.name,
        description: entry.description,
        category: entry.category,
        color: entry.color,
        railIcon: entry.railIcon,
        services: entry.services,
        actions: entry.actions.map((a) => ({
          ...a,
          skill: a.id,
          context: null,
          description: null,
          icon: null,
          confirm: null,
        })),
        views: entry.views.map((v, i) => ({ ...v, section: null, hidden: false, home: i === 0 })),
      });
  mockBus.emit<PluginInstallEvent>("plugin://install", {
    id,
    received: 0,
    total: 0,
    phase: "done",
  });
  return plugin;
}

const handlers: Handlers = {
  app_status: () => ({
    onboarded: state.settings.onboarded,
    version: "0.3.0-dev",
    gateway: "http://127.0.0.1:52417/mcp",
    dirs: {
      plugins: String.raw`C:\Users\moi\AppData\Roaming\fr.cadence.app\plugins`,
      pluginsData: String.raw`C:\Users\moi\AppData\Roaming\fr.cadence.app\plugins-data`,
      sessions: String.raw`C:\Users\moi\AppData\Roaming\fr.cadence.app\sessions`,
      repos: String.raw`C:\Users\moi\AppData\Roaming\fr.cadence.app\repos`,
    },
  }),
  app_settings_get: () => state.settings,
  app_settings_set: ({ patch }) => {
    const next = structuredClone(state.settings);
    if (patch.onboarded !== undefined) next.onboarded = patch.onboarded;
    if (patch.defaultAi !== undefined) next.defaultAi = patch.defaultAi;
    for (const change of patch.ai ?? []) {
      const key = change.ai === "claude-code" ? "claudeCode" : change.ai;
      if (change.path !== undefined) next.ai[key].path = change.path.trim() || null;
      if (change.enabled !== undefined) next.ai[key].enabled = change.enabled;
    }
    for (const [action, ai] of Object.entries(patch.actionAi ?? {})) {
      if (!/^[a-z0-9_-]+\/[a-z0-9_-]+$/.test(action))
        fail("app_settings_set", `« ${action} » n'est pas une action <plugin>/<action>.`);
      if (ai) next.actionAi[action] = ai;
      else delete next.actionAi[action];
    }
    if (patch.catalogUrl !== undefined) {
      const url = patch.catalogUrl.trim();
      if (url && !url.startsWith("https://"))
        fail("app_settings_set", "L'URL du catalogue doit commencer par https://.");
      next.catalogUrl = url || null;
    }
    if (patch.devPlugins !== undefined)
      next.devPlugins = [...new Set(patch.devPlugins.map((d) => d.trim()).filter(Boolean))];
    for (const [plugin, repos] of Object.entries(patch.trackedRepos ?? {})) {
      const list = [...new Set(repos.map((r) => r.trim().replace(/\.git$/, "")))];
      const bad = list.find((r) => !/^[\w.-]+\/[\w.-]+$/.test(r));
      if (bad) fail("app_settings_set", `« ${bad} » n'est pas un dépôt owner/nom.`);
      if (list.length) next.trackedRepos[plugin] = list;
      else delete next.trackedRepos[plugin];
    }
    if (patch.ignoredTools !== undefined)
      next.ignoredTools = [...new Set(patch.ignoredTools)].sort();
    state.settings = next;
    return next;
  },

  tools_list: async ({ refresh }) => {
    if (refresh) await delay(900);
    return mockToolsReport(state.settings, state.plugins);
  },
  tool_install: async ({ tool, recipe }) => {
    try {
      return await mockInstallTool(tool, recipe);
    } catch (e) {
      fail("tool_install", (e as Error).message);
    }
  },
  tool_login: async ({ tool }) => {
    try {
      return await mockLoginTool(tool);
    } catch (e) {
      fail("tool_login", (e as Error).message);
    }
  },
  tool_cancel: ({ tool }) => mockCancelTool(tool),
  tool_set_path: ({ tool, path }) => {
    try {
      mockSetToolPath(tool, path?.trim() || null);
    } catch (e) {
      fail("tool_set_path", (e as Error).message);
    }
    const ai = { claude: "claudeCode", codex: "codex", opencode: "opencode" } as const;
    if (tool in ai) {
      const slot = state.settings.ai[ai[tool as keyof typeof ai]];
      slot.path = path?.trim() || null;
      if (slot.path) slot.enabled = true;
    } else if (path?.trim()) state.settings.toolPaths[tool] = path.trim();
    else delete state.settings.toolPaths[tool];
    return mockToolsReport(state.settings, state.plugins);
  },
  ai_test: async ({ ai }) => {
    const tool = ai === "claude-code" ? "claude" : ai;
    try {
      return await mockTestAi(tool, cliOutcome);
    } catch (e) {
      fail("ai_test", (e as Error).message);
    }
  },

  plugins_installed: () => state.plugins,
  plugins_set_enabled: ({ id, enabled }) => {
    const plugin = installed("plugins_set_enabled", id);
    plugin.enabled = enabled;
    return plugin;
  },
  plugins_reorder: ({ order }) => {
    const ids = state.plugins.map((p) => p.id).sort();
    if (new Set(order).size !== order.length || [...order].sort().join() !== ids.join())
      fail(
        "plugins_reorder",
        "L'ordre du rail doit citer chaque plugin installé exactement une fois.",
      );
    const categories = ["projet", "etudes", "code", "donnees", "outils"];
    state.plugins = order
      .map((id) => state.plugins.find((p) => p.id === id)!)
      .sort((a, b) => categories.indexOf(a.category) - categories.indexOf(b.category));
    return state.plugins;
  },
  plugin_settings_get: ({ plugin }) => {
    installed("plugin_settings_get", plugin);
    const schema = mockSettingsSchema[plugin] ?? null;
    return {
      plugin,
      schema,
      value: withMockDefaults(schema, state.pluginSettings[plugin] ?? {}),
      errors: [],
    };
  },
  plugin_settings_set: ({ plugin, value }) => {
    installed("plugin_settings_set", plugin);
    const schema = mockSettingsSchema[plugin] ?? null;
    const errors = validateMockSettings(schema, value);
    if (errors.length === 0) state.pluginSettings[plugin] = value;
    return { plugin, schema, value: withMockDefaults(schema, value), errors };
  },
  plugin_secret_set: ({ plugin, secret, value }) => {
    const account = declaredSecret("plugin_secret_set", plugin, secret);
    if (!value.trim()) fail("plugin_secret_set", "Le secret est vide.");
    state.secrets.add(account);
    refreshStatus(installed("plugin_secret_set", plugin));
    return null;
  },
  plugin_secret_has: ({ plugin, secret }) =>
    state.secrets.has(declaredSecret("plugin_secret_has", plugin, secret)),
  plugin_secret_delete: ({ plugin, secret }) => {
    state.secrets.delete(`plugin/${plugin}/${secret}`);
    const found = state.plugins.find((p) => p.id === plugin);
    if (found) refreshStatus(found);
    stopProcess(plugin);
    return null;
  },
  plugin_install_zip: async ({ path }) => {
    if (!path.toLowerCase().endsWith(".zip")) fail("plugin_install_zip", "Archive zip invalide.");
    // `journal-0.2.0.zip` → plugin `journal`, version 0.2.0.
    const [, id = "", version = "0.1.0"] =
      /^(.*?)(?:-(\d+\.\d+\.\d+))?\.zip$/i.exec(baseName(path)) ?? [];
    const total = 4_200_000;
    for (const [phase, received] of [
      ["verifying", total],
      ["extracting", total / 2],
      ["extracting", total],
    ] as const) {
      mockBus.emit<PluginInstallEvent>("plugin://install", { id, received, total, phase });
      await delay(200);
    }
    stopProcess(id);
    const plugin = addPlugin("plugin_install_zip", mockInstalledPlugin(id, version, "file"));
    mockBus.emit<PluginInstallEvent>("plugin://install", {
      id,
      received: 0,
      total: 0,
      phase: "done",
    });
    return plugin;
  },
  plugin_install_dev: ({ path }) => {
    const plugin = addPlugin(
      "plugin_install_dev",
      mockInstalledPlugin(baseName(path), "0.1.0", "dev"),
    );
    if (!state.settings.devPlugins.includes(path)) state.settings.devPlugins.push(path);
    stopProcess(plugin.id);
    return plugin;
  },
  plugin_uninstall: ({ id, deleteSecrets }) => {
    const plugin = installed("plugin_uninstall", id);
    stopProcess(id);
    state.plugins = state.plugins.filter((p) => p.id !== id);
    state.processes = state.processes.filter((p) => p.plugin !== id);
    if (deleteSecrets) plugin.secrets.forEach((s) => state.secrets.delete(`plugin/${id}/${s.id}`));
    if (plugin.source === "dev")
      state.settings.devPlugins = state.settings.devPlugins.filter((d) => baseName(d) !== id);
    return null;
  },
  chat_sessions: () =>
    [...state.chats.values()]
      .map((c) => c.session)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
  chat_session_get: ({ id }) => {
    const chat = state.chats.get(id);
    if (!chat) fail("chat_session_get", `Conversation « ${id} » introuvable.`);
    return structuredClone(chat);
  },
  chat_send: ({ input }) => {
    const text = input.text.trim();
    if (!text) fail("chat_send", "Le message est vide.");
    const ai = input.ai ?? state.settings.defaultAi ?? "claude-code";
    if (!input.session && ai !== "claude-code")
      fail("chat_send", "Seul Claude Code est pris en charge pour l'instant dans le chat.");
    const id = input.session ?? newChat(state.chats, text).id;
    const chat = state.chats.get(id);
    if (!chat) fail("chat_send", `Conversation « ${id} » introuvable.`);
    if (chat.session.state === "running" || chat.session.state === "waiting-approval")
      fail("chat_send", "Un tour est déjà en cours dans cette conversation.");
    recordMockChatEvent(state.chats, id, { type: "user-message", text });
    void simulateTurn(state.chats, id, text);
    return { session: id };
  },
  chat_interrupt: ({ session }) => {
    const chat = state.chats.get(session);
    if (chat?.session.state !== "running")
      fail("chat_interrupt", "Aucun tour en cours dans cette conversation.");
    requestInterrupt(session);
    return null;
  },
  chat_approve: ({ request, decision }) => {
    if (!answerApproval(request, decision))
      fail("chat_approve", "Cette demande d'approbation n'attend plus de réponse.");
    return null;
  },
  chat_session_rename: ({ id, title }) => {
    const chat = state.chats.get(id);
    if (!chat) fail("chat_session_rename", `Conversation « ${id} » introuvable.`);
    if (!title.trim()) fail("chat_session_rename", "Le titre est vide.");
    chat.session = { ...chat.session, title: title.trim() };
    return chat.session;
  },
  chat_session_delete: ({ id }) => {
    if (!state.chats.delete(id)) fail("chat_session_delete", `Conversation « ${id} » introuvable.`);
    return null;
  },
  catalog_get: ({ refresh }) => {
    const offline = scenario === "offline";
    if (refresh && !offline) state.catalogFetchedAt = new Date().toISOString();
    const view = mockCatalogView(state.plugins, offline, state.catalogFetchedAt);
    mockBus.emit("catalog://updated", { updates: view.updates });
    return view;
  },
  plugin_install: ({ id }) => installFromMockCatalog("plugin_install", id),
  plugin_update: ({ id }) => installFromMockCatalog("plugin_update", id),
  plugin_call: ({ plugin, tool, args }) => {
    if (!installed("plugin_call", plugin).enabled)
      fail("plugin_call", `Le plugin « ${plugin} » est désactivé.`);
    if (plugin !== "exemple")
      fail("plugin_call", `Le plugin « ${plugin} » n'a pas de serveur en mode mock.`);
    const result = (structured: unknown) => ({
      isError: false,
      structured,
      text: JSON.stringify(structured),
    });
    switch (tool) {
      case "compteur": {
        state.counter += Number(args?.pas ?? 1);
        // Le serveur prévient les iframes du plugin (notifications/cadence/event).
        setTimeout(() =>
          mockBus.emit<PluginEventPayload>("plugin://event", {
            plugin,
            event: "compteur.change",
            data: { valeur: state.counter },
          }),
        );
        return result({ valeur: state.counter });
      }
      case "heure":
        return result({ iso: new Date().toISOString() });
      case "echo":
        return result({ echo: args ?? {} });
      case "erreur":
        return { isError: true, structured: null, text: "L'outil a échoué (exprès)." };
      default:
        fail("plugin_call", `exemple__${tool} : outil inconnu.`);
    }
  },
  plugin_status: () =>
    state.processes.filter((p) => state.plugins.some((plugin) => plugin.id === p.plugin)),
  plugin_restart: async ({ plugin }) => {
    if (!installed("plugin_restart", plugin).enabled)
      fail("plugin_restart", `Le plugin « ${plugin} » est désactivé.`);
    let process = state.processes.find((p) => p.plugin === plugin);
    if (!process) {
      process = {
        plugin,
        state: "stopped",
        pid: null,
        restarts: 0,
        tools: 0,
        message: null,
        logPath: null,
      };
      state.processes.push(process);
    }
    Object.assign(process, { state: "starting", message: null, pid: null });
    emitState(process);
    await delay(600);
    Object.assign(process, {
      state: "ready",
      pid: 4000 + Math.floor(Math.random() * 1000),
      tools: 12,
    });
    emitState(process);
    return null;
  },

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
    if (!report)
      fail("version_detail", `La version ${title ?? ""} n'est pas un milestone du dépôt.`);
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
  issue_get: ({ number }) => data.issueDetail(number),
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
      linkedPr: null,
      flags: input.body ? [] : ["incomplete-description"],
      updatedAt: new Date().toISOString(),
    };
    data.issues.unshift(row);
    return row;
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
};

export async function handleMock<C extends CommandName>(
  command: C,
  args: CommandArgs<C>,
): Promise<CommandResult<C>> {
  await delay();
  const handler = handlers[command] as Handlers[C];
  return structuredClone(await handler(args));
}
