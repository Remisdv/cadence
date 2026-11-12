/**
 * Sélecteur du dépôt actif d'un plugin du service Dépôts (haut de la barre du plugin).
 *
 * Un plugin travaille dans un seul dépôt à la fois : tout ce qu'il gère (issues, catégories,
 * roadmap…) vit dans ce dépôt. Le menu liste les dépôts du compte GitHub connecté (récents en
 * tête), avec recherche et création ; choisir un dépôt le suit et le sélectionne d'un coup. Plus
 * de lien à coller ni de mode « Tous les dépôts ».
 */
import { useEffect, useRef, useState } from "react";
import { Popover } from "radix-ui";
import { Icon } from "@/components/icons/Icon";
import { Avatar } from "@/components/ui/Avatar";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { activeRepo, mergeRepoRows, type RepoRow } from "@/lib/domain/repos";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";
import { useActivatePluginRepo, useAppSettings, useAuthStatus, useRepos } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { CreateRepoForm } from "./CreateRepoForm";

const initials = (fullName: string) => {
  const [owner = "?", name = "?"] = fullName.split("/");
  return (owner[0]! + name[0]!).toUpperCase();
};

export function PluginRepoSelector({ plugin }: { plugin: string }) {
  const settings = useAppSettings().data;
  const tracked = settings?.trackedRepos[plugin] ?? [];
  const current = activeRepo(tracked, settings?.repoSelection[plugin]);
  const [open, setOpenState] = useState(false);
  const [mode, setMode] = useState<{ kind: "list" } | { kind: "create"; name: string }>({
    kind: "list",
  });
  const setOpen = (next: boolean) => {
    setOpenState(next);
    if (!next) setMode({ kind: "list" });
  };
  const activate = useActivatePluginRepo(plugin);
  const choose = async (repo: string) => {
    if (repo !== current) await activate.mutateAsync(repo);
    setOpen(false);
  };

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          aria-label="Choisir le dépôt"
          className={cn(
            "flex h-9 shrink-0 items-center gap-2 rounded-lg border bg-p2 px-2 text-tx hover:bg-p3",
            open ? "border-clay-line" : "border-line2",
          )}
        >
          <span
            aria-hidden="true"
            className="inline-flex size-5 shrink-0 items-center justify-center rounded-md bg-p3 text-tx3"
          >
            <Icon name="github" size={12} />
          </span>
          <span
            className={cn(
              "min-w-0 grow truncate text-left text-body-sm font-medium",
              !current && "text-clay",
            )}
          >
            {current ?? "Choisir un dépôt"}
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
              onCreated={(repo) => choose(repo.fullName)}
            />
          ) : (
            <RepoList
              current={current}
              recents={tracked}
              pending={activate.isPending ? activate.variables : null}
              error={activate.error?.message ?? null}
              onChoose={(repo) => void choose(repo).catch(() => {})}
              onCreate={(name) => setMode({ kind: "create", name })}
              onClose={() => setOpen(false)}
            />
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

type Row = RepoRow;

function RepoList({
  current,
  recents,
  pending,
  error,
  onChoose,
  onCreate,
  onClose,
}: {
  current: string | null;
  recents: string[];
  pending: string | null;
  error: string | null;
  onChoose: (repo: string) => void;
  onCreate: (name: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query);
  const auth = useAuthStatus();
  const connected = auth.data?.github.status === "connected";
  const repos = useRepos(search);
  const navigate = useNavigation((s) => s.navigate);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const rows = mergeRepoRows(recents, connected ? (repos.data ?? []) : [], search);
  useEffect(() => setActive(0), [search]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(0, Math.min(rows.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)));
      setActive(next);
      listRef.current?.querySelectorAll("[role=option]")[next]?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && rows[active]) {
      e.preventDefault();
      onChoose(rows[active].fullName);
    }
  };

  const firstOther = rows.findIndex((r) => !r.recent);

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
        {connected && repos.isFetching && <Spinner size={13} />}
      </div>

      {!connected && auth.data && (
        <div className="flex flex-col gap-2 border-b border-line px-3 py-2.5 text-caption text-tx2">
          Connectez GitHub pour lister vos dépôts et en créer.
          <button
            type="button"
            onClick={() => {
              onClose();
              navigate({ id: "settings", section: "repos" });
            }}
            className="self-start text-clay hover:underline"
          >
            Connecter GitHub…
          </button>
        </div>
      )}

      <div
        ref={listRef}
        role="listbox"
        aria-label="Dépôts"
        className="max-h-[340px] overflow-y-auto p-1.5"
      >
        {connected && repos.isPending && rows.length === 0 ? (
          <div className="flex h-20 items-center justify-center">
            <Spinner />
          </div>
        ) : connected && repos.isError && rows.length === 0 ? (
          <p className="px-2.5 py-3 text-caption text-red">{repos.error.message}</p>
        ) : rows.length === 0 ? (
          <p className="px-2.5 py-3 text-body-sm text-tx3">
            {query.trim() ? `Aucun dépôt ne correspond à « ${query.trim()} ».` : "Aucun dépôt."}
          </p>
        ) : (
          rows.map((row, i) => (
            <div key={row.fullName}>
              {i === 0 && row.recent && <GroupLabel>Récents</GroupLabel>}
              {i === firstOther && <GroupLabel>Vos dépôts GitHub</GroupLabel>}
              <RepoOption
                row={row}
                selected={row.fullName.toLowerCase() === current?.toLowerCase()}
                highlighted={i === active}
                pending={pending === row.fullName}
                disabled={pending !== null}
                onHover={() => setActive(i)}
                onChoose={() => onChoose(row.fullName)}
              />
            </div>
          ))
        )}
      </div>

      {connected && (
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
      )}

      {error && <p className="border-t border-line px-3 py-2 text-caption text-red">{error}</p>}
      <div className="flex h-8 items-center border-t border-line bg-p2 px-3 text-micro text-tx3">
        Tout ce que le plugin gère est enregistré dans ce dépôt.
      </div>
    </div>
  );
}

function GroupLabel({ children }: { children: string }) {
  return (
    <div className="px-2.5 pt-2 pb-1 text-micro font-medium tracking-[0.04em] text-tx3 uppercase">
      {children}
    </div>
  );
}

function RepoOption({
  row,
  selected,
  highlighted,
  pending,
  disabled,
  onHover,
  onChoose,
}: {
  row: Row;
  selected: boolean;
  highlighted: boolean;
  pending: boolean;
  disabled: boolean;
  onHover: () => void;
  onChoose: () => void;
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      disabled={disabled}
      onMouseEnter={onHover}
      onClick={onChoose}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left",
        highlighted ? "bg-p3" : "hover:bg-p2",
      )}
    >
      <Avatar size={24} tone="neutral" initials={initials(row.fullName)} />
      <span className="min-w-0 grow">
        <span className="flex items-center gap-1.5">
          <span className={cn("truncate text-body-sm", selected ? "font-semibold" : "font-medium")}>
            {row.fullName}
          </span>
          {row.private && <span className="text-micro text-tx3">privé</span>}
        </span>
        {row.description && (
          <span className="block truncate text-caption text-tx3">{row.description}</span>
        )}
      </span>
      {pending ? (
        <Spinner size={13} />
      ) : selected ? (
        <span className="flex text-clay">
          <Icon name="check" size={15} />
        </span>
      ) : null}
    </button>
  );
}
