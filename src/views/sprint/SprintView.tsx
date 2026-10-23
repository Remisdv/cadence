/**
 * Sprint en cours (maquette Main.dc.html) : KPI de la version en cours, tableau des issues
 * par état, burndown et ce qui attend l'utilisateur.
 *
 * Tout vient de `version_detail` (version en cours) et des revues en attente. Une carte
 * ouvre le détail de l'issue.
 */
import type { ReactNode } from "react";
import type { VersionReport } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { Burndown } from "@/components/domain/Burndown";
import { KanbanColumn } from "@/components/domain/IssueCard";
import { StatCard } from "@/components/domain/Stats";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { cn } from "@/lib/cn";
import { formatDecimal, formatPercent, formatRange } from "@/lib/domain/format";
import {
  daysLeftLabel,
  delayLabel,
  gainedToday,
  pullStats,
  reviewsToDo,
  sprintBoard,
  sprintTodos,
} from "@/lib/domain/sprint";
import { usePendingReviews, useVersionDetail } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";

export default function SprintView() {
  const report = useVersionDetail(null, { live: true });
  const openDialog = useUiStore((s) => s.openDialog);
  const v = report.data?.detail.version;

  return (
    <Page
      title={v ? `Sprint ${v.title}${v.name ? ` — ${v.name}` : ""}` : "Sprint en cours"}
      subtitle={
        report.data && v
          ? [
              v.kind === "sprint" ? formatRange(v.startOn, v.dueOn) : "palier · sans date",
              v.kind === "sprint" && daysLeftLabel(report.data.detail.daysLeft),
              `milestone #${v.milestoneNumber}`,
            ]
              .filter(Boolean)
              .join(" · ")
          : undefined
      }
      actions={
        <>
          <Button
            icon="tag"
            disabled={!report.data?.current}
            onClick={() => openDialog("version-close")}
          >
            Clôturer la version
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openDialog("issue-create")}>
            Nouvelle issue
          </Button>
        </>
      }
      className="flex flex-col"
    >
      {report.isPending ? (
        <LoadingState label="Lecture du sprint…" />
      ) : report.isError ? (
        <ErrorState error={report.error} />
      ) : (
        <Sprint report={report.data} />
      )}
    </Page>
  );
}

function Sprint({ report }: { report: VersionReport }) {
  const navigate = useNavigation((s) => s.navigate);
  const reviews = usePendingReviews({ live: true }).data ?? [];

  const { detail } = report;
  const issues = report.issues;
  const total = issues.length || detail.version.openIssues + detail.version.closedIssues;
  const closed = issues.length
    ? issues.filter((i) => i.status === "done").length
    : detail.version.closedIssues;
  const prs = pullStats(report.pulls);
  const toReview = reviewsToDo(reviews);
  const gain = gainedToday(detail.burndown, total);
  const open = (number: number) => navigate({ id: "issue", number });

  return (
    <div className="flex min-h-full flex-col gap-3.5">
      <div className="flex gap-3">
        <StatCard
          label="Portée"
          value={total}
          hint={`issues · ${closed} fermée${closed > 1 ? "s" : ""}`}
        />
        <StatCard
          label="Avancement"
          value={formatPercent(detail.progress)}
          hint={detail.burndown.length > 1 ? `+${gain} aujourd'hui` : undefined}
          tone="clay"
        />
        <StatCard
          label="PR ouvertes"
          value={prs.open}
          hint={`${prs.merged} fusionnée${prs.merged > 1 ? "s" : ""}`}
          tone="green"
        />
        <StatCard
          label="Revues à faire"
          value={toReview.count}
          hint={toReview.oldest ? `la plus ancienne : ${toReview.oldest}` : "rien en attente"}
          tone="amber"
        />
        <StatCard
          label="Rythme"
          value={formatDecimal(detail.velocity)}
          hint="issues fermées / jour"
          tone="blue"
        />
      </div>

      <div className="flex min-h-0 grow gap-3.5">
        <Card className="flex min-h-[420px] min-w-0 grow flex-col px-[15px] py-[13px]">
          <CardHeader
            title="Tableau du sprint"
            actions={
              <>
                <Badge tone="neutral">Groupé par état</Badge>
                <TextLink onClick={() => navigate({ id: "issues" })}>
                  Voir toutes les issues
                </TextLink>
              </>
            }
          />
          {issues.length === 0 ? (
            <EmptyState icon="issue" title="Aucune issue dans ce sprint">
              Les issues rattachées au milestone {detail.version.title} apparaissent ici, rangées
              par état (labels status/*).
            </EmptyState>
          ) : (
            <div className="mt-[13px] flex min-h-0 grow items-start gap-[11px] overflow-y-auto">
              {sprintBoard(issues).map((col) => (
                <KanbanColumn
                  key={col.status}
                  status={col.status}
                  issues={col.issues}
                  total={col.total}
                  onOpen={open}
                />
              ))}
            </div>
          )}
        </Card>

        <div className="flex w-[330px] shrink-0 flex-col gap-3">
          <BurndownCard report={report} scope={total} />
          <TodosCard />
        </div>
      </div>
    </div>
  );
}

function BurndownCard({ report, scope }: { report: VersionReport; scope: number }) {
  const { detail, sprintDays } = report;
  const { startOn, dueOn } = detail.version;
  const delay = delayLabel(detail.delayDays);
  const tones = { amber: "text-amber", green: "text-green", tx3: "text-tx3" };
  return (
    <Card className="px-[15px] py-3.5">
      <div className="flex items-center gap-2">
        <h3 className="text-body">Burndown</h3>
        <div className="grow" />
        {detail.burndown.length > 0 && (
          <span className={cn("text-tiny", tones[delay.tone])}>{delay.text}</span>
        )}
      </div>
      <div className="mt-3">
        {sprintDays !== null && startOn && dueOn && detail.burndown.length > 0 ? (
          <Burndown
            points={detail.burndown}
            scope={scope}
            start={startOn}
            end={dueOn}
            days={sprintDays + 1}
            label={`Burndown du sprint ${detail.version.title}`}
          />
        ) : (
          <p className="text-caption leading-[1.6] text-tx3">
            {sprintDays === null
              ? "Pas de burndown pour un palier : il n'a pas d'échéance."
              : "Le burndown commence au premier jour du sprint."}
          </p>
        )}
      </div>
    </Card>
  );
}

function TodosCard() {
  const navigate = useNavigation((s) => s.navigate);
  const todos = sprintTodos(usePendingReviews({ live: true }).data ?? []);
  if (todos.length === 0) {
    return (
      <Card className="px-[15px] py-[13px]">
        <div className="flex items-center gap-[7px] text-body-sm font-medium text-green">
          <Icon name="check" size={15} /> Rien ne vous attend
        </div>
      </Card>
    );
  }
  return (
    <Card className="px-[15px] py-[13px]">
      <div className="flex items-center gap-[7px] text-body-sm font-medium text-amber">
        <Icon name="warning" size={15} />
        {todos.length} point{todos.length > 1 ? "s" : ""} à traiter
      </div>
      <div className="mt-2.5 flex flex-col gap-2">
        {todos.map((todo) => (
          <button
            key={todo.pr}
            type="button"
            onClick={() => navigate({ id: "reviews", pr: todo.pr })}
            className="flex items-center gap-2 text-left text-body-sm text-tx2 hover:text-tx"
          >
            <Icon name="pullRequest" size={14} />
            {todo.text}
          </button>
        ))}
      </div>
    </Card>
  );
}

function TextLink({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" onClick={onClick} className="text-small text-clay hover:text-clay-hover">
      {children}
    </button>
  );
}
