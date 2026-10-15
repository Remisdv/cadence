/** Tableau des issues groupées par version (maquette Issues.dc.html). */
import { useState } from "react";
import type { IssueRow } from "@/lib/types";
import { Icon } from "@/components/icons/Icon";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { IssueStateIcon } from "@/components/domain/Inline";
import { FooterBar } from "@/components/domain/Panels";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { cn } from "@/lib/cn";
import { formatPercent } from "@/lib/domain/format";
import type { IssueGroup } from "@/lib/domain/issueGroups";
import { openExternal } from "@/lib/platform";
import { useNavigation } from "@/stores/navigation";

export interface IssueTableProps {
  groups: IssueGroup[];
  repo: string;
  selected: Set<number>;
  onSelectedChange: (selected: Set<number>) => void;
}

export function IssueTable({ groups, repo, selected, onSelectedChange }: IssueTableProps) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const navigate = useNavigation((s) => s.navigate);

  const toggle = (number: number) => {
    const next = new Set(selected);
    if (next.has(number)) next.delete(number);
    else next.add(number);
    onSelectedChange(next);
  };
  const toggleGroup = (key: string) => {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setCollapsed(next);
  };

  return (
    <>
      <div
        role="row"
        className="flex h-[34px] shrink-0 items-center gap-3 border-b border-line px-3.5 text-tiny font-semibold tracking-[0.06em] text-tx3 uppercase"
      >
        <span className="w-3.5" />
        <span className="w-[15px]" />
        <span className="w-[38px]">N°</span>
        <span className="grow">Titre</span>
        <span className="w-[160px]">Labels</span>
        <span className="w-[62px] text-right">Version</span>
        <span className="w-[112px] text-right">Agent CTO</span>
        <span className="w-[72px] text-right">PR</span>
        <span className="w-[26px]" />
      </div>

      <div role="table" aria-label="Issues" className="min-h-0 grow overflow-y-auto">
        {groups.map((group) => {
          const key = group.version ?? "";
          const isCollapsed = collapsed.has(key);
          return (
            <div key={key || "none"} role="rowgroup">
              <button
                type="button"
                onClick={() => toggleGroup(key)}
                aria-expanded={!isCollapsed}
                className="flex h-10 w-full items-center gap-[11px] border-b border-line bg-p2 px-3.5 text-left hover:bg-p3"
              >
                <span
                  className={cn("flex text-tx3 transition-transform", isCollapsed && "-rotate-90")}
                >
                  <Icon name="chevronDown" size={14} />
                </span>
                {group.version ? (
                  <VersionBadge version={group.version} stage={group.stage ?? undefined} />
                ) : (
                  <Badge tone="neutral">—</Badge>
                )}
                <span className="text-body-sm text-tx2">
                  {group.label} · {group.open} ouverte{group.open > 1 ? "s" : ""} sur {group.total}
                </span>
                <span className="grow" />
                <ProgressBar
                  value={group.progress}
                  tone={group.stage === "shipped" ? "green" : "clay"}
                  height={4}
                  className="w-[110px]"
                  label={`Avancement ${group.version ?? "sans version"}`}
                />
                <span className="w-[34px] text-right mono text-tiny text-tx3">
                  {formatPercent(group.progress)}
                </span>
              </button>

              {!isCollapsed &&
                group.rows.map((issue) => (
                  <IssueLine
                    key={issue.number}
                    issue={issue}
                    group={group}
                    checked={selected.has(issue.number)}
                    onToggle={() => toggle(issue.number)}
                    onOpen={() => navigate({ id: "issue", number: issue.number })}
                  />
                ))}
            </div>
          );
        })}
      </div>

      {selected.size > 0 && (
        <FooterBar
          height={44}
          hint="Actions groupées : arrivent avec l'écriture sur GitHub (0.1.2)"
        >
          <div className="order-first flex items-center gap-2.5">
            <Badge tone="clay">
              {selected.size} sélectionnée{selected.size > 1 ? "s" : ""}
            </Badge>
            <Button size="sm" icon="tag" disabled>
              Changer de version…
            </Button>
            <Button size="sm" icon="user" disabled>
              Assigner
            </Button>
            <Button size="sm" variant="ghost" icon="close" disabled>
              Fermer
            </Button>
          </div>
          <Button
            size="sm"
            variant="ghost"
            icon="github"
            onClick={() =>
              [...selected]
                .slice(0, 5)
                .forEach((n) => openExternal(`https://github.com/${repo}/issues/${n}`))
            }
          >
            Ouvrir sur GitHub
          </Button>
          <Button size="sm" variant="quiet" onClick={() => onSelectedChange(new Set())}>
            Désélectionner
          </Button>
        </FooterBar>
      )}
    </>
  );
}

function IssueLine({
  issue,
  group,
  checked,
  onToggle,
  onOpen,
}: {
  issue: IssueRow;
  group: IssueGroup;
  checked: boolean;
  onToggle: () => void;
  onOpen: () => void;
}) {
  const extraLabels = issue.labels.slice(0, issue.issueType ? 1 : 2);
  const hidden = issue.labels.length - extraLabels.length;
  return (
    <div
      role="row"
      onClick={onOpen}
      className={cn(
        "flex h-[46px] cursor-pointer items-center gap-3 border-b border-line px-3.5 hover:bg-p2",
        checked && "bg-p2",
      )}
    >
      <input
        type="checkbox"
        aria-label={`Sélectionner #${issue.number}`}
        checked={checked}
        onChange={onToggle}
        onClick={(e) => e.stopPropagation()}
        className="size-3.5 shrink-0 cursor-pointer accent-clay"
      />
      <IssueStateIcon status={issue.status} />
      <span className="w-[38px] shrink-0 mono text-caption text-tx3">{issue.number}</span>
      <span
        className={cn("min-w-0 grow truncate text-body", issue.status === "done" && "text-tx2")}
      >
        {issue.title}
      </span>
      <div className="flex w-[160px] shrink-0 items-center gap-[5px] overflow-hidden">
        {issue.issueType && <TypeBadge type={issue.issueType} />}
        {extraLabels.map((l) => (
          <Badge key={l} tone="muted" className="max-w-[90px] truncate">
            {l}
          </Badge>
        ))}
        {hidden > 0 && <span className="text-caption text-tx3">+{hidden}</span>}
      </div>
      <span className="flex w-[62px] shrink-0 justify-end">
        {issue.version ? (
          <VersionBadge version={issue.version} stage={group.stage ?? undefined} />
        ) : (
          <span className="text-caption text-tx3">—</span>
        )}
      </span>
      <span className="flex w-[112px] shrink-0 justify-end text-tiny text-tx3">
        {issue.flags.includes("incomplete-description") ? (
          <span className="inline-flex items-center gap-1 text-amber">
            <Icon name="warning" size={12} /> à compléter
          </span>
        ) : (
          "—"
        )}
      </span>
      <span className="flex w-[72px] shrink-0 justify-end text-caption text-tx3">
        {issue.linkedPr ? `#${issue.linkedPr}` : "—"}
      </span>
      <span className="flex w-[26px] shrink-0 justify-end">
        {issue.assignee && (
          <Avatar
            size={21}
            tone={issue.assignee.isSelf ? "clay" : "neutral"}
            initials={initialsOf(issue.assignee.name ?? issue.assignee.login)}
            src={issue.assignee.avatarUrl}
            title={issue.assignee.login}
          />
        )}
      </span>
    </div>
  );
}
