/** État d'interface transverse : modales globales, palette, bandeau de quota. */
import { create } from "zustand";
import type { RateLimitInfo } from "@/lib/types";

export type GlobalDialog = "issue-create" | "version-close" | null;

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
}));
