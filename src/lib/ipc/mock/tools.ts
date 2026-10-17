/**
 * Registre d'outils simulé (`tools_list`, `tool_install`, `tool_login`, `ai_test`) : la machine
 * des maquettes V2-Onboarding-Tools (Git et Node.js prêts, GitHub CLI manquant, Codex à
 * connecter, OpenCode absent). Installations et connexions rejouent un journal réaliste.
 *
 * `&tools=ready` : tout est prêt ; `&tools=bare` : seul Git est installé.
 */
import type {
  AiId,
  AppSettings,
  CliTest,
  InstalledPluginView,
  ToolId,
  ToolInfo,
  ToolRecipe,
  ToolRequester,
  ToolRunEvent,
  ToolRunKind,
  ToolRunResult,
  ToolsReport,
  ToolStatus,
} from "@/lib/types";
import { mockBus } from "./bus";

interface MockTool {
  path: string | null;
  version: string | null;
  account: string | null;
  connected: boolean;
}

const preset = new URLSearchParams(window.location.search).get("tools") ?? "default";

const installed = (path: string, version: string, account: string | null = null): MockTool => ({
  path,
  version,
  account,
  connected: true,
});
const missing: MockTool = { path: null, version: null, account: null, connected: false };

const machine: Record<ToolId, MockTool> = {
  git: installed("C:\\Program Files\\Git\\cmd\\git.exe", "2.47.1"),
  gh:
    preset === "ready"
      ? installed("C:\\Program Files\\GitHub CLI\\gh.exe", "2.81.0", "@rlhuillier")
      : missing,
  claude:
    preset === "bare"
      ? missing
      : installed(
          "C:\\Users\\moi\\.local\\bin\\claude.exe",
          "2.4.1",
          "Abonnement Max · moi@exemple.fr",
        ),
  codex:
    preset === "bare"
      ? missing
      : {
          path: "C:\\Users\\moi\\AppData\\Roaming\\npm\\codex.cmd",
          version: "0.48.0",
          account: preset === "ready" ? "Compte ChatGPT" : null,
          connected: preset === "ready",
        },
  opencode: missing,
  node: preset === "bare" ? missing : installed("C:\\Program Files\\nodejs\\node.exe", "22.11.0"),
};

const tests: Partial<Record<ToolId, CliTest>> = {};

const META: Record<
  ToolId,
  {
    name: string;
    description: string;
    min: string;
    homepage: string;
    login: string | null;
    interactive?: boolean;
    recipes: ToolRecipe[];
  }
> = {
  git: {
    name: "Git",
    description: "Cloner, préparer les worktrees",
    min: "2.30.0",
    homepage: "https://git-scm.com",
    login: null,
    recipes: [winget("Git.Git")],
  },
  gh: {
    name: "GitHub CLI",
    description: "Réutiliser votre connexion GitHub",
    min: "2.40.0",
    homepage: "https://cli.github.com",
    login: "gh auth login --web --git-protocol https",
    recipes: [winget("GitHub.cli")],
  },
  claude: {
    name: "Claude Code",
    description: "IA de code d'Anthropic",
    min: "2.0.0",
    homepage: "https://docs.claude.com/claude-code",
    login: "claude auth login",
    recipes: [
      {
        id: "script",
        label: "installeur officiel",
        command:
          'powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://claude.ai/install.ps1 | iex"',
        available: true,
        needs: null,
        elevated: false,
      },
      npm("@anthropic-ai/claude-code"),
    ],
  },
  codex: {
    name: "Codex CLI",
    description: "IA de code d'OpenAI",
    min: "0.40.0",
    homepage: "https://github.com/openai/codex",
    login: "codex login",
    recipes: [npm("@openai/codex")],
  },
  opencode: {
    name: "OpenCode",
    description: "IA de code ouverte, multi-fournisseurs",
    min: "0.15.0",
    homepage: "https://opencode.ai",
    login: "opencode auth login",
    interactive: true,
    recipes: [npm("opencode-ai")],
  },
  node: {
    name: "Node.js",
    description: "Prérequis des installations npm",
    min: "18.0.0",
    homepage: "https://nodejs.org",
    login: null,
    recipes: [winget("OpenJS.NodeJS.LTS")],
  },
};

function winget(id: string): ToolRecipe {
  return {
    id: "winget",
    label: "recette winget",
    command: `winget install --id ${id} -e --source winget --accept-package-agreements --accept-source-agreements`,
    available: true,
    needs: null,
    elevated: true,
  };
}

function npm(pkg: string): ToolRecipe {
  return {
    id: "npm",
    label: "paquet npm",
    command: `npm install -g ${pkg}`,
    available: machine.node?.path != null,
    needs: "node",
    elevated: false,
  };
}

const AI_TOOLS: [ToolId, AiId, keyof AppSettings["ai"]][] = [
  ["claude", "claude-code", "claudeCode"],
  ["codex", "codex", "codex"],
  ["opencode", "opencode", "opencode"],
];

function status(id: ToolId): ToolStatus {
  const tool = machine[id];
  if (!tool.path) return "missing";
  if (META[id].login && !tool.connected) return "not-connected";
  return "ready";
}

/** Même règle que `tools::requesters` côté Rust. */
function requesters(
  settings: AppSettings,
  plugins: InstalledPluginView[],
): Partial<Record<ToolId, ToolRequester[]>> {
  const map: Partial<Record<ToolId, ToolRequester[]>> = {};
  const add = (tool: ToolId, r: ToolRequester) => (map[tool] ??= []).push(r);
  if (plugins.some((p) => p.enabled && p.services.includes("repos"))) {
    const repos = (optional: boolean): ToolRequester => ({
      kind: "service",
      id: "repos",
      name: "Dépôts GitHub",
      optional,
    });
    add("git", repos(false));
    add("gh", repos(true));
  }
  for (const [tool, ai, key] of AI_TOOLS) {
    const isDefault = settings.defaultAi === ai;
    if (isDefault || (machine[tool].path && settings.ai[key].enabled))
      add(tool, { kind: "ai", id: "ai", name: "IA de code", optional: !isDefault });
  }
  for (const tool of ["codex", "opencode"] as ToolId[]) {
    if (map[tool] && !machine[tool].path && !machine.node.path)
      add("node", {
        kind: "tool",
        id: tool,
        name: META[tool].name,
        optional: map[tool]!.every((r) => r.optional),
      });
  }
  return map;
}

export function mockToolsReport(
  settings: AppSettings,
  plugins: InstalledPluginView[],
): ToolsReport {
  const requested = requesters(settings, plugins);
  const tools = (Object.keys(META) as ToolId[]).map((id): ToolInfo => {
    const meta = META[id];
    const tool = machine[id];
    const ai = AI_TOOLS.find(([t]) => t === id);
    const requestedBy = requested[id] ?? [];
    return {
      id,
      name: meta.name,
      description: meta.description,
      status: status(id),
      path: tool.path,
      version: tool.version,
      minVersion: meta.min,
      account: tool.connected ? tool.account : null,
      test: tests[id] ?? null,
      login: meta.login,
      loginInteractive: meta.interactive ?? false,
      homepage: meta.homepage,
      recipes: meta.recipes.map((r) =>
        r.needs === "node" ? { ...r, available: machine.node.path != null } : r,
      ),
      requestedBy,
      required: requestedBy.some((r) => !r.optional),
      ignored: ai ? !settings.ai[ai[2]].enabled : settings.ignoredTools.includes(id),
      message: null,
    };
  });
  return { platform: "Windows 11", managers: ["winget 1.9.25200", "npm 10.9.0"], tools };
}

const running = new Map<ToolId, () => void>();
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Rejoue un journal ligne à ligne ; `null` en fin si la commande a été annulée. */
async function replay(
  tool: ToolId,
  kind: ToolRunKind,
  lines: (string | { text: string; replace: true })[],
  step = 260,
): Promise<boolean> {
  let cancelled = false;
  running.set(tool, () => (cancelled = true));
  for (const line of lines) {
    await wait(step);
    if (cancelled) break;
    const [text, replace] = typeof line === "string" ? [line, false] : [line.text, true];
    mockBus.emit<ToolRunEvent>("tool://install", { tool, kind, line: text, replace, done: null });
  }
  running.delete(tool);
  return !cancelled;
}

function done(tool: ToolId, kind: ToolRunKind, result: ToolRunResult): ToolRunResult {
  mockBus.emit<ToolRunEvent>("tool://install", {
    tool,
    kind,
    line: null,
    replace: false,
    done: result,
  });
  return result;
}

const INSTALLED_VERSION: Record<ToolId, string> = {
  git: "2.47.1",
  gh: "2.81.0",
  claude: "2.4.1",
  codex: "0.48.0",
  opencode: "0.15.8",
  node: "22.11.0",
};

export async function mockInstallTool(
  tool: ToolId,
  recipeId: string | null,
): Promise<ToolRunResult> {
  if (running.has(tool)) throw new Error(`${META[tool].name} : une commande est déjà en cours.`);
  const recipe = META[tool].recipes.find((r) => (recipeId ? r.id === recipeId : true));
  if (!recipe) throw new Error(`Recette inconnue pour ${META[tool].name}.`);
  if (recipe.needs === "node" && !machine.node.path)
    throw new Error("Installez d'abord Node.js : cette recette passe par npm.");
  const started = Date.now();
  const version = INSTALLED_VERSION[tool];
  const bar = (n: number) => ({
    text: `  ${"█".repeat(n)}${"░".repeat(26 - n)}  ${((13.8 * n) / 26).toFixed(1).replace(".", ",")} Mo / 13,8 Mo`,
    replace: true as const,
  });
  const lines =
    recipe.id === "winget"
      ? [
          `Trouvé ${META[tool].name} [${recipe.command.split(" ")[3]}] Version ${version}`,
          "Ce package vous est concédé sous licence par son propriétaire.",
          "Téléchargement en cours",
          { text: bar(4).text, replace: true as const },
          bar(12),
          bar(19),
          bar(26),
          "Le hachage de l'installation a été vérifié avec succès",
          "Démarrage du package d'installation... Merci de patienter.",
          "Installé correctement",
        ]
      : [`added 1 package in 4s`, `${tool} ${version}`];
  const finished = await replay(tool, "install", lines);
  if (!finished)
    return done(tool, "install", {
      ok: false,
      exitCode: null,
      message: "annulé",
      durationMs: Date.now() - started,
    });
  machine[tool] = {
    path:
      recipe.id === "npm"
        ? `C:\\Users\\moi\\AppData\\Roaming\\npm\\${tool}.cmd`
        : `C:\\Program Files\\${META[tool].name}\\${tool}.exe`,
    version,
    account: null,
    connected: !META[tool].login,
  };
  return done(tool, "install", {
    ok: true,
    exitCode: 0,
    message: null,
    durationMs: Date.now() - started,
  });
}

const ACCOUNTS: Partial<Record<ToolId, string>> = {
  gh: "@rlhuillier",
  claude: "Abonnement Max · moi@exemple.fr",
  codex: "Compte ChatGPT",
};

export async function mockLoginTool(tool: ToolId): Promise<ToolRunResult> {
  const meta = META[tool];
  if (!meta.login) throw new Error(`${meta.name} n'a pas de compte à connecter.`);
  if (!machine[tool].path) throw new Error(`${meta.name} n'est pas installé.`);
  if (meta.interactive)
    return {
      ok: true,
      exitCode: null,
      message: "Terminal ouvert : terminez la connexion, puis revérifiez.",
      durationMs: 0,
    };
  if (running.has(tool)) throw new Error(`${meta.name} : une commande est déjà en cours.`);
  const started = Date.now();
  const lines =
    tool === "gh"
      ? [
          "! First copy your one-time code: A1B2-C3D4",
          "Press Enter to open https://github.com/login/device in your browser...",
          "✓ Authentication complete.",
          "- gh config set -h github.com git_protocol https",
          "✓ Logged in as rlhuillier",
        ]
      : ["Opening browser for sign-in…", "Successfully logged in"];
  const finished = await replay(tool, "login", lines, tool === "gh" ? 1400 : 700);
  if (!finished)
    return done(tool, "login", {
      ok: false,
      exitCode: null,
      message: "annulé",
      durationMs: Date.now() - started,
    });
  Object.assign(machine[tool], { connected: true, account: ACCOUNTS[tool] ?? null });
  return done(tool, "login", {
    ok: true,
    exitCode: 0,
    message: null,
    durationMs: Date.now() - started,
  });
}

export function mockCancelTool(tool: ToolId): boolean {
  const cancel = running.get(tool);
  cancel?.();
  return cancel !== undefined;
}

export function mockSetToolPath(tool: ToolId, path: string | null) {
  if (path === null) return;
  if (!/\.(exe|cmd)$|\/[a-z]+$/i.test(path)) throw new Error(`${path} n'est pas un fichier.`);
  machine[tool] = {
    path,
    version: INSTALLED_VERSION[tool],
    account: ACCOUNTS[tool] ?? null,
    connected: true,
  };
}

export async function mockTestAi(tool: ToolId, outcome: string): Promise<CliTest> {
  if (!machine[tool].path) throw new Error("Binaire introuvable : impossible de le tester.");
  await wait(1100);
  const test: CliTest =
    outcome === "fail" || !machine[tool].connected
      ? { ok: false, latencyMs: 1840, message: "Invalid API key · Please run /login" }
      : { ok: true, latencyMs: tool === "claude" ? 1200 : 2300, message: null };
  tests[tool] = test;
  return test;
}
