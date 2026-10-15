/**
 * Issues (maquette Issues.dc.html) : toutes les issues du dépôt ouvert, groupées par version.
 *
 * Lecture seule en 0.1.0 : les actions groupées (version, assignation, fermeture) et l'agent
 * CTO arrivent avec les versions qui écrivent sur GitHub ; leurs boutons sont désactivés.
 */
import { useMemo, useState } from "react";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { FilterChip, FilterInput } from "@/components/domain/Filters";
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
import {
  groupByVersion,
  matchesView,
  type IssueViewFilter,
  type StateFilter,
} from "@/lib/domain/issueGroups";
import { useCurrentProject, useIssues, useVersions } from "@/lib/query/queries";
import { useUiStore } from "@/stores/ui";
import { IssueTable } from "./IssueTable";

const stateLabels: Record<StateFilter, string> = {
  open: "ouvertes",
  closed: "fermées",
  all: "toutes",
};

const defaultFilter: IssueViewFilter = { query: "", versions: [], state: "open", assignee: "all" };

export default function IssuesView() {
  const project = useCurrentProject().data;
  const issues = useIssues({}, { live: true });
  const versions = useVersions().data ?? [];
  const openDialog = useUiStore((s) => s.openDialog);

  const [filter, setFilter] = useState<IssueViewFilter>(defaultFilter);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const set = (patch: Partial<IssueViewFilter>) => setFilter((f) => ({ ...f, ...patch }));

  const all = useMemo(() => issues.data ?? [], [issues.data]);
  const visible = useMemo(() => all.filter((i) => matchesView(i, filter)), [all, filter]);
  const groups = useMemo(
    () => groupByVersion(all, visible, versions, project?.currentVersion ?? null),
    [all, visible, versions, project?.currentVersion],
  );

  const openCount = all.filter((i) => i.status !== "done").length;
  const versionTitles = [...new Set(all.map((i) => i.version ?? ""))].sort();
  const filtered =
    filter.query !== "" ||
    filter.versions.length > 0 ||
    filter.state !== "open" ||
    filter.assignee !== "all";

  return (
    <Page
      title="Issues"
      subtitle={
        project &&
        (issues.data
          ? `${openCount} ouvertes · ${all.length - openCount} fermées · dépôt ${project.repo.fullName}`
          : `dépôt ${project.repo.fullName}`)
      }
      actions={
        <>
          <Button icon="bot" disabled title="Arrive avec l'agent CTO (version 0.1.3)">
            Agent CTO
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openDialog("issue-create")}>
            Nouvelle issue
          </Button>
        </>
      }
      className="flex flex-col gap-3"
    >
      <div className="flex shrink-0 items-center gap-2">
        <FilterInput
          aria-label="Filtrer les issues"
          placeholder="Filtrer par titre, label, n°, assigné…"
          value={filter.query}
          onChange={(e) => set({ query: e.target.value })}
        />

        <Menu>
          <MenuTrigger asChild>
            <FilterChip
              icon="tag"
              label="Version"
              value={filter.versions.length > 0 ? formatVersions(filter.versions) : undefined}
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
                {v === project?.currentVersion && (
                  <span className="ml-auto text-caption text-tx3">en cours</span>
                )}
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

        <Menu>
          <MenuTrigger asChild>
            <FilterChip
              icon="issue"
              label="État"
              value={filter.state === "open" ? undefined : stateLabels[filter.state]}
            />
          </MenuTrigger>
          <MenuContent>
            <MenuRadioGroup
              value={filter.state}
              onValueChange={(v) => set({ state: v as StateFilter })}
            >
              <MenuRadioItem value="open">Ouvertes</MenuRadioItem>
              <MenuRadioItem value="closed">Fermées</MenuRadioItem>
              <MenuRadioItem value="all">Toutes</MenuRadioItem>
            </MenuRadioGroup>
          </MenuContent>
        </Menu>

        <Menu>
          <MenuTrigger asChild>
            <FilterChip
              icon="user"
              label="Assigné"
              value={filter.assignee === "me" ? "moi" : undefined}
            />
          </MenuTrigger>
          <MenuContent>
            <MenuRadioGroup
              value={filter.assignee}
              onValueChange={(v) => set({ assignee: v as "all" | "me" })}
            >
              <MenuRadioItem value="all">Tout le monde</MenuRadioItem>
              <MenuRadioItem value="me">Moi</MenuRadioItem>
            </MenuRadioGroup>
          </MenuContent>
        </Menu>

        <div className="grow" />
        {filtered && (
          <Button variant="quiet" size="sm" onClick={() => setFilter(defaultFilter)}>
            Réinitialiser
          </Button>
        )}
        <Badge tone="muted">Groupé par version</Badge>
      </div>

      <Card className="flex min-h-0 grow flex-col overflow-hidden">
        {issues.isPending ? (
          <LoadingState label="Lecture des issues sur GitHub…" />
        ) : issues.isError ? (
          <ErrorState error={issues.error} />
        ) : all.length === 0 ? (
          <EmptyState icon="issue" title="Aucune issue dans ce dépôt">
            Créez la première avec « Nouvelle issue » : elle sera rattachée à une version.
          </EmptyState>
        ) : groups.length === 0 ? (
          <EmptyState
            icon="search"
            title="Aucune issue ne correspond aux filtres"
            action={
              <Button variant="ghost" onClick={() => setFilter(defaultFilter)}>
                Réinitialiser les filtres
              </Button>
            }
          />
        ) : (
          <IssueTable
            groups={groups}
            repo={project?.repo.fullName ?? ""}
            selected={selected}
            onSelectedChange={setSelected}
          />
        )}
      </Card>
    </Page>
  );
}

/** « 0.1.2 et 0.1.3 », « 0.1.2, 0.1.3 +1 » */
function formatVersions(list: string[]): string {
  const names = list.map((v) => v || "sans version");
  if (names.length <= 2) return names.join(" et ");
  return `${names.slice(0, 2).join(", ")} +${names.length - 2}`;
}
