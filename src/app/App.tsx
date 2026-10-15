import { QueryClientProvider } from "@tanstack/react-query";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { queryClient } from "@/lib/query/client";
import { useBackendEvents } from "@/lib/query/useBackendEvents";
import { DialogHost } from "./DialogHost";
import { ViewOutlet } from "./ViewOutlet";
import { useBoot } from "./useBoot";

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Root />
    </QueryClientProvider>
  );
}

function Root() {
  useBackendEvents();
  const { ready, error } = useBoot();

  if (error) return <ErrorState error={error} />;
  if (!ready) return <LoadingState label="Ouverture de Cadence…" />;

  return (
    <>
      <ViewOutlet />
      <CommandPalette />
      <DialogHost />
    </>
  );
}
