/**
 * Détail d'une issue (maquette Issue-Detail.dc.html) : description rendue, état de l'agent
 * CTO et ses questions, propriétés (version, labels, assigné, branche, PR, estimation).
 *
 * Lancer l'agent développeur arrive en 0.2.0 ; le bouton montre déjà la branche qu'il créera.
 */
import { useState } from "react";
import type { IssueDetail } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { AgentQuestion } from "@/components/domain/AgentQuestion";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { Markdown } from "@/components/domain/Markdown";
import { EmptyValue, PropertyList, PropertyRow } from "@/components/domain/Panels";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { Spinner } from "@/components/ui/Spinner";
import { formatAgo } from "@/lib/domain/format";
import { branchName } from "@/lib/domain/issues";
import { openExternal } from "@/lib/platform";
import {
  useAgentLatestRun,
  useAnswerCto,
  useCurrentProject,
  useIssue,
  usePullRequests,
  useRejectItems,
  useStartAgent,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
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
  const fromCto = issue.flags.includes("cto-reviewed");
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
          <CtoBanner issue={issue} />
          <div className="mt-4">
            {issue.body.trim() ? (
              fromCto ? (
                <div className="rounded-[10px] border border-green-line bg-p2 px-4 py-3.5">
                  <div className="mb-2 flex items-center gap-[7px] text-caption font-medium text-green">
                    <Icon name="plus" size={13} /> Rédigé avec l'agent CTO
                  </div>
                  <Markdown source={issue.body} />
                </div>
              ) : (
                <Markdown source={issue.body} />
              )
            ) : (
              <p className="text-body-sm text-tx3">
                Pas encore de description. L'agent CTO peut en proposer une à partir du titre et du code.
              </p>
            )}
          </div>
        </div>
      </div>

      <aside className="flex w-[352px] shrink-0 flex-col gap-4 overflow-y-auto border-l border-line bg-panel px-4 py-3">
        <Properties issue={issue} />
        <CtoCard issue={issue} />
        <div className="grow" />
        <div className="flex flex-col gap-2">
          <Button variant="success" size="tall" icon="branch" disabled title="Arrive avec l'agent développeur (version 0.2.0)">
            Lancer l'agent développeur
          </Button>
          <p className="text-center text-tiny leading-[1.55] text-tx3">
            Créera <span className="mono">{branchName(issue.issueType, issue.number, issue.title)}</span> et une PR
            en brouillon (version 0.2.0).
          </p>
        </div>
      </aside>
    </div>
  );
}

/** Où en est l'agent CTO sur cette issue, et comment l'y envoyer. */
function CtoBanner({ issue }: { issue: IssueDetail }) {
  const navigate = useNavigation((s) => s.navigate);
  const start = useStartAgent();
  const item = useAgentLatestRun("cto").data?.items.find((i) => i.number === issue.number);
  const n = issue.questions.length;
  const queued = item?.state === "queued";
  const running = issue.activeAgent === "cto" || item?.state === "running";
  const ready = item?.state === "ready";
  const treated = issue.flags.includes("cto-reviewed");
  if (issue.status === "done" && !treated) return null;

  const agent = <span className="text-tx">agent CTO</span>;
  const text = running ? (
    <>L'{agent} travaille sur cette issue.</>
  ) : queued ? (
    <>En file chez l'{agent}.</>
  ) : n > 0 ? (
    <>
      L'{agent} a proposé une description —{" "}
      <span className="text-amber">
        {n} question{n > 1 ? "s" : ""}
      </span>{" "}
      attend{n > 1 ? "ent" : ""} votre réponse.
    </>
  ) : ready ? (
    <>L'{agent} a proposé une description : elle attend votre relecture.</>
  ) : treated ? (
    <>Description enrichie et validée avec l'{agent}.</>
  ) : (
    <>Pas encore passée par l'{agent}.</>
  );
  const followUp = running || queued || ready || n > 0 || treated;

  return (
    <>
      <div className="flex items-center gap-[9px] rounded-field border border-blue-line bg-blue-d px-3 py-[9px]">
        <AgentBadge agent="cto" size={22} />
        <span className="text-body-sm text-tx2">{text}</span>
        <div className="grow" />
        {followUp ? (
          <Button size="sm" icon="layers" onClick={() => navigate({ id: "agent", kind: "cto" })}>
            {ready ? "Relire la proposition" : "Voir l'agent"}
          </Button>
        ) : (
          <Button
            size="sm"
            icon="sparkle"
            disabled={start.isPending}
            onClick={() => start.mutate({ kind: "cto", input: { issues: [issue.number] } })}
          >
            {start.isPending && <Spinner />}
            Envoyer à l'agent CTO
          </Button>
        )}
      </div>
      {start.isError && (
        <Callout tone="danger" className="mt-2">
          {start.error.message}
        </Callout>
      )}
    </>
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
          <>
            <Badge tone="muted">
              {issue.estimate}
              {ESTIMATE_HOURS[issue.estimate] ? ` · ${ESTIMATE_HOURS[issue.estimate]}` : ""}
            </Badge>
            {issue.flags.includes("cto-reviewed") && <span className="text-tiny text-tx3">proposée par le CTO</span>}
          </>
        ) : (
          <EmptyValue />
        )}
      </PropertyRow>
    </PropertyList>
  );
}

/** Questions de l'agent CTO : réponses renvoyées dans sa session. */
function CtoCard({ issue }: { issue: IssueDetail }) {
  const answer = useAnswerCto(issue.number);
  const run = useAgentLatestRun("cto").data;
  const reject = useRejectItems(run?.run ?? "");
  const [answers, setAnswers] = useState<Record<string, string>>({});
  if (issue.questions.length === 0) return null;

  const typed = issue.questions.filter((q) => answers[q.id]?.trim());
  return (
    <Card className="p-3.5">
      <div className="flex items-center gap-2">
        <AgentBadge agent="cto" size={22} />
        <span className="grow text-body-sm font-semibold">Agent CTO</span>
        <Badge tone="amber">
          {issue.questions.length} question{issue.questions.length > 1 ? "s" : ""}
        </Badge>
      </div>
      <div className="mt-1">
        {issue.questions.map((q) => (
          <AgentQuestion
            key={q.id}
            question={q}
            value={answers[q.id] ?? ""}
            onChange={(v) => setAnswers((a) => ({ ...a, [q.id]: v }))}
          />
        ))}
      </div>
      {(answer.isError || reject.isError) && (
        <Callout tone="danger" className="mt-3">
          {(answer.error ?? reject.error)?.message}
        </Callout>
      )}
      <div className="mt-3 flex gap-2">
        <Button
          variant="primary"
          size="sm"
          icon="sparkle"
          disabled={typed.length === 0 || answer.isPending}
          onClick={() => answer.mutate(typed.map((q) => ({ questionId: q.id, answer: answers[q.id]!.trim() })))}
        >
          {answer.isPending && <Spinner />}
          Renvoyer à l'agent
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={!run || reject.isPending}
          title="Passer l'issue sans rien écrire"
          onClick={() => reject.mutate([issue.number])}
        >
          Ignorer
        </Button>
      </div>
    </Card>
  );
}
