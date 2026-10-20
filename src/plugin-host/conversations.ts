/**
 * Méthodes `conversations.*` du SDK (les deux variantes du protocole) : un plugin lit les
 * conversations que ses actions ont ouvertes, regroupées au besoin (un sprint), et en ouvre une
 * dans la vue Chat. L'`HostApi` est liée au plugin : il ne voit que ses conversations et ses
 * groupes (l'hôte préfixe les ids de groupe par l'id du plugin).
 *
 * L'événement `conversations` `{ id, group?, state }` part à chaque création ou changement d'état
 * (relais de `chat://state` par le pool d'iframes).
 */
import type { ChatState, ChatStatePayload } from "@/lib/types";
import type { HostApi } from "./bridge";
import { BridgeError } from "./protocol";

/** Id de groupe accepté (même règle que le contexte d'action). */
export const GROUP_ID = /^[A-Za-z0-9:/._-]{1,120}$/;

type Handler = (params: Record<string, unknown>) => unknown;

function id(params: Record<string, unknown>): string {
  const value = params.id;
  if (typeof value !== "string" || !value.trim())
    throw new BridgeError("invalid_params", "« id » est obligatoire.");
  return value;
}

export function conversationHandlers(api: HostApi): Record<string, Handler> {
  return {
    "conversations.list": (params) => {
      const group = params.group;
      if (group === undefined || group === null) return api.conversationsList(null);
      if (typeof group !== "string" || !GROUP_ID.test(group))
        throw new BridgeError("invalid_params", "« group » n'est pas un id de groupe valide.");
      return api.conversationsList(group);
    },
    "conversations.get": (params) => api.conversationGet(id(params)),
    "conversations.open": async (params) => {
      await api.conversationOpen(id(params));
      return null;
    },
  };
}

/** Charge de l'événement `conversations` d'un plugin. */
export interface ConversationEvent {
  id: string;
  group?: { id: string; label: string };
  state: ChatState;
}

/** `chat://state` → événement `conversations` du plugin ; `null` si la conversation n'est pas à lui. */
export function conversationEvent(
  plugin: string,
  payload: ChatStatePayload,
): ConversationEvent | null {
  if (payload.plugin !== plugin) return null;
  return {
    id: payload.session,
    ...(payload.group ? { group: payload.group } : {}),
    state: payload.state,
  };
}
