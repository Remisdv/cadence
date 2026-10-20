/**
 * État d'interface transverse : modales globales, palette, bandeau de quota, toasts, et les
 * préférences d'affichage du chat (gardées dans le stockage local du webview).
 */
import { create } from "zustand";
import type { RateLimitInfo } from "@/lib/types";

export type GlobalDialog = "issue-create" | "version-close" | null;

export type ToastKind = "info" | "success" | "warning" | "error";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
  /** Qui l'a émis (nom d'un plugin), affiché en petit. */
  source?: string;
}

interface UiState {
  dialog: GlobalDialog;
  openDialog: (dialog: Exclude<GlobalDialog, null>) => void;
  closeDialog: () => void;

  /** Sélecteur de dépôt de la barre latérale. */
  projectPickerOpen: boolean;
  setProjectPickerOpen: (open: boolean) => void;

  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;

  rateLimit: RateLimitInfo | null;
  setRateLimit: (info: RateLimitInfo) => void;

  toasts: Toast[];
  pushToast: (toast: Omit<Toast, "id">) => void;
  dismissToast: (id: number) => void;

  /** Chat : commandes, appels d'outils et fichiers montrés dans le fil (sinon résumés). */
  showCommands: boolean;
  setShowCommands: (show: boolean) => void;
}

let nextToast = 1;

/** Préférence « afficher les commandes » du chat, gardée entre deux lancements. */
export const SHOW_COMMANDS_KEY = "cadence.chat.showCommands";

function readShowCommands(): boolean {
  try {
    return globalThis.localStorage?.getItem(SHOW_COMMANDS_KEY) !== "false";
  } catch {
    return true;
  }
}

function writeShowCommands(show: boolean) {
  try {
    globalThis.localStorage?.setItem(SHOW_COMMANDS_KEY, String(show));
  } catch {
    // Stockage indisponible : la préférence vaut pour cette session seulement.
  }
}

export const useUiStore = create<UiState>((set) => ({
  dialog: null,
  openDialog: (dialog) => set({ dialog, paletteOpen: false }),
  closeDialog: () => set({ dialog: null }),

  projectPickerOpen: false,
  setProjectPickerOpen: (projectPickerOpen) => set({ projectPickerOpen, paletteOpen: false }),

  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),

  rateLimit: null,
  setRateLimit: (rateLimit) => set({ rateLimit }),

  // Quatre toasts au plus : les plus anciens partent.
  toasts: [],
  pushToast: (toast) =>
    set((s) => ({ toasts: [...s.toasts, { ...toast, id: nextToast++ }].slice(-4) })),
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  showCommands: readShowCommands(),
  setShowCommands: (showCommands) => {
    writeShowCommands(showCommands);
    set({ showCommands });
  },
}));
