import { useEffect, useState } from "react";

/** Secondes écoulées depuis `since` (ISO), rafraîchies chaque seconde. */
export function useElapsedSeconds(since: string | null | undefined): number {
  const start = since ? new Date(since).getTime() : null;
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (start === null) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [start]);

  return start === null ? 0 : Math.max(0, (now - start) / 1000);
}
