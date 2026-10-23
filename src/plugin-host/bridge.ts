/**
 * Pont entre le shell et l'iframe d'un plugin (plan § 3.3).
 *
 * - Un message n'est accepté que s'il vient de l'iframe que le shell a créée pour ce plugin
 *   (`event.source`) : l'identité du plugin vient de l'iframe, jamais du message.
 * - Les réponses partent vers cette fenêtre précise (`postMessage(msg, "*")`, origine opaque).
 * - `tools.call` ne vise que les outils du plugin lui-même : un nom préfixé est refusé.
 *
 * Le pont ne touche ni au DOM ni à l'IPC : tout passe par `HostApi`, ce qui le rend testable
 * sans navigateur.
 *
 * Deux variantes de l'enveloppe sont comprises : celle du cœur (`{ cadence: 1, kind: "req" }`)
 * et celle du SDK publié `@cadence/plugin-sdk` (`published.ts`) ; chaque réponse part dans la
 * variante de la requête, les événements dans celle de la dernière poignée de main.
 */
import type {
  ConversationDetail,
  ConversationInfo,
  PluginSettingsView,
  PluginToolResult,
} from "@/lib/types";
import { CommandError } from "@/lib/ipc/errors";
import {
  BridgeError,
  PROTOCOL_VERSION,
  asRequest,
  codeOfKind,
  type Envelope,
  type HelloResult,
  type SdkError,
} from "./protocol";
import { conversationHandlers } from "./conversations";
import {
  asPublishedRequest,
  publishedEvent,
  publishedHandlers,
  publishedResponse,
} from "./published";

/** Fenêtre de l'iframe (son `contentWindow`). */
export interface FrameTarget {
  postMessage(message: unknown, targetOrigin: string): void;
}

export type NotifyKind = "info" | "success" | "warning" | "error";

/** Ce que l'hôte sait faire pour un plugin (déjà lié à ce plugin). */
export interface HostApi {
  call(tool: string, args: Record<string, unknown> | null): Promise<PluginToolResult>;
  navigate(view: string, params: Record<string, string>): void;
  notify(message: string, kind: NotifyKind): void;
  openExternal(url: string): void;
  shortcut(key: string): void;
  settingsGet(): Promise<PluginSettingsView>;
  settingsSet(value: Record<string, unknown>): Promise<PluginSettingsView>;
  secretSet(id: string, value: string): Promise<void>;
  secretHas(id: string): Promise<boolean>;
  storageGet(key: string): unknown;
  storageSet(key: string, value: unknown): void;
  runAction(
    id: string,
    context: Record<string, unknown> | null,
    ai: string | null,
  ): Promise<{ runId: string; sessionId: string | null; mode: string }>;
  pickFiles(options: {
    title?: string;
    multiple: boolean;
    directory: boolean;
    filters: { name: string; extensions: string[] }[];
  }): Promise<string[]>;
  /** Dépôts suivis par le plugin (service `repos`). */
  reposList(): Promise<string[]>;
  /** Modifie la liste des dépôts suivis ; renvoie la nouvelle liste. */
  reposSet(update: (repos: string[]) => string[]): Promise<string[]>;
  /** Dépôt choisi dans la barre du plugin ; `null` = « Tous les dépôts ». */
  reposSelection(): Promise<string | null>;
  /** Clone local de chaque dépôt suivi (`owner/nom` en minuscules → dossier), s'il existe. */
  reposLocalPaths(): Promise<Record<string, string>>;
  /** Clone au besoin et prépare le worktree de la tâche ; renvoie son chemin. */
  reposCheckout(repo: string, task: string | null): Promise<string>;
  /** État d'un outil externe ; `ensure` propose son installation s'il manque. */
  toolStatus(
    tool: string,
    ensure: boolean,
  ): Promise<{ installed: boolean; version: string | null; authenticated: boolean | null }>;
  /**
   * Conversations ouvertes par les actions du plugin ; avec `group` (id vu par lui), celles du
   * groupe, cachées comprises ; avec `involved`, aussi celles qui ont appelé ses outils (chat
   * global compris, cachées comprises).
   */
  conversationsList(group: string | null, involved?: boolean): Promise<ConversationInfo[]>;
  /**
   * Boîte « Enregistrer sous » de l'OS puis écriture du texte par Cadence ; chemin choisi, ou
   * `null` si l'utilisateur annule. Le plugin ne choisit jamais le chemin lui-même.
   */
  saveFile(options: {
    name: string;
    content: string;
    filters: { name: string; extensions: string[] }[];
  }): Promise<string | null>;
  /** Une conversation du plugin, sans sorties d'outils ; celle d'un autre plugin est introuvable. */
  conversationGet(id: string): Promise<ConversationDetail>;
  /** Ouvre la vue Chat sur une conversation du plugin (refusée si elle n'est pas à lui). */
  conversationOpen(id: string): Promise<void>;
}

export interface BridgeOptions {
  frame: FrameTarget;
  api: HostApi;
  /** Réponse à `hello`, lue au moment de la poignée de main (route et thème à jour). */
  hello: () => Omit<HelloResult, "methods">;
}

/** Message tel que reçu par `window.addEventListener("message")`. */
export interface IncomingMessage {
  source: unknown;
  data: unknown;
}

const TOOL_NAME = /^[a-z0-9_]{1,40}$/;

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

/** Résultat d'outil tel que le SDK le rend : `structuredContent`, sinon le texte (JSON si possible). */
export function toolValue(result: PluginToolResult): unknown {
  if (result.structured !== null && result.structured !== undefined) return result.structured;
  try {
    return JSON.parse(result.text);
  } catch {
    return result.text;
  }
}

type Handler = (params: Record<string, unknown>) => unknown;

/** Taille maximale d'un fichier enregistré par un plugin (`saveFile`). */
export const SAVE_MAX_CHARS = 32 * 1024 * 1024;

/** `saveFile` / `files.save` : `{ name, content, filters? }` → `{ path }` (`null` si annulé). */
export async function saveFileOf(
  api: HostApi,
  params: Record<string, unknown>,
): Promise<{ path: string | null }> {
  const name = text(params, "name");
  const content = params.content;
  if (typeof content !== "string")
    throw new BridgeError("invalid_params", "« content » doit être du texte.");
  if (content.length > SAVE_MAX_CHARS)
    throw new BridgeError(
      "invalid_params",
      "Fichier trop volumineux (32 millions de caractères au plus).",
    );
  const filters = Array.isArray(params.filters)
    ? (params.filters as unknown[])
        .map(object)
        .filter((f) => typeof f.name === "string" && Array.isArray(f.extensions))
        .map((f) => ({
          name: f.name as string,
          extensions: (f.extensions as unknown[]).filter(
            (e): e is string => typeof e === "string" && /^[A-Za-z0-9]{1,10}$/.test(e),
          ),
        }))
    : [];
  return { path: await api.saveFile({ name, content, filters }) };
}

export function createBridge({ frame, api, hello }: BridgeOptions) {
  const handlers: Record<string, Handler> = {
    hello: () => ({ ...hello(), methods: Object.keys(handlers) }),
    "tools.call": async (params) => {
      const tool = text(params, "tool");
      if (tool.includes("__"))
        throw new BridgeError(
          "invalid_params",
          "Un plugin n'appelle que ses propres outils : donnez le nom sans préfixe.",
        );
      if (!TOOL_NAME.test(tool))
        throw new BridgeError("invalid_params", `« ${tool} » n'est pas un nom d'outil.`);
      const args = params.args === undefined || params.args === null ? null : object(params.args);
      const result = await api.call(tool, args);
      if (result.isError)
        throw new BridgeError("tool_error", result.text || "L'outil a échoué.", result.structured);
      return toolValue(result);
    },
    "nav.go": (params) => {
      const raw = object(params.params);
      const view = text(params, "view");
      api.navigate(view, Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, String(v)])));
      return null;
    },
    "ui.notify": (params) => {
      const kinds: NotifyKind[] = ["info", "success", "warning", "error"];
      const kind = kinds.includes(params.kind as NotifyKind) ? (params.kind as NotifyKind) : "info";
      api.notify(text(params, "message"), kind);
      return null;
    },
    "ui.open": (params) => {
      const url = text(params, "url");
      if (!/^(https?:\/\/|mailto:)/i.test(url))
        throw new BridgeError("invalid_params", "Seuls les liens http(s) et mailto s'ouvrent.");
      api.openExternal(url);
      return null;
    },
    "ui.shortcut": (params) => {
      api.shortcut(text(params, "key"));
      return null;
    },
    "settings.get": () => api.settingsGet(),
    "settings.set": async (params) => {
      const current = await api.settingsGet();
      return api.settingsSet({ ...current.value, ...object(params.patch) });
    },
    "secrets.set": async (params) => {
      await api.secretSet(text(params, "id"), text(params, "value"));
      return null;
    },
    "secrets.has": (params) => api.secretHas(text(params, "id")),
    "actions.run": (params) => {
      const ai = typeof params.ai === "string" ? params.ai : null;
      if (ai && !["claude-code", "codex", "opencode"].includes(ai))
        throw new BridgeError("invalid_params", `IA « ${ai} » inconnue.`);
      const context =
        params.context === undefined || params.context === null ? null : object(params.context);
      return api.runAction(text(params, "id"), context, ai);
    },
    "files.save": (params) => saveFileOf(api, params),
    "storage.get": (params) => api.storageGet(text(params, "key")) ?? null,
    "storage.set": (params) => {
      api.storageSet(text(params, "key"), params.value ?? null);
      return null;
    },
    ...conversationHandlers(api),
  };

  const post = (message: Envelope) => frame.postMessage(message, "*");
  const published = publishedHandlers(api, hello);
  /** Variante parlée par l'iframe (fixée par sa dernière requête). */
  let dialect: "core" | "published" = "core";

  async function handlePublished(id: string, method: string, params: unknown) {
    dialect = "published";
    const handler = published[method];
    try {
      if (!handler)
        throw new BridgeError("method_not_found", `Méthode « ${method} » inconnue de cet hôte.`);
      const result = await handler(object(params));
      frame.postMessage(publishedResponse(id, { result }), "*");
    } catch (err) {
      frame.postMessage(publishedResponse(id, { error: toSdkError(err) }), "*");
    }
  }

  /** Traite un message ; `false` s'il ne vient pas de l'iframe de ce plugin. */
  async function handle(message: IncomingMessage): Promise<boolean> {
    if (message.source !== frame) return false;
    const publishedRequest = asPublishedRequest(message.data);
    if (publishedRequest) {
      await handlePublished(publishedRequest.id, publishedRequest.method, publishedRequest.params);
      return true;
    }
    const request = asRequest(message.data);
    if (!request) return true;
    dialect = "core";
    const handler = handlers[request.method];
    try {
      if (!handler)
        throw new BridgeError(
          "method_not_found",
          `Méthode « ${request.method} » inconnue de cet hôte.`,
        );
      const result = await handler(object(request.params));
      post({ cadence: PROTOCOL_VERSION, kind: "res", id: request.id, result: result ?? null });
    } catch (err) {
      post({ cadence: PROTOCOL_VERSION, kind: "res", id: request.id, error: toSdkError(err) });
    }
    return true;
  }

  /** Événement vers l'iframe (`route`, `theme`, événements du serveur du plugin). */
  function emit(event: string, data?: unknown) {
    if (dialect === "published") {
      const translated = publishedEvent(event, data);
      if (translated)
        frame.postMessage({ channel: "cadence", api: 1, kind: "event", ...translated }, "*");
      return;
    }
    post({ cadence: PROTOCOL_VERSION, kind: "evt", event, data });
  }

  return { handle, emit, methods: () => Object.keys(handlers) };
}

export type Bridge = ReturnType<typeof createBridge>;

function toSdkError(err: unknown): SdkError {
  if (err instanceof BridgeError) return { code: err.code, message: err.message, data: err.data };
  if (err instanceof CommandError) return { code: codeOfKind(err.kind), message: err.message };
  return { code: "internal", message: err instanceof Error ? err.message : String(err) };
}
