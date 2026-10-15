import type { BurndownPoint } from "@/lib/types";
import { formatDay } from "@/lib/domain/format";

export interface BurndownProps {
  points: BurndownPoint[];
  /** Nombre d'issues au départ (haut de l'axe). */
  scope: number;
  start: string;
  end: string;
  /** Nombre de jours du sprint (points attendus sur l'axe x). */
  days: number;
  label?: string;
}

const W = 320;
const H = 76;
const TOP = 8;
const BOTTOM = 66;

/**
 * Burndown du sprint : droite idéale en tirets, réel en clay, projection atténuée
 * du dernier point jusqu'à zéro à l'échéance.
 */
export function Burndown({ points, scope, start, end, days, label }: BurndownProps) {
  const step = W / Math.max(1, days - 1);
  const y = (remaining: number) => BOTTOM - (remaining / Math.max(1, scope)) * (BOTTOM - TOP);
  const coords = points.map((p, i) => [i * step, y(p.remaining)] as const);
  const last = coords.at(-1);

  return (
    <div>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        width="100%"
        height={H}
        role="img"
        aria-label={label ?? "Burndown du sprint"}
        className="block"
      >
        <line
          x1="0"
          y1={TOP}
          x2={W}
          y2={BOTTOM}
          stroke="var(--color-line2)"
          strokeWidth="1.4"
          strokeDasharray="4 4"
        />
        <polyline
          points={coords.map((c) => c.join(",")).join(" ")}
          fill="none"
          stroke="var(--color-clay)"
          strokeWidth="2.2"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
        {last && (
          <>
            <polyline
              points={`${last.join(",")} ${W},${BOTTOM}`}
              fill="none"
              stroke="var(--color-clay)"
              strokeWidth="2"
              strokeDasharray="4 4"
              opacity=".45"
            />
            <circle cx={last[0]} cy={last[1]} r="3.4" fill="var(--color-clay)" />
          </>
        )}
      </svg>
      <div className="mt-1.5 flex justify-between">
        <span className="mono text-micro text-tx3">{formatDay(start)}</span>
        <span className="mono text-micro text-tx3">{formatDay(end)}</span>
      </div>
    </div>
  );
}
