/**
 * Variante du protocole parlée par le SDK publié `@cadence/plugin-sdk` (dépôt
 * `RemiSDV/cadence-plugins`, API 1), que les interfaces des plugins officiels embarquent :
 *
 * - enveloppe `{ channel: "cadence", api: 1, kind: "request" | "response" | "event" }` ;
 * - réponses `{ id, ok: true, result }` ou `{ id, ok: false, error }` ;
 * - événements `{ topic, data }` : `event` (backend), `run`, `route`, `theme`, `repos`,
 *   `conversations` ;
 * - méthodes `tool.call` (résultat MCP brut), `action.run`, `repos.*`, `tools.*`, `navigate`,
 *   `notify`, `openExternal`, `pickFiles`, `saveFile`, `settings.*`, `secrets.*`,
 *   `conversations.*`.
 *
 * Le pont (`bridge.ts`) répond dans la variante de la requête reçue ; ce module ne fait que
 * traduire, sans DOM ni IPC (tout passe par `HostApi`).
 */
import { saveFileOf, type HostApi, type NotifyKind } from "./bridge";
import { conversationHandlers } from "./conversations";
import { BridgeError, type HelloResult, type SdkError } from "./protocol";

export const CHANNEL = "cadence";

export interface PublishedRequest {
  channel: "cadence";
  api: 1;
  kind: "request";
  id: string;
  method: string;
  params?: unknown;
}

export function asPublishedRequest(data: unknown): PublishedRequest | null {
  if (typeof data !== "object" || data === null) return null;
  const m = data as Record<string, unknown>;
  return m.channel === CHANNEL &&
    m.api === 1 &&
    m.kind === "request" &&
    typeof m.id === "string" &&
    typeof m.method === "string"
    ? (m as unknown as PublishedRequest)
    : null;
}

export function publishedResponse(id: string, outcome: { result: unknown } | { error: SdkError }) {
  return "error" in outcome
    ? { channel: CHANNEL, api: 1, kind: "response", id, ok: false, error: outcome.error }
    : { channel: CHANNEL, api: 1, kind: "response", id, ok: true, result: outcome.result ?? null };
}

/** État d'une exécution vu par le SDK publié. */
type RunStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

/**
 * Événement du pont (`emit(event, data)`) → sujet du SDK publié ; `null` s'il n'a pas
 * d'équivalent. Les événements du backend deviennent `event { name, data }`.
 */
export function publishedEvent(
  event: string,
  data: unknown,
): { topic: string; data: unknown } | null {
  if (event === "route" || event === "theme" || event === "repos" || event === "conversations")
    return { topic: event, data };
  if (event === "run") {
    const run = (data ?? {}) as {
      runId: string;
      text?: string;
      progress?: number | null;
      final?: boolean;
      data?: unknown;
      state?: string;
    };
    if (run.state !== undefined) {
      const status: Record<string, RunStatus> = {
        running: "running",
        "waiting-approval": "running",
        // Tour terminé sur une question (N10) : l'exécution a rendu son résultat.
        "waiting-reply": "succeeded",
        idle: "succeeded",
        failed: "failed",
        interrupted: "cancelled",
      };
      return { topic: "run", data: { runId: run.runId, status: status[run.state] ?? "running" } };
    }
    return {
      topic: "run",
      data: {
        runId: run.runId,
        status: run.final ? "succeeded" : "running",
        message: run.text,
        ...(typeof run.progress === "number" ? { progress: run.progress / 100 } : {}),
        ...(run.final ? { result: run.data ?? run.text } : {}),
      },
    };
  }
  return { topic: "event", data: { name: event, data: data ?? null } };
}

const object = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

function text(params: Record<string, unknown>, key: string): string {
  const value = params[key];
  if (typeof value !== "string" || !value.trim())
    throw new BridgeError("invalid_params", `« ${key} » est obligatoire.`);
  return value;
}

const TOOL_NAME = /^[a-z0-9_]{1,40}$/;
const REPO = /^[\w.-]+\/[\w.-]+$/;

/** Dépôt suivi tel que le SDK publié le décrit. */
export function repoInfo(fullName: string, localPath: string | null = null) {
  const [owner, name] = fullName.split("/") as [string, string];
  return { fullName, owner, name, url: `https://github.com/${fullName}`, localPath };
}

/** Dépôts suivis avec leur clone local (`localPath`), s'il existe déjà. */
async function repoInfos(api: HostApi, repos: string[]) {
  const paths = await api.reposLocalPaths().catch(() => ({}) as Record<string, string>);
  return repos.map((repo) => repoInfo(repo, paths[repo.toLowerCase()] ?? null));
}

/** Sélecteur de la barre du plugin, tel que le SDK publié le décrit. */
function selectionOf(repo: string | null) {
  return repo ? { kind: "one" as const, repo } : { kind: "all" as const };
}

/** Charge de l'événement `repos` du SDK publié : dépôts suivis et sélection de la barre. */
export async function reposChange(api: HostApi, repos: string[], selection: string | null) {
  return { repos: await repoInfos(api, repos), selection: selectionOf(selection) };
}

type Handler = (params: Record<string, unknown>) => unknown;

/** Méthodes du SDK publié, appuyées sur la même `HostApi` que le protocole du cœur. */
export function publishedHandlers(
  api: HostApi,
  hello: () => Omit<HelloResult, "methods">,
): Record<string, Handler> {
  return {
    hello: () => {
      const { api: version, plugin, theme, route } = hello();
      return { api: version, plugin, theme, route };
    },
    "tool.call": async (params) => {
      const tool = text(params, "tool");
      if (tool.includes("__") || !TOOL_NAME.test(tool))
        throw new BridgeError("invalid_params", `« ${tool} » n'est pas un outil de ce plugin.`);
      const result = await api.call(
        tool,
        Object.keys(object(params.args)).length ? object(params.args) : null,
      );
      // Résultat MCP brut : le SDK publié fait lui-même structuredContent / texte / erreur.
      return {
        content: result.text ? [{ type: "text", text: result.text }] : [],
        ...(result.structured !== null && result.structured !== undefined
          ? { structuredContent: result.structured }
          : {}),
        isError: result.isError,
      };
    },
    "action.run": async (params) => {
      const context =
        params.context === undefined || params.context === null ? null : object(params.context);
      const { runId } = await api.runAction(text(params, "action"), context, null);
      return { runId };
    },
    navigate: (params) => {
      const raw = object(params.params);
      api.navigate(
        text(params, "view"),
        Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, String(v)])),
      );
      return null;
    },
    notify: (params) => {
      const kinds: NotifyKind[] = ["info", "success", "warning", "error"];
      api.notify(
        text(params, "message"),
        kinds.includes(params.kind as NotifyKind) ? (params.kind as NotifyKind) : "info",
      );
      return null;
    },
    openExternal: (params) => {
      const url = text(params, "url");
      if (!/^(https?:\/\/|mailto:)/i.test(url))
        throw new BridgeError("invalid_params", "Seuls les liens http(s) et mailto s'ouvrent.");
      api.openExternal(url);
      return null;
    },
    saveFile: (params) => saveFileOf(api, params),
    pickFiles: (params) =>
      api.pickFiles({
        title: typeof params.title === "string" ? params.title : undefined,
        multiple: params.multiple === true,
        directory: params.directory === true,
        filters: Array.isArray(params.filters)
          ? (params.filters as { name: string; extensions: string[] }[])
          : [],
      }),
    "settings.get": async () => (await api.settingsGet()).value,
    "settings.set": async (params) => {
      const current = await api.settingsGet();
      const saved = await api.settingsSet({ ...current.value, ...object(params.patch) });
      if (saved.errors.length > 0)
        throw new BridgeError("invalid_params", "Réglages refusés.", saved.errors);
      return saved.value;
    },
    "secrets.set": async (params) => {
      await api.secretSet(text(params, "id"), text(params, "value"));
      return null;
    },
    "secrets.has": (params) => api.secretHas(text(params, "id")),
    "repos.list": async () => repoInfos(api, await api.reposList()),
    "repos.selection": async () => selectionOf(await api.reposSelection()),
    "repos.follow": async (params) => {
      const repo = text(params, "repo");
      if (!REPO.test(repo))
        throw new BridgeError("invalid_params", `Dépôt invalide : « ${repo} ».`);
      return repoInfos(
        api,
        await api.reposSet((list) => (list.includes(repo) ? list : [...list, repo])),
      );
    },
    "repos.unfollow": async (params) => {
      const repo = text(params, "repo");
      return repoInfos(api, await api.reposSet((list) => list.filter((r) => r !== repo)));
    },
    "repos.checkout": async (params) => {
      const repo = text(params, "repo");
      if (!REPO.test(repo))
        throw new BridgeError("invalid_params", `Dépôt invalide : « ${repo} ».`);
      const tracked = await api.reposList();
      if (!tracked.some((r) => r.toLowerCase() === repo.toLowerCase()))
        throw new BridgeError("not_found", `Le plugin ne suit pas le dépôt ${repo}.`);
      const task = typeof params.task === "string" && params.task.trim() ? params.task : null;
      return { path: await api.reposCheckout(repo, task) };
    },
    "tools.status": (params) => api.toolStatus(text(params, "tool"), false),
    "tools.ensure": (params) => api.toolStatus(text(params, "tool"), true),
    ...conversationHandlers(api),
  };
}
