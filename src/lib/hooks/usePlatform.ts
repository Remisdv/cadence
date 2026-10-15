import type { KeyboardEvent as ReactKeyboardEvent } from "react";

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.userAgent);

/** Touche de commande affichée dans les raccourcis : ⌘ sur macOS, Ctrl ailleurs. */
export const modKey = isMac ? "⌘" : "Ctrl+";

/** Vrai si l'événement porte la touche de commande de la plateforme. */
export function hasModKey(e: KeyboardEvent | ReactKeyboardEvent): boolean {
  return isMac ? e.metaKey : e.ctrlKey;
}
