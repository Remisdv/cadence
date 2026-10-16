/**
 * Détail d'une version (maquette Version-Detail.dc.html) : KPI, portée (issues de la
 * version), progression par état, burndown d'un sprint, aperçu des notes de version et
 * conditions de clôture. Sans paramètre, la version en cours.
 *
 * Les notes définitives sont générées par GitHub à la clôture (brouillon de release) ;
 * l'onglet « Déploiement » reviendra avec l'action « Préparer la release » du plugin.
 */
import { useState, type ReactNode } from "react";
import type { PrRow, VersionReport } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { Burndown } from "@/components/domain/Burndown";
import { VersionBadge } from "@/components/domain/Badges";
import { PillTabs } from "@/components/domain/Filters";
import { PrStateIcon } from "@/components/domain/Inline";
import { IssueLine } from "@/components/domain/IssueLine";
import { TitledCard } from "@/components/domain/Panels";
import { MeterList, StatStrip, type StatStripItem } from "@/components/domain/Stats";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { cn } from "@/lib/cn";
import {
  formatAgo,
  formatDay,
  formatDecimal,
  formatPercent,
  formatRange,
} from "@/lib/domain/format";
import { prStateMeta } from "@/lib/domain/issues";
import { themesOf } from "@/lib/domain/roadmap";
import {
  closeChecks,
  delayLabel,
  pullStats,
  releaseNotes,
  requiredPace,
  statusMeters,
} from "@/lib/domain/sprint";
import { useVersionDetail } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";

type Tab = "scope" | "notes" | "deploy" | "history";

const tabs: { value: Tab; label: string }[] = [
  { value: "scope", label: "Portée" },
  { value: "notes", label: "Notes de version" },
  { value: "deploy", label: "Déploiement" },
  { value: "history", label: "Historique" },
];

export default function VersionView({ view }: ViewProps<"version">) {
  const report = useVersionDetail(view.version ?? null, { live: true });
  const openDialog = useUiStore((s) => s.openDialog);
  const [tab, setTab] = useState<Tab>("scope");
  const r = report.data;
  const v = r?.detail.version;
  const total = r ? Math.max(r.issues.length, v!.openIssues + v!.closedIssues) : 0;

  return (
    <Page
      title={
        v
          ? `Version ${v.title}${v.name ? ` — ${v.name}` : ""}`
          : `Version ${view.version ?? "en cours"}`
      }
      subtitle={
        v
          ? [
              `milestone #${v.milestoneNumber}`,
              v.kind === "sprint" ? formatRange(v.startOn, v.dueOn) : "palier · sans date",
              `${total} issue${total > 1 ? "s" : ""}`,
            ]
              .filter(Boolean)
              .join(" · ")
          : undefined
      }
      actions={
        <>
          <Button icon="book" disabled={!r} onClick={() => setTab("notes")}>
            Notes de version
          </Button>
          {r?.current && (
            <Button variant="primary" icon="tag" onClick={() => openDialog("version-close")}>
              Clôturer la version
            </Button>
          )}
        </>
      }
      className="flex flex-col"
    >
      {report.isPending ? (
        <LoadingState label="Lecture de la version…" />
      ) : report.isError ? (
        <ErrorState error={report.error} />
      ) : (
        <Detail report={report.data} tab={tab} onTab={setTab} />
      )}
    </Page>
  );
}

function Detail({
  report,
  tab,
  onTab,
}: {
  report: VersionReport;
  tab: Tab;
  onTab: (t: Tab) => void;
}) {
  const navigate = useNavigation((s) => s.navigate);
  const { detail } = report;
  const v = detail.version;
  const shipped = v.state === "closed";
  const issues = report.issues;
  // Sans issue en cache (version ancienne), les compteurs du milestone font foi.
  const cached = issues.length > 0;
  const openCount = cached ? issues.filter((i) => i.status !== "done").length : v.openIssues;
  const closedCount = cached ? issues.length - openCount : v.closedIssues;
  const prs = pullStats(report.pulls);
  const remaining = openCount;

  const stats: StatStripItem[] = [
    {
      label: "Issues fermées",
      value: `${closedCount} / ${openCount + closedCount}`,
      suffix: formatPercent(detail.progress),
      tone: "clay",
    },
    {
      label: "PR fusionnées",
      value: prs.merged,
      suffix:
        prs.open > 0 ? `${prs.open} encore ouverte${prs.open > 1 ? "s" : ""}` : "aucune ouverte",
      tone: "green",
    },
    shipped || detail.daysLeft === null
      ? {
          label: "Échéance",
          value: v.dueOn ? formatDay(v.dueOn) : "—",
          suffix: shipped ? "livrée" : "palier sans date",
        }
      : {
          label: "Jours restants",
          value: Math.max(0, detail.daysLeft),
          suffix: report.sprintDays !== null ? `sur ${report.sprintDays}` : undefined,
        },
    shipped || requiredPace(remaining, detail.daysLeft) === null
      ? {
          label: "Rythme",
          value: v.kind === "sprint" ? formatDecimal(detail.velocity) : "—",
          suffix: v.kind === "sprint" ? "issues / jour" : "palier sans sprint",
          tone: "blue",
        }
      : {
          label: "Rythme requis",
          value: formatDecimal(requiredPace(remaining, detail.daysLeft)!),
          suffix: "issue / jour",
          tone: "amber",
        },
  ];

  return (
    <div className="flex min-h-full flex-col gap-[13px]">
      <div className="flex items-center gap-1.5">
        <PillTabs items={tabs} value={tab} onChange={onTab} aria-label="Sections de la version" />
        <div className="grow" />
        {report.previous && (
          <NeighbourLink onClick={() => navigate({ id: "version", version: report.previous! })}>
            <Badge tone="muted">Version précédente : {report.previous}</Badge>
          </NeighbourLink>
        )}
        {report.next && (
          <NeighbourLink onClick={() => navigate({ id: "version", version: report.next! })}>
            <Badge tone="neutral">Suivante : {report.next}</Badge>
          </NeighbourLink>
        )}
      </div>

      <StatStrip items={stats} />

      <div className="flex min-h-0 grow gap-[13px]">
        <div className="flex min-h-[420px] min-w-0 grow flex-col">
          {tab === "scope" && (
            <TitledCard
              title="Portée de la version"
              className="grow"
              actions={
                <>
                  <Badge tone="green">
                    {openCount} ouverte{openCount > 1 ? "s" : ""}
                  </Badge>
                  <Badge tone="purple">
                    {closedCount} fermée{closedCount > 1 ? "s" : ""}
                  </Badge>
                </>
              }
            >
              {issues.length === 0 ? (
                <ThemesOrEmpty description={v.description} />
              ) : (
                issues.map((issue) => (
                  <IssueLine
                    key={issue.number}
                    issue={issue}
                    onOpen={(number) => navigate({ id: "issue", number })}
                  />
                ))
              )}
            </TitledCard>
          )}
          {tab === "notes" && (
            <TitledCard
              title="Notes de version"
              className="grow"
              actions={<Badge tone="amber">aperçu</Badge>}
            >
              <div className="px-4 py-3.5">
                <ReleaseNotes report={report} />
                <p className="mt-3.5 text-tiny leading-[1.6] text-tx3">
                  Aperçu tiré des issues fermées, groupées par type. À la clôture, GitHub génère les
                  notes définitives dans un brouillon de release, jamais publié automatiquement.
                </p>
              </div>
            </TitledCard>
          )}
          {tab === "deploy" && (
            <Card className="grow">
              <EmptyState icon="rocket" title="Déploiement">
                {report.tag
                  ? `Le tag ${report.tag} a été posé à la clôture. `
                  : "Le tag de la version est posé à la clôture. "}
                Builds, artefacts et tests de vérification reviendront avec le plugin Gestion de
                projet.
              </EmptyState>
            </Card>
          )}
          {tab === "history" && (
            <TitledCard
              title="Pull requests de la version"
              subtitle={`${report.pulls.length}`}
              className="grow"
            >
              {report.pulls.length === 0 ? (
                <EmptyState icon="pullRequest" title="Aucune pull request">
                  Les PR rattachées au milestone, ou qui ferment une issue de la version,
                  apparaissent ici (PR ouvertes et 30 dernières fusionnées ou fermées).
                </EmptyState>
              ) : (
                report.pulls.map((pr) => (
                  <PullLine
                    key={pr.number}
                    pr={pr}
                    onOpen={() => navigate({ id: "reviews", pr: pr.number })}
                  />
                ))
              )}
            </TitledCard>
          )}
        </div>

        <div className="flex w-[340px] shrink-0 flex-col gap-[13px]">
          <Card className="px-[15px] py-3.5">
            <h3 className="text-body">Progression</h3>
            <MeterList
              className="mt-[13px]"
              items={
                cached
                  ? statusMeters(issues)
                  : [
                      {
                        label: "Fermées",
                        value: closedCount,
                        percent: detail.progress,
                        tone: "green",
                      },
                      {
                        label: "Ouvertes",
                        value: openCount,
                        percent: openCount > 0 ? 100 - detail.progress : 0,
                        tone: "neutral",
                      },
                    ]
              }
            />
          </Card>

          {report.sprintDays !== null && v.startOn && v.dueOn && detail.burndown.length > 0 && (
            <Card className="px-[15px] py-3.5">
              <div className="flex items-center gap-2">
                <h3 className="text-body">Burndown</h3>
                <div className="grow" />
                {!shipped && <DelayText delay={detail.delayDays} />}
              </div>
              <div className="mt-3">
                <Burndown
                  points={detail.burndown}
                  scope={Math.max(1, openCount + closedCount)}
                  start={v.startOn}
                  end={v.dueOn}
                  days={report.sprintDays + 1}
                  label={`Burndown de la version ${v.title}`}
                />
              </div>
            </Card>
          )}

          {tab !== "notes" && (
            <Card className="px-[15px] py-3.5">
              <div className="flex items-center gap-2">
                <h3 className="text-body">Notes de version</h3>
                <div className="grow" />
                <Badge tone="amber">aperçu</Badge>
              </div>
              <div className="mt-[11px]">
                <ReleaseNotes report={report} limit={2} />
              </div>
              <div className="mt-3 flex gap-2">
                <Button variant="ghost" icon="file" onClick={() => onTab("notes")}>
                  Tout voir
                </Button>
              </div>
            </Card>
          )}

          <CloseCard report={report} />
        </div>
      </div>
    </div>
  );
}

function ReleaseNotes({ report, limit }: { report: VersionReport; limit?: number }) {
  const sections = releaseNotes(report.issues);
  if (sections.length === 0) {
    return (
      <p className="text-small text-tx3">
        {report.issues.length === 0
          ? "Aucune issue de cette version dans le cache."
          : "Aucune issue fermée pour l'instant."}
      </p>
    );
  }
  return (
    <div className="text-small leading-[1.7] text-tx2">
      {sections.map((s, i) => (
        <div key={s.title} className={cn(i > 0 && "mt-2")}>
          <div className="font-semibold text-tx">{s.title}</div>
          {(limit ? s.items.slice(0, limit) : s.items).map((item) => (
            <div key={item}>· {item}</div>
          ))}
          {limit && s.items.length > limit && (
            <div className="text-tx3">· et {s.items.length - limit} autre(s)</div>
          )}
        </div>
      ))}
    </div>
  );
}

function CloseCard({ report }: { report: VersionReport }) {
  const v = report.detail.version;
  if (v.state === "closed") {
    return (
      <Card className="px-[15px] py-3.5">
        <div className="flex items-center gap-2">
          <span className="flex text-green">
            <Icon name="check" size={15} />
          </span>
          <span className="text-body-sm font-medium">Version livrée</span>
          <div className="grow" />
          {report.tag && <VersionBadge version={report.tag} tone="green" />}
        </div>
        <p className="mt-2.5 text-small leading-[1.6] text-tx2">
          Milestone fermé{v.dueOn ? ` (échéance : ${formatDay(v.dueOn)})` : ""}. Les issues restées
          ouvertes ont été traitées à la clôture.
        </p>
      </Card>
    );
  }
  const checks = closeChecks(report.issues, report.pulls);
  const blocked = checks.some((c) => !c.ok);
  return (
    <Card className="px-[15px] py-3.5">
      <div className="flex items-center gap-2">
        <span className={cn("flex", blocked ? "text-amber" : "text-green")}>
          <Icon name={blocked ? "warning" : "check"} size={15} />
        </span>
        <span className="text-body-sm font-medium">
          {!report.current ? "Avant la clôture" : blocked ? "Clôture bloquée" : "Prête à clôturer"}
        </span>
      </div>
      <div className="mt-2.5 flex flex-col gap-[7px] text-small text-tx2">
        {checks.map((c) => (
          <div key={c.text} className="flex gap-2">
            <span className={cn("flex", c.ok ? "text-green" : "text-red")}>
              <Icon name={c.ok ? "check" : "close"} size={13} />
            </span>
            {c.text}
          </div>
        ))}
      </div>
      <p className="mt-2.5 text-tiny leading-[1.6] text-tx3">
        {report.current
          ? blocked
            ? "Vous pouvez clôturer malgré tout : les issues restantes seront reportées sur la version suivante."
            : "Tout est prêt : la clôture pose le tag et prépare le brouillon de release."
          : "Seule la version en cours se clôture ; celle-ci le sera à son tour."}
      </p>
    </Card>
  );
}

function ThemesOrEmpty({ description }: { description: string | null }) {
  const themes = themesOf(description);
  if (themes.length === 0) {
    return (
      <EmptyState icon="issue" title="Aucune issue dans cette version">
        Les issues rattachées à ce milestone apparaissent ici.
      </EmptyState>
    );
  }
  return (
    <div className="px-3.5 py-3">
      <div className="text-tiny text-tx3">Thèmes du palier — pas encore d'issues</div>
      <ul className="mt-2 flex flex-col gap-1.5">
        {themes.map((t) => (
          <li key={t} className="flex items-start gap-2 text-body-sm text-tx2">
            <span className="mt-px flex text-tx3">
              <Icon name="sprint" size={12} />
            </span>
            {t}
          </li>
        ))}
      </ul>
    </div>
  );
}

function PullLine({ pr, onOpen }: { pr: PrRow; onOpen: () => void }) {
  const meta = prStateMeta[pr.state];
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex h-10 w-full items-center gap-[11px] border-b border-line px-3.5 text-left hover:bg-p2"
    >
      <PrStateIcon state={pr.state} size={14} />
      <span className="w-9 shrink-0 mono text-tiny text-tx3">{pr.number}</span>
      <span className="min-w-0 grow truncate text-body-sm">{pr.title}</span>
      <Badge tone={meta.tone}>{meta.label}</Badge>
      <span className="w-[72px] shrink-0 text-right text-tiny text-tx3">
        {formatAgo(pr.updatedAt)}
      </span>
    </button>
  );
}

function DelayText({ delay }: { delay: number }) {
  const d = delayLabel(delay);
  const tones = { amber: "text-amber", green: "text-green", tx3: "text-tx3" };
  return <span className={cn("text-tiny", tones[d.tone])}>{d.text}</span>;
}

function NeighbourLink({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" onClick={onClick} className="flex hover:opacity-80">
      {children}
    </button>
  );
}
