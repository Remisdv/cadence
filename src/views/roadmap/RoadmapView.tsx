/**
 * Roadmap (maquette Roadmap.dc.html) : une carte par version (milestone), livrées et en
 * cours en haut, sprints planifiés et paliers en bas. Lecture seule en 0.1.1.
 *
 * Le nom d'une version est la première ligne de la description de son milestone
 * (`# Cockpit Tauri`), les thèmes d'un palier en sont les éléments de liste.
 * Créer une version arrive en 0.1.2. « Proposer avec l'agent CEO » lance l'agent sur le
 * prochain palier (ou ouvre son écran), et un bandeau signale les propositions à relire.
 */
import type { ReactNode } from "react";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon, type IconName } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { VersionBadge } from "@/components/domain/Badges";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ProgressBar, type ProgressTone } from "@/components/ui/ProgressBar";
import { cn } from "@/lib/cn";
import { formatDaysLeft, formatPercent, formatRange } from "@/lib/domain/format";
import { buildRoadmap, daysUntil, periodOf, type RoadmapCard } from "@/lib/domain/roadmap";
import { ceoTarget, pendingProposals, themesLabel } from "@/lib/domain/ceo";
import { versionStageMeta, type VersionStage } from "@/lib/domain/versions";
import { useAgentLatestRun, useCurrentProject, useIssues, useStartAgent, useVersions } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";

const stageText: Record<VersionStage, string> = {
  shipped: "text-green",
  current: "text-clay",
  planned: "text-tx2",
  milestone: "text-blue",
  vision: "text-purple",
};

const stageIcon: Record<VersionStage, IconName> = {
  shipped: "check",
  current: "clock",
  planned: "calendar",
  milestone: "calendar",
  vision: "calendar",
};

const progressTone: Record<VersionStage, ProgressTone> = {
  shipped: "green",
  current: "clay",
  planned: "tx2",
  milestone: "blue",
  vision: "purple",
};

export default function RoadmapView() {
  const project = useCurrentProject().data;
  const versions = useVersions();
  const issues = useIssues({}, { live: true });
  const navigate = useNavigation((s) => s.navigate);
  const openDialog = useUiStore((s) => s.openDialog);

  const current = project?.currentVersion ?? null;
  const roadmap = buildRoadmap(versions.data ?? [], issues.data ?? [], current);
  const shippedCount = roadmap.shipped.length + roadmap.olderShipped;
  const vision = roadmap.planned.find((c) => c.stage === "vision");
  const period = periodOf(versions.data ?? []);
  const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;

  const subtitle = versions.data
    ? [
        plural(shippedCount, "version livrée", "versions livrées"),
        roadmap.current && "1 en cours",
        `${roadmap.planned.length} planifiée${roadmap.planned.length > 1 ? "s" : ""}`,
        vision && `un palier vers ${vision.version.title}`,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  const open = (title: string) => navigate({ id: "version", version: title });

  // Agent CEO : propositions à relire, ou lancement sur le prochain palier.
  const ceoRun = useAgentLatestRun("ceo").data ?? null;
  const startAgent = useStartAgent();
  const pending = pendingProposals(ceoRun);
  const ceoActive = ceoRun?.state === "running" || ceoRun?.state === "prepare";
  const ceoVersion = ceoTarget(versions.data ?? [], current);
  const ceoVersionName = versions.data?.find((v) => v.title === ceoRun?.version)?.name ?? null;
  const openCeo = () => navigate({ id: "agent", kind: "ceo" });
  const proposeWithCeo = () => {
    if (ceoActive || pending.length > 0 || !ceoVersion) return openCeo();
    startAgent.mutate({ kind: "ceo", input: { version: ceoVersion } }, { onSettled: openCeo });
  };

  return (
    <Page
      title="Roadmap"
      subtitle={subtitle}
      actions={
        <>
          <Button
            icon="sparkle"
            disabled={startAgent.isPending}
            onClick={proposeWithCeo}
            title={
              pending.length > 0 || ceoActive
                ? "Ouvrir les propositions de l'agent CEO"
                : ceoVersion
                  ? `Dériver les issues manquantes de ${ceoVersion}`
                  : "Aucun palier ouvert après la version en cours"
            }
          >
            Proposer avec l'agent CEO
          </Button>
          <Button variant="primary" icon="plus" disabled title="Arrive avec l'écriture sur GitHub (0.1.2)">
            Nouvelle version
          </Button>
        </>
      }
      className="flex flex-col gap-3.5"
    >
      <div className="flex shrink-0 items-center gap-2.5">
        {period && (
          <span className="inline-flex h-[30px] items-center gap-[7px] rounded-control border border-line bg-p2 px-[11px] text-body-sm text-tx2">
            <Icon name="calendar" size={14} /> {period}
          </span>
        )}
        <Badge tone="neutral">Une version = un milestone</Badge>
        <div className="grow" />
        {current && (
          <>
            <Button variant="quiet" size="sm" onClick={() => open(current)}>
              Ouvrir le détail de {current}
            </Button>
            <Button size="sm" icon="tag" onClick={() => openDialog("version-close")}>
              Clôturer {current}
            </Button>
          </>
        )}
      </div>

      {versions.isPending ? (
        <LoadingState label="Lecture des milestones…" />
      ) : versions.isError ? (
        <ErrorState error={versions.error} />
      ) : versions.data.length === 0 ? (
        <Card className="grow">
          <EmptyState icon="roadmap" title="Aucune version dans ce dépôt">
            Une version est un milestone GitHub au titre semver (0.1.0) : daté pour un sprint, sans
            date pour un palier. Sa description donne le nom de la version (« # Cockpit Tauri ») et,
            pour un palier, ses thèmes en liste.
          </EmptyState>
        </Card>
      ) : (
        <>
          <Lane
            title="Livré"
            count={shippedCount}
            hint={
              roadmap.olderShipped > 0
                ? `milestones fermés · ${roadmap.olderShipped} plus ancien${roadmap.olderShipped > 1 ? "s" : ""} masqué${roadmap.olderShipped > 1 ? "s" : ""}`
                : "milestones fermés"
            }
          >
            {roadmap.shipped.map((c) => (
              <VersionCard key={c.version.title} card={c} onOpen={() => open(c.version.title)} />
            ))}
            {roadmap.current && (
              <>
                {roadmap.shipped.length > 0 && <Arrow />}
                <VersionCard card={roadmap.current} onOpen={() => open(roadmap.current!.version.title)} />
              </>
            )}
            {roadmap.shipped.length === 0 && !roadmap.current && (
              <p className="text-caption text-tx3">Aucune version livrée pour l'instant.</p>
            )}
          </Lane>

          <Lane title="Planifié" count={roadmap.planned.length} hint="milestones ouverts, sans date pour le palier">
            {roadmap.planned.length === 0 ? (
              <p className="text-caption text-tx3">Aucune version planifiée.</p>
            ) : (
              roadmap.planned.map((c, i) => {
                const prev = roadmap.planned[i - 1];
                const toPalier = prev?.stage === "planned" && c.stage !== "planned";
                return (
                  <div key={c.version.title} className="flex shrink-0 gap-3">
                    {toPalier && <Arrow />}
                    <VersionCard card={c} onOpen={() => open(c.version.title)} />
                  </div>
                );
              })
            )}
          </Lane>

          {pending.length > 0 && ceoRun && (
            <CeoBanner
              version={ceoRun.version}
              titles={pending.map((p) => p.title)}
              themes={ceoVersionName ? `de « ${ceoVersionName} »` : themesLabel(pending)}
              onOpen={openCeo}
            />
          )}
        </>
      )}
    </Page>
  );
}

/**
 * Bandeau du bas : propositions de l'agent CEO en attente de relecture. `themes` complète
 * « dérivées des thèmes … » (nom du palier, ou liste des thèmes à défaut).
 */
function CeoBanner({
  version,
  titles,
  themes,
  onOpen,
}: {
  version: string | null;
  titles: string[];
  themes: string;
  onOpen: () => void;
}) {
  const n = titles.length;
  return (
    <section className="flex shrink-0 items-center gap-3.5 rounded-card border border-line bg-panel px-4 py-3.5">
      <AgentBadge agent="ceo" size={32} />
      <div className="min-w-0 grow">
        <div className="text-body">
          <span className="font-semibold">
            L'agent CEO propose {n} issue{n > 1 ? "s" : ""}
          </span>
          {version && ` pour la version ${version}`}
          {themes ? `, dérivée${n > 1 ? "s" : ""} des thèmes ${themes}.` : "."}
        </div>
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {titles.map((t) => (
            <Badge key={t} tone="muted">
              {t}
            </Badge>
          ))}
        </div>
      </div>
      <Button variant="purple" icon="review" className="h-[34px] gap-[7px] rounded-lg px-3.5" onClick={onOpen}>
        Examiner les propositions
      </Button>
    </section>
  );
}

function Lane({
  title,
  count,
  hint,
  children,
}: {
  title: string;
  count: number;
  hint: string;
  children: ReactNode;
}) {
  return (
    <section className="shrink-0">
      <div className="mb-2.5 flex items-center gap-[9px]">
        <span className="text-small font-semibold">{title}</span>
        <span className="mono text-tiny text-tx3">{count}</span>
        <span className="text-tiny text-tx3">{hint}</span>
      </div>
      <div className="flex gap-3 overflow-x-auto pb-1">{children}</div>
    </section>
  );
}

function Arrow() {
  return (
    <div className="flex shrink-0 items-center px-2 text-line2" aria-hidden>
      <Icon name="arrowRight" size={18} />
    </div>
  );
}

function VersionCard({ card, onOpen }: { card: RoadmapCard; onOpen: () => void }) {
  const { version, stage } = card;
  const days = stage === "current" ? daysUntil(version.dueOn) : null;
  const dated = version.kind === "sprint";
  return (
    <button
      type="button"
      onClick={onOpen}
      className={cn(
        "flex w-[262px] shrink-0 flex-col gap-[11px] rounded-card border bg-panel p-[15px] text-left hover:bg-p2",
        stage === "current" ? "border-clay-line" : "border-line",
      )}
    >
      <div className="flex w-full items-center gap-2">
        <VersionBadge version={version.title} stage={stage} />
        <span className={cn("text-tiny", stageText[stage])}>{versionStageMeta[stage].label}</span>
        <div className="grow" />
        <span className="flex text-tx3">
          <Icon name={stageIcon[stage]} size={14} />
        </span>
      </div>

      <div>
        <div className="text-body leading-[1.35] font-semibold">{version.name ?? `Version ${version.title}`}</div>
        <div className="mt-1 text-tiny text-tx3">
          {dated ? formatRange(version.startOn, version.dueOn) || "sans échéance" : "palier · sans date"}
        </div>
      </div>

      {card.items.length > 0 && (
        <ul className="flex flex-col gap-1.5">
          {card.items.map((item) => (
            <li
              key={item.text}
              className={cn("flex items-start gap-2 text-small leading-[1.45]", item.done ? "text-tx3" : "text-tx2")}
            >
              <span className={cn("mt-px flex shrink-0", item.done ? "text-green" : "text-tx3")}>
                <Icon name={item.done ? "check" : "sprint"} size={12} />
              </span>
              <span className={cn("line-clamp-2", item.done && "line-through decoration-line2")}>{item.text}</span>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-auto w-full border-t border-line pt-2.5">
        {dated || card.total > 0 ? (
          <>
            {dated && (
              <ProgressBar
                value={card.progress}
                tone={progressTone[stage]}
                height={4}
                label={`Avancement ${version.title}`}
              />
            )}
            <div className="mt-1.5 flex justify-between text-micro text-tx3">
              {dated && <span>{formatPercent(card.progress)}</span>}
              <span className={cn(!dated && "grow")}>
                {card.total} issue{card.total > 1 ? "s" : ""}
                {stage === "current" &&
                  days !== null &&
                  ` · ${days < 0 ? `en retard de ${-days} j` : formatDaysLeft(days)}`}
                {stage === "planned" && " rattachée" + (card.total > 1 ? "s" : "")}
              </span>
            </div>
          </>
        ) : (
          <div className="text-micro text-tx3">Thèmes seulement — pas encore d'issues</div>
        )}
      </div>
    </button>
  );
}
