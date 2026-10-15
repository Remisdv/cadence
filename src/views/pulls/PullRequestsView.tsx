/**
 * Pull requests (maquette Pull-Requests.dc.html) : PR ouvertes et récentes du dépôt, triées
 * par activité. Une ligne ouvre l'écran de revue de la PR.
 *
 * Le bandeau « l'agent développeur écrit la PR » arrive avec l'agent (0.2.0).
 */
import { useMemo, useState } from "react";
import type { PrRow } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { VersionBadge } from "@/components/domain/Badges";
import { CiStatus, Diffstat, InlineMeta, PrStateIcon } from "@/components/domain/Inline";
import { FilterChip } from "@/components/domain/Filters";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import {
  Menu,
  MenuCheckboxItem,
  MenuContent,
  MenuItem,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/Menu";
import { cn } from "@/lib/cn";
import { agents } from "@/lib/domain/agents";
import { prStateMeta } from "@/lib/domain/issues";
import {
  defaultPrFilter,
  isOpenPr,
  matchesPr,
  reviewSummary,
  type PrOriginFilter,
  type PrStateFilter,
  type PrViewFilter,
} from "@/lib/domain/prs";
import { useCurrentProject, usePullRequests, useVersions } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";

const stateLabels: Record<PrStateFilter, string> = {
  open: "ouvertes",
  merged: "fusionnées",
  closed: "fermées",
  all: "toutes",
};

const originLabels: Record<PrOriginFilter, string> = {
  all: "humain et agent",
  human: "humain",
  agent: "agent",
};

export default function PullRequestsView() {
  const project = useCurrentProject().data;
  const prs = usePullRequests({}, { live: true });
  const versions = useVersions().data ?? [];
  const openDialog = useUiStore((s) => s.openDialog);
  const navigate = useNavigation((s) => s.navigate);
  const [filter, setFilter] = useState<PrViewFilter>(defaultPrFilter);
  const set = (patch: Partial<PrViewFilter>) => setFilter((f) => ({ ...f, ...patch }));

  const all = useMemo(() => prs.data ?? [], [prs.data]);
  const visible = useMemo(() => all.filter((p) => matchesPr(p, filter)), [all, filter]);
  const current = project?.currentVersion ?? null;
  const open = all.filter(isOpenPr);
  const byAgent = open.filter((p) => p.agent !== null).length;
  const mergedOnCurrent = all.filter((p) => p.state === "merged" && p.version === current).length;
  const versionTitles = [...new Set(all.map((p) => p.version ?? ""))].sort();
  const stageOf = (title: string | null) => versions.find((v) => v.title === title)?.state;
  const filtered =
    filter.versions.length > 0 ||
    filter.state !== "open" ||
    filter.author !== "all" ||
    filter.origin !== "all";

  const subtitle = prs.data
    ? [
        `${open.length} ouverte${open.length > 1 ? "s" : ""}`,
        byAgent > 0 && `${byAgent} écrite${byAgent > 1 ? "s" : ""} par un agent`,
        current && `${mergedOnCurrent} fusionnée${mergedOnCurrent > 1 ? "s" : ""} sur ${current}`,
      ]
        .filter(Boolean)
        .join(" · ")
    : project && `dépôt ${project.repo.fullName}`;

  return (
    <Page
      title="Pull requests"
      subtitle={subtitle || undefined}
      actions={
        <>
          <Button
            icon="refresh"
            onClick={() => void prs.refetch()}
            disabled={prs.isFetching}
            title="Relit GitHub (au plus une fois par minute)"
          >
            Synchroniser
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openDialog("issue-create")}>
            Nouvelle issue
          </Button>
        </>
      }
      className="flex flex-col gap-3"
    >
      <div className="flex shrink-0 items-center gap-2">
        <Menu>
          <MenuTrigger asChild>
            <FilterChip
              icon="tag"
              label="Version"
              value={filter.versions.length > 0 ? filter.versions.map((v) => v || "sans version").join(", ") : undefined}
            />
          </MenuTrigger>
          <MenuContent>
            {versionTitles.map((v) => (
              <MenuCheckboxItem
                key={v || "none"}
                checked={filter.versions.includes(v)}
                onCheckedChange={(checked) =>
                  set({
                    versions: checked
                      ? [...filter.versions, v]
                      : filter.versions.filter((x) => x !== v),
                  })
                }
              >
                {v ? <span className="mono">{v}</span> : "Sans version"}
                {v === current && <span className="ml-auto text-caption text-tx3">en cours</span>}
              </MenuCheckboxItem>
            ))}
            {filter.versions.length > 0 && (
              <>
                <MenuSeparator />
                <MenuItem onSelect={() => set({ versions: [] })}>Toutes les versions</MenuItem>
              </>
            )}
          </MenuContent>
        </Menu>

        <RadioFilter
          icon="pullRequest"
          label="État"
          value={filter.state}
          defaultValue="open"
          labels={stateLabels}
          onChange={(state) => set({ state })}
        />
        <RadioFilter
          icon="user"
          label="Auteur"
          value={filter.author}
          defaultValue="all"
          labels={{ all: "tous", me: "moi" }}
          onChange={(author) => set({ author })}
        />
        <RadioFilter
          icon="bot"
          label="Origine"
          value={filter.origin}
          defaultValue="all"
          labels={originLabels}
          onChange={(origin) => set({ origin })}
        />

        <div className="grow" />
        {filtered && (
          <Button variant="quiet" size="sm" onClick={() => setFilter(defaultPrFilter)}>
            Réinitialiser
          </Button>
        )}
        <Badge tone="muted">Trié par activité</Badge>
      </div>

      <Card className="flex min-h-0 grow flex-col overflow-hidden">
        {prs.isPending ? (
          <LoadingState label="Lecture des pull requests sur GitHub…" />
        ) : prs.isError ? (
          <ErrorState error={prs.error} />
        ) : all.length === 0 ? (
          <EmptyState icon="pullRequest" title="Aucune pull request dans ce dépôt">
            Les PR ouvertes et les 30 dernières fusionnées ou fermées apparaîtront ici.
          </EmptyState>
        ) : visible.length === 0 ? (
          <EmptyState
            icon="search"
            title="Aucune pull request ne correspond aux filtres"
            action={
              <Button variant="ghost" onClick={() => setFilter(defaultPrFilter)}>
                Réinitialiser les filtres
              </Button>
            }
          />
        ) : (
          <>
            <div
              role="row"
              className="flex h-[34px] shrink-0 items-center gap-[13px] border-b border-line px-4 text-tiny font-semibold tracking-[0.06em] text-tx3 uppercase"
            >
              <span className="w-4" />
              <span className="grow">Pull request</span>
              <span className="w-[78px]">CI</span>
              <span className="w-24">Revues</span>
              <span className="w-14 text-right">Version</span>
              <span className="w-[26px]" />
            </div>
            <div role="table" aria-label="Pull requests" className="min-h-0 grow overflow-y-auto">
              {visible.map((pr) => (
                <PrLine
                  key={pr.number}
                  pr={pr}
                  stage={stageOf(pr.version) === "closed" ? "shipped" : undefined}
                  onOpen={() => navigate({ id: "reviews", pr: pr.number })}
                />
              ))}
            </div>
          </>
        )}
      </Card>
    </Page>
  );
}

function PrLine({
  pr,
  stage,
  onOpen,
}: {
  pr: PrRow;
  stage?: "shipped";
  onOpen: () => void;
}) {
  const review = reviewSummary(pr);
  const closesVerb = pr.state === "merged" ? "a fermé" : "ferme";
  return (
    <div
      role="row"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter") onOpen();
      }}
      className="flex cursor-pointer items-center gap-[13px] border-b border-line px-4 py-[13px] hover:bg-p2 focus-visible:bg-p2 focus-visible:outline-none"
    >
      <span className="mt-px">
        <PrStateIcon state={pr.state} />
      </span>
      <div className="min-w-0 grow">
        <div className="flex items-center gap-2">
          <span className="truncate text-body font-medium">{pr.title}</span>
          {pr.state !== "open" && <Badge tone={prStateMeta[pr.state].tone}>{prStateMeta[pr.state].label}</Badge>}
        </div>
        <div className="mt-[5px] flex min-w-0 items-center gap-[9px] text-caption text-tx3">
          <span className="mono">#{pr.number}</span>
          <span className="inline-flex min-w-0 items-center gap-1">
            <Icon name="branch" size={12} />
            <span className="truncate mono">{pr.branch}</span>
          </span>
          <span className="text-line2">→</span>
          <span className="mono">{pr.base}</span>
          {pr.agent && (
            <InlineMeta icon="bot" tone={agents[pr.agent].tone}>
              agent {pr.agent}
            </InlineMeta>
          )}
          {pr.linkedIssue && (
            <InlineMeta icon="link">
              {closesVerb} #{pr.linkedIssue}
            </InlineMeta>
          )}
          {pr.state !== "draft" && <Diffstat additions={pr.additions} deletions={pr.deletions} plain />}
        </div>
      </div>
      <span className="w-[78px] shrink-0">
        <CiStatus state={pr.ci} passed={pr.ciPassed} total={pr.ciTotal || undefined} />
      </span>
      <span className="w-24 shrink-0 text-caption text-tx2">
        {review ? (
          <span className={cn(review.tone === "amber" && "text-amber", review.tone === "red" && "text-red")}>
            {review.text}
          </span>
        ) : (
          "—"
        )}
      </span>
      <span className="flex w-14 shrink-0 justify-end">
        {pr.version ? (
          <VersionBadge version={pr.version} stage={stage} />
        ) : (
          <span className="text-caption text-tx3">—</span>
        )}
      </span>
      <span className="flex w-[26px] shrink-0 justify-end">
        {pr.agent ? (
          <AgentBadge agent={pr.agent} size={21} />
        ) : (
          <Avatar
            size={21}
            tone={pr.author.isSelf ? "clay" : "neutral"}
            initials={initialsOf(pr.author.name ?? pr.author.login)}
            src={pr.author.avatarUrl}
            title={pr.author.login}
          />
        )}
      </span>
    </div>
  );
}

/** Puce de filtre à choix unique ; neutre tant que la valeur par défaut est choisie. */
function RadioFilter<T extends string>({
  icon,
  label,
  value,
  defaultValue,
  labels,
  onChange,
}: {
  icon: "pullRequest" | "user" | "bot";
  label: string;
  value: T;
  defaultValue: T;
  labels: Record<T, string>;
  onChange: (value: T) => void;
}) {
  return (
    <Menu>
      <MenuTrigger asChild>
        <FilterChip icon={icon} label={label} value={value === defaultValue ? undefined : labels[value]} />
      </MenuTrigger>
      <MenuContent>
        <MenuRadioGroup value={value} onValueChange={(v) => onChange(v as T)}>
          {(Object.keys(labels) as T[]).map((key) => (
            <MenuRadioItem key={key} value={key}>
              {labels[key].charAt(0).toUpperCase() + labels[key].slice(1)}
            </MenuRadioItem>
          ))}
        </MenuRadioGroup>
      </MenuContent>
    </Menu>
  );
}
