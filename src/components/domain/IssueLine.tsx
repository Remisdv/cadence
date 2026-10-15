import type { IssueRow } from "@/lib/types";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { cn } from "@/lib/cn";
import { AgentBadge } from "./AgentBadge";
import { TypeBadge } from "./Badges";
import { IssueStateIcon } from "./Inline";

export interface IssueLineProps {
  issue: IssueRow;
  onOpen?: (number: number) => void;
}

/**
 * Ligne compacte d'issue (portée d'une version) : état, n°, titre, type, puis l'agent qui
 * y travaille ou la personne assignée.
 */
export function IssueLine({ issue, onOpen }: IssueLineProps) {
  const closed = issue.status === "done";
  return (
    <button
      type="button"
      onClick={() => onOpen?.(issue.number)}
      className="flex h-10 w-full shrink-0 items-center gap-[11px] border-b border-line px-3.5 text-left hover:bg-p2"
    >
      <IssueStateIcon status={issue.status} size={14} />
      <span className="w-9 shrink-0 mono text-tiny text-tx3">{issue.number}</span>
      <span className={cn("min-w-0 grow truncate text-body-sm", closed && "text-tx2")}>
        {issue.title}
      </span>
      {issue.issueType && <TypeBadge type={issue.issueType} />}
      <span className="flex w-6 shrink-0 justify-end">
        {issue.activeAgent ? (
          <AgentBadge agent={issue.activeAgent} size={21} />
        ) : (
          issue.assignee && (
            <Avatar
              size={21}
              tone={issue.assignee.isSelf ? "clay" : "neutral"}
              initials={initialsOf(issue.assignee.name ?? issue.assignee.login)}
              src={issue.assignee.avatarUrl}
              title={issue.assignee.login}
            />
          )
        )}
      </span>
    </button>
  );
}
