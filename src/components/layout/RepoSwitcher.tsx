/**
 * Sélecteur de dépôt (haut de la barre latérale) : Cadence fonctionne sur n'importe quel dépôt
 * accessible au compte. Le choix est mémorisé côté Rust et rouvert au lancement suivant.
 */
import { useEffect, useRef, useState } from "react";
import { Popover } from "radix-ui";
import type { Repo } from "@/lib/types";
import { Icon } from "@/components/icons/Icon";
import { Avatar } from "@/components/ui/Avatar";
import { Spinner } from "@/components/ui/Spinner";
import { CreateRepoForm } from "@/components/domain/CreateRepoForm";
import { cn } from "@/lib/cn";
import { formatDaysLeft } from "@/lib/domain/format";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";
import {
  useCurrentProject,
  useCurrentVersion,
  useOpenProject,
  useRepos,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { useUiStore } from "@/stores/ui";

export const repoInitials = (repo: Pick<Repo, "owner" | "name">) =>
  (repo.owner[0]! + repo.name[0]!).toUpperCase();

export function RepoSwitcher() {
  const open = useUiStore((s) => s.projectPickerOpen);
  const setPickerOpen = useUiStore((s) => s.setProjectPickerOpen);
  // `create` : formulaire de nouveau dépôt, dans le même panneau.
  const [mode, setMode] = useState<{ kind: "list" } | { kind: "create"; name: string }>({
    kind: "list",
  });
  const setOpen = (next: boolean) => {
    setPickerOpen(next);
    if (!next) setMode({ kind: "list" });
  };
  const project = useCurrentProject().data;
  const openProject = useOpenProject();
  const navigate = useNavigation((s) => s.navigate);
  const current = useCurrentVersion();
  const version = project ? current.data : undefined;

  const subtitle = !project
    ? "Choisir un dépôt"
    : version
      ? `Sprint ${version.version.title} · ${formatDaysLeft(version.daysLeft)}`
      : project.currentVersion
        ? `Sprint ${project.currentVersion}`
        : "Aucun sprint ouvert";

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          aria-label="Changer de dépôt"
          className={cn(
            "flex h-[42px] w-full items-center gap-[9px] rounded-field border bg-p2 px-2.5 hover:bg-p3",
            open ? "border-clay-line" : "border-line2",
          )}
        >
          <Avatar size={24} tone="neutral" initials={project ? repoInitials(project.repo) : "?"} />
          <span className="min-w-0 grow text-left">
            <span className="block truncate text-body-sm font-medium">
              {project?.repo.fullName ?? "Aucun dépôt"}
            </span>
            <span className={cn("block truncate text-micro", project ? "text-tx3" : "text-clay")}>
              {subtitle}
            </span>
          </span>
          <span className="flex text-tx3">
            <Icon name="chevronDown" size={14} />
          </span>
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          side="bottom"
          align="start"
          sideOffset={6}
          collisionPadding={12}
          className="z-50 w-[340px] overflow-hidden rounded-card border border-line2 bg-panel shadow-modal outline-none"
        >
          {mode.kind === "create" ? (
            <CreateRepoForm
              initialName={mode.name}
              onBack={() => setMode({ kind: "list" })}
              opening={openProject.isPending}
              onCreated={(repo) =>
                openProject.mutate(repo.fullName, {
                  onSuccess: () => {
                    setOpen(false);
                    navigate({ id: "sprint" });
                  },
                })
              }
            />
          ) : (
            <RepoList
              current={project?.repo.fullName ?? null}
              onDone={() => setOpen(false)}
              onCreate={(name) => setMode({ kind: "create", name })}
            />
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function RepoList({
  current,
  onDone,
  onCreate,
}: {
  current: string | null;
  onDone: () => void;
  onCreate: (name: string) => void;
}) {
  const [query, setQuery] = useState("");
  const repos = useRepos(useDebouncedValue(query));
  const openProject = useOpenProject();
  const navigate = useNavigation((s) => s.navigate);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const list = repos.data ?? [];

  useEffect(() => setActive(0), [query]);

  const choose = (repo: Repo) => {
    if (repo.fullName === current) return onDone();
    openProject.mutate(repo.fullName, {
      onSuccess: () => {
        onDone();
        navigate({ id: "sprint" });
      },
    });
  };

  // Navigation au clavier dans la liste, sans quitter le champ de recherche.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(
        0,
        Math.min(list.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)),
      );
      setActive(next);
      listRef.current?.children[next]?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && list[active]) {
      e.preventDefault();
      choose(list[active]);
    }
  };

  return (
    <div>
      <div className="flex h-11 items-center gap-2 border-b border-line px-3 text-tx3">
        <Icon name="search" size={15} />
        <input
          autoFocus
          aria-label="Rechercher un dépôt"
          placeholder="Rechercher un dépôt…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          className="min-w-0 grow bg-transparent text-body text-tx outline-none placeholder:text-tx3"
        />
        {repos.isFetching && <Spinner size={13} />}
      </div>

      <div
        ref={listRef}
        role="listbox"
        aria-label="Dépôts"
        className="max-h-[340px] overflow-y-auto p-1.5"
      >
        {repos.isPending ? (
          <div className="flex h-20 items-center justify-center">
            <Spinner />
          </div>
        ) : repos.isError ? (
          <p className="px-2.5 py-3 text-caption text-red">{repos.error.message}</p>
        ) : list.length === 0 ? (
          <p className="px-2.5 py-3 text-body-sm text-tx3">
            Aucun dépôt ne correspond à « {query} ».
          </p>
        ) : (
          list.map((repo, i) => {
            const selected = repo.fullName === current;
            const opening = openProject.isPending && openProject.variables === repo.fullName;
            return (
              <button
                key={repo.fullName}
                type="button"
                role="option"
                aria-selected={selected}
                disabled={openProject.isPending}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(repo)}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left",
                  i === active ? "bg-p3" : "hover:bg-p2",
                )}
              >
                <Avatar size={24} tone="neutral" initials={repoInitials(repo)} />
                <span className="min-w-0 grow">
                  <span className="flex items-center gap-1.5">
                    <span
                      className={cn(
                        "truncate text-body-sm",
                        selected ? "font-semibold" : "font-medium",
                      )}
                    >
                      {repo.fullName}
                    </span>
                    {repo.private && <span className="text-micro text-tx3">privé</span>}
                  </span>
                  <span className="block truncate text-caption text-tx3">
                    {repo.description ?? "Sans description"}
                  </span>
                </span>
                {opening ? (
                  <Spinner size={13} />
                ) : selected ? (
                  <span className="flex text-clay">
                    <Icon name="check" size={15} />
                  </span>
                ) : (
                  <span className="mono text-tiny whitespace-nowrap text-tx3">
                    {repo.milestones} ms
                  </span>
                )}
              </button>
            );
          })
        )}
      </div>

      <div className="border-t border-line p-1.5">
        <button
          type="button"
          onClick={() => onCreate(query)}
          className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-body-sm text-tx2 hover:bg-p2 hover:text-tx"
        >
          <span className="inline-flex size-6 items-center justify-center rounded-full border border-dashed border-line2 text-tx3">
            <Icon name="plus" size={13} />
          </span>
          {query.trim() ? (
            <span className="min-w-0 truncate">
              Créer « <span className="mono text-tx">{query.trim().replace(/\s+/g, "-")}</span> »
            </span>
          ) : (
            "Créer un dépôt"
          )}
        </button>
      </div>

      {openProject.error && (
        <p className="border-t border-line px-3 py-2 text-caption text-red">
          {openProject.error.message}
        </p>
      )}
      <div className="flex h-8 items-center border-t border-line bg-p2 px-3 text-micro text-tx3">
        Le dépôt choisi est rouvert au prochain lancement.
      </div>
    </div>
  );
}
