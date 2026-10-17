/**
 * L'hôte côté shell : adresse de l'interface d'un plugin, thème transmis à l'iframe, et
 * `HostApi` branchée sur l'IPC, la navigation et les toasts.
 */
import { call, isMockIpc } from "@/lib/ipc/transport";
import { openExternal } from "@/lib/platform";
import type { InstalledPluginView } from "@/lib/types";
import { navigate } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";
import type { HostApi } from "./bridge";

const isWindows = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/**
 * URL d'un fichier de l'interface d'un plugin : protocole `cadence-plugin` (plan § 2.7), ou
 * `public/mock-plugins/<id>/` en mode mock.
 */
export function pluginUrl(
  plugin: Pick<InstalledPluginView, "id" | "version" | "uiEntry">,
  path = "",
): string {
  const file = path || plugin.uiEntry || "index.html";
  if (isMockIpc) return `/mock-plugins/${plugin.id}/${file}`;
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
    fetch(pluginUrl(plugin)).then((r) => r.text()),
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

/** `HostApi` d'un plugin : chaque méthode est liée à ce plugin, jamais à un autre. */
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
