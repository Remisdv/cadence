/**
 * Briques de la console des agents (maquette Agents-Hub.dc.html) : carte de statut d'un agent
 * et ligne du journal d'exécution.
 */
import type { ReactNode } from "react";
import type { AgentKind } from "@/lib/types";
import { Badge, type BadgeTone } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Note } from "@/components/domain/Panels";
import { cn } from "@/lib/cn";
import { agents } from "@/lib/domain/agents";
import { AgentBadge } from "./AgentBadge";

const borders = {
  none: "border-line",
  amber: "border-amber-line",
  green: "border-green-line",
} as const;

export interface AgentStatusCardProps {
  agent: AgentKind;
  /** Autonomie lisible (« Applique après validation »). */
  autonomy: ReactNode;
  status: { label: ReactNode; tone: BadgeTone; dot?: boolean };
  description: ReactNode;
  /** Encadré : ce qui attend ou ce qui se passe. */
  note?: ReactNode;
  footer: ReactNode;
  /** Bordure teintée : ambre (à valider), verte (en cours). */
  highlight?: "amber" | "green" | null;
  onOpen: () => void;
}

/** Carte d'agent : statut, autonomie, rôle, encadré d'attente, dernier passage et « Ouvrir ». */
export function AgentStatusCard({
  agent,
  autonomy,
  status,
  description,
  note,
  footer,
  highlight,
  onOpen,
}: AgentStatusCardProps) {
  return (
    <div
      className={cn(
        "flex min-w-0 flex-1 flex-col gap-3 rounded-card border bg-panel p-[15px]",
        borders[highlight ?? "none"],
      )}
    >
      <div className="flex items-center gap-2.5">
        <AgentBadge agent={agent} size={32} />
        <div className="min-w-0 grow">
          <div className="truncate text-body-lg font-semibold">{agents[agent].name}</div>
          <div className="mt-px truncate text-tiny text-tx3">{autonomy}</div>
        </div>
        <Badge tone={status.tone} dot={status.dot}>
          {status.label}
        </Badge>
      </div>
      <p className="min-h-[58px] text-small leading-[1.6] text-tx2">{description}</p>
      {note && <Note>{note}</Note>}
      <div className="mt-auto flex items-center gap-2 border-t border-line pt-[11px]">
        <span className="min-w-0 grow text-tiny text-tx3">{footer}</span>
        <Button size="compact" icon="play" onClick={onOpen}>
          Ouvrir
        </Button>
      </div>
    </div>
  );
}

export interface RunLogRowProps {
  agent: AgentKind;
  /** « 11:24 », « hier ». */
  time: ReactNode;
  children: ReactNode;
  status: { label: ReactNode; tone: BadgeTone };
  onClick?: () => void;
}

/** Ligne du journal d'exécution : heure mono, carré d'agent, texte, pastille d'état. */
export function RunLogRow({ agent, time, children, status, onClick }: RunLogRowProps) {
  const Tag = onClick ? "button" : "div";
  return (
    <Tag
      type={onClick ? "button" : undefined}
      onClick={onClick}
      className={cn(
        "flex h-[38px] w-full items-center gap-[11px] border-b border-line px-3.5 text-left",
        onClick && "hover:bg-p2",
      )}
    >
      <span className="mono w-14 shrink-0 text-tiny text-tx3">{time}</span>
      <AgentBadge agent={agent} size={20} />
      <span className="min-w-0 grow truncate text-body-sm text-tx2">{children}</span>
      <Badge tone={status.tone}>{status.label}</Badge>
    </Tag>
  );
}
