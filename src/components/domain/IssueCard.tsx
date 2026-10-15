import type { ReactNode } from "react";
import type { IssueRow } from "@/lib/types";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { issueStatuses } from "@/lib/domain/issues";
import { AgentBadge } from "./AgentBadge";
import { TypeBadge } from "./Badges";
import { InlineMeta } from "./Inline";

export interface IssueCardProps {
  issue: IssueRow;
  onOpen?: (number: number) => void;
}

/** Carte du tableau de sprint : n°, type, titre, puis le signal le plus utile. */
export function IssueCard({ issue, onOpen }: IssueCardProps) {
  return (
    <button
      type="button"
      onClick={() => onOpen?.(issue.number)}
      className="block w-full rounded-field border border-line bg-p2 px-[11px] py-2.5 text-left transition-colors hover:border-line2"
    >
      <div className="flex items-center gap-[7px]">
        <span className="mono text-tiny text-tx3">#{issue.number}</span>
        <div className="grow" />
        {issue.issueType && <TypeBadge type={issue.issueType} prefixed />}
      </div>
      <div className="mt-1.5 text-body-sm leading-[1.45] text-tx">{issue.title}</div>
      <IssueCardFooter issue={issue} />
    </button>
  );
}

function IssueCardFooter({ issue }: { issue: IssueRow }) {
  const parts: ReactNode[] = [];

  if (issue.activeAgent) {
    parts.push(
      <span key="agent" className="inline-flex items-center gap-[7px]">
        <AgentBadge agent={issue.activeAgent} size={19} />
        <span className="text-tiny text-green">agent {issue.activeAgent}</span>
      </span>,
    );
  } else if (issue.assignee) {
    parts.push(
      <Avatar
        key="assignee"
        size={19}
        tone={issue.assignee.isSelf ? "clay" : "neutral"}
        initials={initialsOf(issue.assignee.name ?? issue.assignee.login)}
        title={issue.assignee.login}
      />,
    );
  }
  if (issue.estimate) {
    parts.push(
      <span key="estimate" className="text-tiny text-tx3">
        {issue.estimate}
      </span>,
    );
  }
  if (issue.flags.includes("incomplete-description")) {
    parts.push(
      <InlineMeta key="incomplete" icon="warning" tone="amber">
        description incomplète
      </InlineMeta>,
    );
  }
  if (issue.flags.includes("ci-failed")) {
    parts.push(
      <InlineMeta key="ci" icon="close" tone="red">
        CI en échec
      </InlineMeta>,
    );
  } else if (issue.linkedPr && issue.status === "review") {
    parts.push(
      <InlineMeta key="pr" icon="pullRequest">
        PR #{issue.linkedPr}
      </InlineMeta>,
    );
  } else if (issue.linkedPr && issue.status === "done") {
    parts.push(
      <InlineMeta key="merged" icon="pullRequest" tone="purple">
        fusionnée
      </InlineMeta>,
    );
  }

  if (parts.length === 0) return null;
  return <div className="mt-[9px] flex items-center gap-[7px]">{parts}</div>;
}

export interface KanbanColumnProps {
  status: IssueRow["status"];
  issues: IssueRow[];
  /** Nombre total si la colonne est tronquée (colonne « Fait »). */
  total?: number;
  onOpen?: (number: number) => void;
  className?: string;
}

export function KanbanColumn({ status, issues, total, onOpen, className }: KanbanColumnProps) {
  const meta = issueStatuses[status];
  return (
    <div className={cn("flex min-w-0 flex-1 flex-col gap-[9px]", className)}>
      <div className="flex items-center gap-[7px] px-0.5">
        <StatusDot tone={meta.dot} size={7} />
        <span className="text-small font-semibold">{meta.label}</span>
        <span className="mono text-tiny text-tx3">{total ?? issues.length}</span>
      </div>
      <div className="flex flex-col gap-2">
        {issues.map((issue) => (
          <IssueCard key={issue.number} issue={issue} onOpen={onOpen} />
        ))}
      </div>
    </div>
  );
}
