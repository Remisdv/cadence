/**
 * L'hôte côté shell : adresse de l'interface d'un plugin, thème transmis à l'iframe, et
 * `HostApi` branchée sur l'IPC, la navigation et les toasts.
 */
import { CommandError } from "@/lib/ipc/errors";
import { call, isMockIpc } from "@/lib/ipc/transport";
import { openExternal, pickPaths } from "@/lib/platform";
import type { AiId, InstalledPluginView } from "@/lib/types";
import { navigate } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";
import type { HostApi } from "./bridge";
import { BridgeError } from "./protocol";

const isWindows = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/**
 * URL d'un fichier de l'interface d'un plugin : protocole `cadence-plugin` (plan § 2.7), ou
 * `public/mock-plugins/<id>/` en mode mock.
 */
export function pluginUrl(
  plugin: Pick<InstalledPluginView, "id" | "version" | "uiEntry" | "uiDevUrl">,
  path = "",
): string {
  const file = path || plugin.uiEntry || "index.html";
  if (isMockIpc) return `/mock-plugins/${plugin.id}/${file}`;
  // Plugin de dev avec `ui.dev_url` : son serveur de dev local (rechargement à chaud).
  if (plugin.uiDevUrl) return `${plugin.uiDevUrl}/${path}`;
  const base = isWindows ? "http://cadence-plugin.localhost" : "cadence-plugin://localhost";
  return `${base}/${plugin.id}/${plugin.version}/${path}`;
}

/**
 * Mode mock : la page du plugin, avec le SDK intégré, pour `srcdoc`. Le serveur de dev de
 * Vite refuse les scripts demandés depuis une origine opaque (iframe `sandbox` sans
 * `allow-same-origin`) ; dans l'app, le protocole `cadence-plugin` les sert lui-même.
 */
export async function mockFrameDocument(
  plugin: Pick<InstalledPluginView, "id" | "version" | "uiEntry">,
) {
  const [page, sdk] = await Promise.all([
    fetch(pluginUrl({ ...plugin, uiDevUrl: null })).then((r) => r.text()),
    fetch("/mock-plugins/cadence-sdk.js").then((r) => r.text()),
  ]);
  return page.replace(
    /<script src="\.\.\/cadence-sdk\.js"><\/script>/,
    () => `<script>${sdk.replace(/<\/script/gi, "<\\/script")}</script>`,
  );
}

const THEME_PREFIXES = ["--color-", "--text-", "--radius-", "--font-", "--shadow-"];

/** Variables CSS du shell transmises à l'iframe (le SDK les pose sur son `:root`). */
export function shellTheme(): Record<string, string> {
  if (typeof document === "undefined") return {};
  const style = getComputedStyle(document.documentElement);
  const theme: Record<string, string> = {};
  for (let i = 0; i < style.length; i++) {
    const name = style.item(i);
    if (THEME_PREFIXES.some((p) => name.startsWith(p)))
      theme[name] = style.getPropertyValue(name).trim();
  }
  return theme;
}

const storageKey = (plugin: string, key: string) => `cadence.plugin.${plugin}.${key}`;

/** Conversation d'un plugin ; celle d'un autre (ou absente) : erreur `not_found`. */
async function conversationOf(plugin: string, conversation: string) {
  try {
    return await call("plugin_conversation_get", { plugin, id: conversation });
  } catch (err) {
    if (err instanceof CommandError && err.kind === "validation")
      throw new BridgeError("not_found", err.message);
    throw err;
  }
}

/** `HostApi` d'un plugin : chaque méthode est liée à ce plugin, jamais à un autre. */
/** Exécutions lancées par chaque plugin (`runId` → plugin), pour lui relayer leur suivi. */
export const pluginRuns = new Map<string, string>();

export function hostApi(plugin: Pick<InstalledPluginView, "id" | "name">): HostApi {
  const id = plugin.id;
  return {
    call: (tool, args) => call("plugin_call", { plugin: id, tool, args }),
    navigate: (view, params) => navigate({ id: "plugin", plugin: id, view, params }),
    notify: (message, kind) =>
      useUiStore.getState().pushToast({ message, kind, source: plugin.name }),
    openExternal: (url) => void openExternal(url),
    shortcut: (key) => {
      if (key.toLowerCase() === "k") useUiStore.getState().setPaletteOpen(true);
    },
    settingsGet: () => call("plugin_settings_get", { plugin: id }),
    settingsSet: (value) => call("plugin_settings_set", { plugin: id, value }),
    secretSet: async (secret, value) => {
      await call("plugin_secret_set", { plugin: id, secret, value });
    },
    secretHas: (secret) => call("plugin_secret_has", { plugin: id, secret }),
    pickFiles: (options) => pickPaths(options),
    reposList: async () => (await call("app_settings_get")).trackedRepos[id] ?? [],
    reposSet: async (update) => {
      const current = (await call("app_settings_get")).trackedRepos[id] ?? [];
      const next = update(current);
      const saved = await call("app_settings_set", { patch: { trackedRepos: { [id]: next } } });
      return saved.trackedRepos[id] ?? [];
    },
    reposSelection: async () => (await call("app_settings_get")).repoSelection[id] ?? null,
    reposLocalPaths: async () =>
      Object.fromEntries(
        (await call("repos_status"))
          .filter((clone) => clone.path !== null)
          .map((clone) => [clone.repo.toLowerCase(), clone.path as string]),
      ),
    reposCheckout: async (repo, task) =>
      (await call("repos_checkout", { plugin: id, repo, task })).path,
    toolStatus: async (tool, ensure) => {
      const report = await call("tools_list", { refresh: false });
      const info = report.tools.find((t) => t.id === tool);
      const status = {
        installed: info !== undefined && info.status !== "missing",
        version: info?.version ?? null,
        authenticated: info?.login ? info.status === "ready" : null,
      };
      if (ensure && info && info.status !== "ready")
        useUiStore.getState().pushToast({
          message: `${plugin.name} a besoin de ${info.name} : installez-le ou connectez-le dans Réglages › Outils externes.`,
          kind: "warning",
          source: plugin.name,
        });
      return status;
    },
    runAction: async (action, context, ai) => {
      const result = await call("action_run", {
        input: { plugin: id, action, context, ...(ai ? { ai: ai as AiId } : {}) },
      });
      pluginRuns.set(result.session, id);
      // Mode chat : la conversation s'ouvre dans le shell.
      if (result.mode === "chat") navigate({ id: "chat", session: result.session });
      return {
        runId: result.session,
        sessionId: result.mode === "chat" ? result.session : null,
        mode: result.mode,
      };
    },
    conversationsList: (group, involved) =>
      call("plugin_conversations", { plugin: id, group, involved: involved ?? false }),
    saveFile: ({ name, content, filters }) =>
      call("plugin_file_save", { plugin: id, name, content, filters }),
    conversationGet: (conversation) => conversationOf(id, conversation),
    conversationOpen: async (conversation) => {
      // Vérifiée d'abord : un plugin n'ouvre que ses propres conversations.
      await conversationOf(id, conversation);
      navigate({ id: "chat", session: conversation });
    },
    storageGet: (key) => {
      try {
        const raw = localStorage.getItem(storageKey(id, key));
        return raw === null ? null : JSON.parse(raw);
      } catch {
        return null;
      }
    },
    storageSet: (key, value) => {
      try {
        if (value === null) localStorage.removeItem(storageKey(id, key));
        else localStorage.setItem(storageKey(id, key), JSON.stringify(value));
      } catch {
        // Stockage indisponible (quota, navigation privée) : l'état d'interface est perdu, rien de plus.
      }
    },
  };
}
