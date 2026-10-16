/**
 * Événements back → front. Les noms doivent refléter `src-tauri/src/events.rs`.
 */
import type { AuthChanged, RateLimitInfo, SyncUpdated } from "@/lib/types";
import { isMockIpc } from "./transport";
import { mockBus } from "./mock/bus";

export interface Events {
  "sync://updated": SyncUpdated;
  "sync://rate-limit": RateLimitInfo;
  "auth://changed": AuthChanged;
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
