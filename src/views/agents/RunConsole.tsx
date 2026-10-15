/** Journal d'une exécution d'agent, complété en direct par `agent://event`. */
import { useEffect, useState } from "react";
import type { AgentLogLine, AgentRunDetail } from "@/lib/types";
import { subscribe } from "@/lib/ipc/events";

export function useRunLog(run: AgentRunDetail): AgentLogLine[] {
  const [live, setLive] = useState<AgentLogLine[]>([]);
  // Un nouveau journal relu efface les lignes reçues entre-temps (elles y sont).
  useEffect(() => setLive([]), [run.log.length]);
  useEffect(
    () =>
      subscribe("agent://event", (e) => {
        if (e.run === run.run) setLive((l) => [...l, { kind: e.kind, text: e.text }]);
      }),
    [run.run],
  );
  return [...run.log, ...live];
}
