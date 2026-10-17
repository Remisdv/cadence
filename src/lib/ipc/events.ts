/**
 * Événements back → front. Les noms doivent refléter `src-tauri/src/events.rs`.
 */
import type {
  AuthChanged,
  ChatEventPayload,
  ChatStatePayload,
  OpenViewRequest,
  PluginEventPayload,
  PluginInstallEvent,
  PluginStateEvent,
  RateLimitInfo,
  RunReport,
  SyncUpdated,
} from "@/lib/types";
import { isMockIpc } from "./transport";
import { mockBus } from "./mock/bus";

export interface Events {
  "sync://updated": SyncUpdated;
  "sync://rate-limit": RateLimitInfo;
  "auth://changed": AuthChanged;
  "plugin://state": PluginStateEvent;
  "plugin://install": PluginInstallEvent;
  /** `notifications/cadence/event` d'un serveur, relayée aux iframes du plugin. */
  "plugin://event": PluginEventPayload;
  /** Une IA demande d'ouvrir une vue de plugin (`cadence__open_view`). */
  "ui://open-view": OpenViewRequest;
  /** Progression ou résultat d'une exécution de fond (`cadence__run_report`). */
  "run://report": RunReport;
  /** Événement d'une conversation ; `seq` nul pour un delta (jamais enregistré). */
  "chat://event": ChatEventPayload;
  "chat://state": ChatStatePayload;
}

export type EventName = keyof Events;
export type Unlisten = () => void;

/** S'abonne à un événement ; renvoie la fonction de désabonnement (synchrone, sûre en effet React). */
export function subscribe<E extends EventName>(
  event: E,
  handler: (payload: Events[E]) => void,
): Unlisten {
  if (isMockIpc) {
    return mockBus.on(event, handler);
  }

  let disposed = false;
  let unlisten: Unlisten | undefined;
  import("@tauri-apps/api/event").then(({ listen }) =>
    listen<Events[E]>(event, (e) => handler(e.payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    }),
  );
  return () => {
    disposed = true;
    unlisten?.();
  };
}
