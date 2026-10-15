/**
 * Indicateurs en ligne (icône + texte, sans fond) : états, CI, diffstat, branche.
 */
import type { ReactNode } from "react";
import type { CiState, IssueStatus, PrState } from "@/lib/types";
import { Icon, type IconName } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";
import { prStateMeta } from "@/lib/domain/issues";

/** Icône d'état d'issue : ouverte (vert) ou fermée (coche violette). */
export function IssueStateIcon({ status, size = 15 }: { status: IssueStatus; size?: number }) {
  const closed = status === "done";
  return (
    <span className={cn("flex", closed ? "text-purple" : "text-green")}>
      <Icon name={closed ? "check" : "issue"} size={size} label={closed ? "Fermée" : "Ouverte"} />
    </span>
  );
}

export function PrStateIcon({ state, size = 16 }: { state: PrState; size?: number }) {
  return (
    <span className={cn("flex", prStateMeta[state].iconClass)}>
      <Icon name="pullRequest" size={size} label={`PR ${prStateMeta[state].label}`} />
    </span>
  );
}

/** Statut de CI : « en cours » (ambre), « 4/4 » (vert), « 3/4 » (rouge). */
export function CiStatus({
  state,
  passed,
  total,
}: {
  state: CiState;
  passed?: number;
  total?: number;
}) {
  if (state === "none") return <span className="text-caption text-tx3">—</span>;
  const meta: Record<Exclude<CiState, "none">, { icon: IconName; cls: string }> = {
    pending: { icon: "clock", cls: "text-amber" },
    success: { icon: "check", cls: "text-green" },
    failure: { icon: "close", cls: "text-red" },
  };
  const m = meta[state];
  const label =
    state === "pending"
      ? "en cours"
      : total !== undefined
        ? `${passed ?? 0}/${total}`
        : state === "success"
          ? "verte"
          : "en échec";
  return (
    <span className={cn("inline-flex items-center gap-1 text-caption", m.cls)}>
      <Icon name={m.icon} size={13} />
      {label}
    </span>
  );
}

/** Indicateur générique icône + texte 11px (« description incomplète », « PR #86 », « fusionnée »). */
export function InlineMeta({
  icon,
  tone = "tx3",
  children,
  dim,
}: {
  icon: IconName;
  tone?: "tx3" | "amber" | "red" | "green" | "purple" | "blue";
  children: ReactNode;
  /** Icône atténuée (état « en attente »). */
  dim?: boolean;
}) {
  const color = {
    tx3: "text-tx3",
    amber: "text-amber",
    red: "text-red",
    green: "text-green",
    purple: "text-purple",
    blue: "text-blue",
  }[tone];
  return (
    <span className={cn("inline-flex items-center gap-1 text-tiny", color)}>
      <Icon name={icon} size={12} className={dim ? "opacity-70" : undefined} />
      {children}
    </span>
  );
}

/** « +412 −87 » : vert / rouge, ou neutre dans les listes de PR. */
export function Diffstat({
  additions,
  deletions,
  plain,
  className,
}: {
  additions: number;
  deletions: number;
  plain?: boolean;
  className?: string;
}) {
  return (
    <span className={cn("mono text-tiny", plain && "text-tx3", className)}>
      <span className={plain ? undefined : "text-green"}>+{additions}</span>{" "}
      <span className={plain ? undefined : "text-red"}>−{deletions}</span>
    </span>
  );
}

/** « ⑂ feat/142-github-device-flow → main » */
export function BranchRef({
  head,
  base,
  arrowIcon,
}: {
  head: string;
  base: string;
  arrowIcon?: boolean;
}) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 text-caption text-tx3">
      <Icon name="branch" size={12} />
      <span className="truncate mono">{head}</span>
      {arrowIcon ? <Icon name="arrowRight" size={12} /> : <span className="text-line2">→</span>}
      <span className="mono">{base}</span>
    </span>
  );
}
