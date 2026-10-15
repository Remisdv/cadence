import { QueryClient } from "@tanstack/react-query";
import { isPermanent } from "@/lib/ipc/errors";

/**
 * Les données viennent du cache local côté Rust et sont rafraîchies par événements
 * (`sync://updated`) : pas de refetch au focus ni d'expiration par le temps.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
      retry: (count, error) => !isPermanent(error) && count < 2,
    },
    mutations: {
      retry: false,
    },
  },
});
