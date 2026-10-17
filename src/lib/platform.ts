/**
 * Accès au système, avec repli navigateur pour le mode mock :
 * ouvrir une URL, copier, choisir un fichier.
 */
import { isTauri } from "@/lib/ipc/transport";

/** Ouvre une URL dans le navigateur par défaut (jamais dans le webview). */
export async function openExternal(url: string): Promise<void> {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Sélecteur de fichier natif ; `null` si l'utilisateur annule. */
export async function pickFile(title: string): Promise<string | null> {
  if (!isTauri) {
    // Mode navigateur : chemin simulé, pour dérouler le parcours.
    return "/usr/local/bin/codex";
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ title, multiple: false, directory: false });
  return typeof selected === "string" ? selected : null;
}

/** Choix d'une archive de plugin (`.zip`) ; `null` si l'utilisateur annule. */
export async function pickArchive(): Promise<string | null> {
  if (!isTauri) return "C:/Users/moi/Téléchargements/tests-couverture-0.3.1.zip";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    title: "Installer un plugin depuis une archive",
    multiple: false,
    directory: false,
    filters: [{ name: "Archive de plugin", extensions: ["zip"] }],
  });
  return typeof selected === "string" ? selected : null;
}

/** Montre un dossier dans l'explorateur de fichiers (worktree d'un agent). */
export async function revealPath(path: string): Promise<void> {
  if (isTauri) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  } else {
    await copyText(path);
  }
}
