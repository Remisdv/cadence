/**
 * Méthodes `conversations.*` du SDK (les deux variantes du protocole) : un plugin lit les
 * conversations que ses actions ont ouvertes, regroupées au besoin (un sprint), et en ouvre une
 * dans la vue Chat. L'`HostApi` est liée au plugin : il ne voit que ses conversations et ses
 * groupes (l'hôte préfixe les ids de groupe par l'id du plugin).
 *
 * L'événement `conversations` `{ id, group?, state, waiting? }` part à chaque création ou
 * changement d'état (relais de `chat://state` par le pool d'iframes).
 *
 * Compatibilité du SDK (API 1) : ses états restent `idle`, `running`, `waiting-approval`,
 * `failed`, `interrupted`. « En attente de votre réponse » (`waiting-reply` dans Cadence, tour
 * terminé sur une question, recette du 21/10, N10) y devient `waiting-approval` avec
 * `waiting: "reply"` ; une demande d'accord ou une question à choix porte `waiting: "approval"`.
 * Même règle pour `conversations.list/get` (côté Rust, `sdk_state`).
 */
import type { ChatState, ChatStatePayload, ConversationWaiting } from "@/lib/types";
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
  /** État au sens du SDK : jamais `waiting-reply` (voir l'en-tête). */
  state: Exclude<ChatState, "waiting-reply">;
  waiting?: ConversationWaiting;
}

/** État de Cadence → état et attente vus par le SDK (API 1). */
export function sdkState(state: ChatState): Pick<ConversationEvent, "state" | "waiting"> {
  if (state === "waiting-reply") return { state: "waiting-approval", waiting: "reply" };
  if (state === "waiting-approval") return { state, waiting: "approval" };
  return { state };
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
    ...sdkState(payload.state),
  };
}
