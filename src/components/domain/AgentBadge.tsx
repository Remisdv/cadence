import type { AgentKind } from "@/lib/types";
import { agents } from "@/lib/domain/agents";
import { cn } from "@/lib/cn";

/** Tailles relevées dans les maquettes (radius 5 à 17px, 9 à 34px, 7 sinon). */
const sizes = {
  17: "size-[17px] rounded-badge text-[9.5px]",
  19: "size-[19px] rounded-control text-[8px]",
  20: "size-5 rounded-control text-[9px]",
  21: "size-[21px] rounded-control text-[9px]",
  22: "size-[22px] rounded-control text-[10px]",
  26: "size-[26px] rounded-control text-[11px]",
  28: "size-7 rounded-control text-[12px]",
  30: "size-[30px] rounded-control text-[13px]",
  32: "size-8 rounded-control text-[14px]",
  34: "size-[34px] rounded-field text-[14px]",
} as const;

const fills = {
  blue: "bg-blue",
  purple: "bg-purple",
  green: "bg-green",
  amber: "bg-amber",
} as const;

export interface AgentBadgeProps {
  agent: AgentKind;
  size?: keyof typeof sizes;
  className?: string;
}

/** Carré d'agent : lettre Space Grotesk 700 sur l'aplat de la couleur de l'agent. */
export function AgentBadge({ agent, size = 22, className }: AgentBadgeProps) {
  const meta = agents[agent];
  return (
    <span
      title={meta.name}
      className={cn(
        "inline-flex shrink-0 items-center justify-center font-display font-bold text-ink",
        sizes[size],
        fills[meta.tone],
        className,
      )}
    >
      {meta.letter}
    </span>
  );
}
