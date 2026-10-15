/** Bus d'événements en mémoire, utilisé à la place de `@tauri-apps/api/event` en mode mock. */
type Handler = (payload: never) => void;

const handlers = new Map<string, Set<Handler>>();

export const mockBus = {
  on<P>(event: string, handler: (payload: P) => void) {
    const set = handlers.get(event) ?? new Set<Handler>();
    set.add(handler as Handler);
    handlers.set(event, set);
    return () => {
      set.delete(handler as Handler);
    };
  },
  emit<P>(event: string, payload: P) {
    handlers.get(event)?.forEach((h) => (h as (p: P) => void)(payload));
  },
};
