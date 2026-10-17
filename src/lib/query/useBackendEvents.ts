import { useEffect } from "react";
import type { ChatSessionDetail } from "@/lib/types";
import { useQueryClient } from "@tanstack/react-query";
import { subscribe } from "@/lib/ipc/events";
import { keys, keysForResource } from "./keys";
import { useUiStore } from "@/stores/ui";

/**
 * Pont événements Rust → cache front. Monté une fois à la racine de l'app.
 * Les vues n'ont rien à faire : leurs requêtes se rafraîchissent seules.
 */
export function useBackendEvents() {
  const queryClient = useQueryClient();
  const setRateLimit = useUiStore((s) => s.setRateLimit);

  useEffect(() => {
    const offs = [
      subscribe("sync://updated", ({ resource }) => {
        for (const key of keysForResource[resource]) {
          void queryClient.invalidateQueries({ queryKey: key });
        }
      }),
      subscribe("sync://rate-limit", setRateLimit),
      subscribe("auth://changed", () => {
        void queryClient.invalidateQueries({ queryKey: keys.auth });
      }),
      subscribe("plugin://state", () => {
        void queryClient.invalidateQueries({ queryKey: keys.pluginStatus() });
      }),
      subscribe("plugin://install", ({ phase }) => {
        if (phase === "done") void queryClient.invalidateQueries({ queryKey: keys.plugins });
      }),
      subscribe("catalog://updated", () => {
        void queryClient.invalidateQueries({ queryKey: keys.catalog(), refetchType: "none" });
      }),
      subscribe("chat://event", ({ session, seq, turn, event }) => {
        const drafts = keys.chatDrafts(session);
        if (event.type === "message-delta") {
          queryClient.setQueryData<Record<string, string>>(drafts, (prev = {}) => ({
            ...prev,
            [event.message]: (prev[event.message] ?? "") + event.text,
          }));
          return;
        }
        if (event.type === "message-completed" || event.type === "turn-completed") {
          queryClient.setQueryData<Record<string, string>>(drafts, (prev = {}) => {
            if (event.type === "turn-completed") return {};
            const { [event.message]: _done, ...rest } = prev;
            return rest;
          });
        }
        if (seq === null) return;
        queryClient.setQueryData<ChatSessionDetail>(keys.chatSession(session), (prev) =>
          prev && !prev.events.some((e) => e.seq === seq)
            ? {
                ...prev,
                events: [...prev.events, { seq, turn, at: new Date().toISOString(), event }],
              }
            : prev,
        );
      }),
      subscribe("chat://state", ({ session, state }) => {
        queryClient.setQueryData<ChatSessionDetail>(keys.chatSession(session), (prev) =>
          prev ? { ...prev, session: { ...prev.session, state } } : prev,
        );
        void queryClient.invalidateQueries({ queryKey: keys.chatSessions() });
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [queryClient, setRateLimit]);
}
