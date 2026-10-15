/**
 * Version de l'app et mise à jour automatique (plugins Tauri `updater` et `process`).
 *
 * Le manifeste `latest.json` est lu sur les releases GitHub de Cadence, la signature de
 * l'installeur est vérifiée côté Rust avec la clé publique de `tauri.conf.json` : le webview
 * ne télécharge rien lui-même. En mode mock (navigateur, `pnpm tauri:mock`), tout est simulé ;
 * `?update=available` annonce une version, `?update=fail` fait échouer la vérification.
 */
import type { Update } from "@tauri-apps/plugin-updater";
import { isMockIpc, isTauri } from "@/lib/ipc/transport";
import { version as packageVersion } from "../../package.json";

export interface AvailableUpdate {
  version: string;
  currentVersion: string;
  /** Date de publication (RFC 3339), si le manifeste la donne. */
  date: string | null;
  /** Notes de version, si le manifeste les donne. */
  notes: string | null;
}

/** Avancement du téléchargement, en octets ; `total` inconnu tant que le serveur ne l'a pas dit. */
export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

/** Mise à jour trouvée par la dernière vérification : une ressource côté Rust. */
let pending: Update | null = null;

const mockScenario = () =>
  typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("update");
const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Version de l'app en cours d'exécution (celle de `tauri.conf.json` dans l'app de bureau). */
export async function appVersion(): Promise<string> {
  if (!isTauri) return packageVersion;
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

/** Interroge le point de terminaison des releases ; `null` si l'app est à jour. */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  if (isMockIpc) {
    await delay(900);
    const scenario = mockScenario();
    if (scenario === "fail")
      throw new Error("Vérification impossible : le serveur des mises à jour ne répond pas.");
    if (scenario !== "available") return null;
    return {
      version: "1.0.1",
      currentVersion: packageVersion,
      date: "2026-10-14T09:00:00Z",
      notes: "Corrections de stabilité de l'agent développeur.",
    };
  }
  const { check } = await import("@tauri-apps/plugin-updater");
  let update: Update | null;
  try {
    update = await check({ timeout: 20_000 });
  } catch (error) {
    throw new Error(`Vérification impossible : ${String(error)}`);
  }
  if (pending && pending !== update) void pending.close().catch(() => {});
  pending = update;
  if (!update) return null;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    date: update.date ?? null,
    notes: update.body?.trim() || null,
  };
}

/**
 * Télécharge et installe la mise à jour trouvée par la dernière vérification. Sous Windows,
 * l'installeur ferme l'app et la relance lui-même ; ailleurs, appeler `restartApp()` ensuite.
 */
export async function installUpdate(onProgress: (progress: UpdateProgress) => void): Promise<void> {
  if (isMockIpc) {
    const total = 18_400_000;
    for (let downloaded = 0; downloaded <= total; downloaded += total / 8) {
      onProgress({ downloaded, total });
      await delay(250);
    }
    return;
  }
  if (!pending) throw new Error("Aucune mise à jour à installer : relancez la vérification.");
  let downloaded = 0;
  let total: number | null = null;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? null;
      else if (event.event === "Progress") downloaded += event.data.chunkLength;
      onProgress({ downloaded, total });
    });
  } catch (error) {
    throw new Error(`Installation impossible : ${String(error)}`);
  }
}

/** Redémarre l'app sur la version installée (sans effet en mode mock, à part recharger). */
export async function restartApp(): Promise<void> {
  if (isMockIpc || !isTauri) {
    window.location.reload();
    return;
  }
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
