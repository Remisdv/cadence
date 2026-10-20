/**
 * Mode d'une conversation (`ChatPermissions`, profils de `ai/claude_code.rs`) : ce que l'IA peut
 * faire sans demander. Choisi dans le composeur du chat, mémorisé avec la conversation.
 */
import type { IconName } from "@/components/icons/Icon";
import type { ChatPermissions } from "@/lib/types";

export interface ChatModeInfo {
  label: string;
  /** Une ligne pour le menu du composeur. */
  description: string;
  icon: IconName;
  /** Classe de couleur de l'icône (tokens de `globals.css`). */
  tone: string;
}

export const chatModeOrder: ChatPermissions[] = ["ask", "auto-edit", "full"];

export const chatModes: Record<ChatPermissions, ChatModeInfo> = {
  ask: {
    label: "Demander",
    description: "Chaque commande, modification ou outil attend votre accord.",
    icon: "shield",
    tone: "text-green",
  },
  "auto-edit": {
    label: "Édition auto",
    description: "Modifie les fichiers sans demander ; demande avant une commande shell.",
    icon: "pen",
    tone: "text-blue",
  },
  full: {
    label: "Auto",
    description: "Tout est autorisé, sans demande. À réserver aux tâches sûres.",
    icon: "bolt",
    tone: "text-amber",
  },
};

/** Mode d'une nouvelle conversation. */
export const defaultChatMode: ChatPermissions = "ask";
