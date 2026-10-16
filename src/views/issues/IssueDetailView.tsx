/**
 * Détail d'une issue (maquette Issue-Detail.dc.html) : description rendue, propriétés
 * (version, labels, assigné, branche, PR, estimation). Les boutons « Préparer avec l'IA » et
 * « Implémenter » reviendront comme actions du plugin Gestion de projet.
 */
import type { IssueDetail } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { Markdown } from "@/components/domain/Markdown";
import { EmptyValue, PropertyList, PropertyRow } from "@/components/domain/Panels";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { formatAgo } from "@/lib/domain/format";
import { openExternal } from "@/lib/platform";
import { useCurrentProject, useIssue, usePullRequests } from "@/lib/query/queries";
import { useUiStore } from "@/stores/ui";

const ESTIMATE_HOURS: Record<string, string> = { S: "~1 h", M: "~4 h", L: "~1 j", XL: "plusieurs jours" };

export default function IssueDetailView({ view }: ViewProps<"issue">) {
  const project = useCurrentProject().data;
  const issue = useIssue(view.number);
  const openDialog = useUiStore((s) => s.openDialog);

  return (
    <Page
      title="Issues"
      subtitle={project ? `${project.repo.fullName} · #${view.number}` : `#${view.number}`}
      padding="none"
      actions={
        <>
          <Button
            icon="github"
            disabled={!issue.data}
            onClick={() => issue.data && openExternal(issue.data.htmlUrl)}
          >
            Ouvrir sur GitHub
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openDialog("issue-create")}>
            Nouvelle issue
          </Button>
        </>
      }
    >
      {issue.isPending ? (
        <LoadingState label={`Lecture de #${view.number}…`} />
      ) : issue.isError ? (
        <ErrorState error={issue.error} />
      ) : (
        <Detail issue={issue.data} />
      )}
    </Page>
  );
}

function Detail({ issue }: { issue: IssueDetail }) {
  const closed = issue.status === "done";
  const prepared = issue.flags.includes("cto-reviewed");
  return (
    <div className="flex min-w-0 grow">
      <div className="flex min-w-0 grow flex-col overflow-hidden">
        <div className="shrink-0 border-b border-line px-6 pt-5 pb-4">
          <div className="flex items-center gap-[9px]">
            <Badge tone={closed ? "purple" : "green"} icon={closed ? "check" : "sprint"}>
              {closed ? "Fermée" : "Ouverte"}
            </Badge>
            {issue.version && <VersionBadge version={issue.version} />}
            {issue.issueType && <TypeBadge type={issue.issueType} />}
            <span className="mono text-small text-tx3">#{issue.number}</span>
            <div className="grow" />
            <span className="flex items-center gap-1.5 text-caption text-tx3">
              ouverte {formatAgo(issue.createdAt)}
              {issue.author && (
                <>
                  par
                  <Avatar
                    size={19}
                    tone={issue.author.isSelf ? "clay" : "neutral"}
                    initials={initialsOf(issue.author.name ?? issue.author.login)}
                    src={issue.author.avatarUrl}
                    title={issue.author.login}
                  />
                </>
              )}
            </span>
          </div>
          <h1 className="mt-[11px] font-display text-[21px] leading-[1.3] font-semibold">{issue.title}</h1>
        </div>

        <div className="min-h-0 grow overflow-y-auto px-6 py-[18px]">
          <div>
            {issue.body.trim() ? (
              prepared ? (
                <div className="rounded-[10px] border border-green-line bg-p2 px-4 py-3.5">
                  <div className="mb-2 flex items-center gap-[7px] text-caption font-medium text-green">
                    <Icon name="sparkle" size={13} /> Préparée avec l'IA
                  </div>
                  <Markdown source={issue.body} />
                </div>
              ) : (
                <Markdown source={issue.body} />
              )
            ) : (
              <p className="text-body-sm text-tx3">
                Pas encore de description.
              </p>
            )}
          </div>
        </div>
      </div>

      <aside className="flex w-[352px] shrink-0 flex-col gap-4 overflow-y-auto border-l border-line bg-panel px-4 py-3">
        <Properties issue={issue} />
      </aside>
    </div>
  );
}

function Properties({ issue }: { issue: IssueDetail }) {
  const prs = usePullRequests().data ?? [];
  const pr = prs.find((p) => p.linkedIssue === issue.number && (p.state === "open" || p.state === "draft"))
    ?? prs.find((p) => p.linkedIssue === issue.number);
  return (
    <PropertyList>
      <PropertyRow label="Version">
        {issue.version ? <VersionBadge version={issue.version} /> : <EmptyValue />}
      </PropertyRow>
      <PropertyRow label="Labels">
        {issue.issueType && <TypeBadge type={issue.issueType} />}
        {issue.labels.map((l) => (
          <Badge key={l} tone="muted">
            {l}
          </Badge>
        ))}
        {!issue.issueType && issue.labels.length === 0 && <EmptyValue />}
      </PropertyRow>
      <PropertyRow label="Assigné">
        {issue.assignee ? (
          <>
            <Avatar
              size={20}
              tone={issue.assignee.isSelf ? "clay" : "neutral"}
              initials={initialsOf(issue.assignee.name ?? issue.assignee.login)}
              src={issue.assignee.avatarUrl}
            />
            <span className="text-body-sm">{issue.assignee.isSelf ? "vous" : issue.assignee.login}</span>
          </>
        ) : (
          <EmptyValue>personne</EmptyValue>
        )}
      </PropertyRow>
      <PropertyRow label="Branche">{pr ? <span className="truncate mono text-small text-tx2">{pr.branch}</span> : <EmptyValue />}</PropertyRow>
      <PropertyRow label="PR liée">{pr ? <span className="mono text-small text-tx2">#{pr.number}</span> : <EmptyValue />}</PropertyRow>
      <PropertyRow label="Estimation">
        {issue.estimate ? (
          <Badge tone="muted">
            {issue.estimate}
            {ESTIMATE_HOURS[issue.estimate] ? ` · ${ESTIMATE_HOURS[issue.estimate]}` : ""}
          </Badge>
        ) : (
          <EmptyValue />
        )}
      </PropertyRow>
    </PropertyList>
  );
}
