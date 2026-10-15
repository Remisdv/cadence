import { useEffect } from "react";
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
      subscribe("agent://state", () => {
        void queryClient.invalidateQueries({ queryKey: keys.agentsOverview() });
      }),
      subscribe("auth://changed", () => {
        void queryClient.invalidateQueries({ queryKey: keys.auth });
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [queryClient, setRateLimit]);
}
