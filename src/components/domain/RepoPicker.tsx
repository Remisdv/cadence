/**
 * Choix du dépôt courant d'un plugin qui déclare `services = ["repos"]` : recherche dans tous
 * les dépôts du compte GitHub, récents en tête (oubliables), création d'un dépôt. Un plugin
 * travaille toujours sur un dépôt : son affichage, ses outils et ses données (fichiers, issues
 * cachées) sont ceux de ce dépôt ; un nouveau dépôt donne un espace tout neuf.
 *
 * Choisir un dépôt l'ouvre (`repoSelection`) : il passe en tête des récents, dont le moteur de
 * fond du plugin continue de s'occuper.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Popover } from "radix-ui";
import type { Repo } from "@/lib/types";
import { Icon } from "@/components/icons/Icon";
import { Avatar } from "@/components/ui/Avatar";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { CreateRepoForm } from "./CreateRepoForm";
import { cn } from "@/lib/cn";
import { formatAgo } from "@/lib/domain/format";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";
import { useAppSettings, useRepos, useSaveAppSettings } from "@/lib/query/queries";

const initialsOf = (fullName: string) => {
  const [owner = "", name = ""] = fullName.split("/");
  return `${owner[0] ?? ""}${name[0] ?? ""}`.toUpperCase();
};

/** Une ligne de la liste : un récent (nom seul) ou un dépôt du compte. */
type Entry = { fullName: string; repo: Repo | null; recent: boolean };

export interface RepoPickerProps {
  plugin: string;
  /** Après le choix d'un dépôt (fermer le panneau). */
  onDone?: () => void;
  /** Pied du panneau (rappel de ce que change le choix). */
  footer?: ReactNode;
  className?: string;
}

export function RepoPicker({ plugin, onDone, footer, className }: RepoPickerProps) {
  const [creating, setCreating] = useState<string | null>(null);
  const save = useSaveAppSettings();
  const open = (repo: string) =>
    save.mutate({ repoSelection: { [plugin]: repo } }, { onSuccess: () => onDone?.() });

  return (
    <div className={className}>
      {creating !== null ? (
        <CreateRepoForm
          initialName={creating}
          onBack={() => setCreating(null)}
          opening={save.isPending}
          onCreated={(repo) => open(repo.fullName)}
        />
      ) : (
        <RepoList
          plugin={plugin}
          onOpen={open}
          opening={save.isPending ? (save.variables.repoSelection?.[plugin] ?? null) : null}
          onCreate={setCreating}
        />
      )}
      {save.error && (
        <p className="border-t border-line px-3 py-2 text-caption text-red">{save.error.message}</p>
      )}
      {footer && creating === null && (
        <div className="flex min-h-8 items-center border-t border-line bg-p2 px-3 py-1.5 text-micro leading-[1.5] text-tx3">
          {footer}
        </div>
      )}
    </div>
  );
}

function RepoList({
  plugin,
  onOpen,
  opening,
  onCreate,
}: {
  plugin: string;
  onOpen: (repo: string) => void;
  opening: string | null;
  onCreate: (name: string) => void;
}) {
  const [query, setQuery] = useState("");
  const settings = useAppSettings().data;
  const save = useSaveAppSettings();
  const repos = useRepos(useDebouncedValue(query));
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const recents = settings?.trackedRepos[plugin] ?? [];
  const current = settings?.repoSelection[plugin] ?? null;
  const q = query.trim().toLowerCase();

  const account = repos.data ?? [];
  const byName = new Map(account.map((r) => [r.fullName.toLowerCase(), r]));
  const recentEntries: Entry[] = recents
    .filter((name) => !q || name.toLowerCase().includes(q))
    .map((name) => ({
      fullName: name,
      repo: byName.get(name.toLowerCase()) ?? null,
      recent: true,
    }));
  const known = new Set(recents.map((r) => r.toLowerCase()));
  const others: Entry[] = account
    .filter((r) => !known.has(r.fullName.toLowerCase()))
    .map((r) => ({ fullName: r.fullName, repo: r, recent: false }));
  const entries = [...recentEntries, ...others];

  useEffect(() => setActive(0), [query]);

  const choose = (entry: Entry) => onOpen(entry.fullName);
  const forget = (name: string) =>
    save.mutate({ trackedRepos: { [plugin]: recents.filter((r) => r !== name) } });

  // Navigation au clavier dans la liste, sans quitter le champ de recherche.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(
        0,
        Math.min(entries.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)),
      );
      setActive(next);
      listRef.current
        ?.querySelector(`[data-index="${next}"]`)
        ?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && entries[active]) {
      e.preventDefault();
      choose(entries[active]);
    }
  };

  const line = (entry: Entry, i: number) => {
    const selected = entry.fullName === current;
    const [owner, name] = entry.fullName.split("/");
    const busy = opening === entry.fullName;
    return (
      <div
        key={entry.fullName}
        data-index={i}
        role="option"
        aria-selected={selected}
        tabIndex={-1}
        onMouseEnter={() => setActive(i)}
        onClick={() => opening === null && choose(entry)}
        className={cn(
          "group flex w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-left",
          i === active ? "bg-p3" : "hover:bg-p2",
        )}
      >
        <Avatar size={24} tone="neutral" initials={initialsOf(entry.fullName)} />
        <span className="min-w-0 grow">
          <span className="flex items-center gap-1.5">
            <span
              className={cn("truncate text-body-sm", selected ? "font-semibold" : "font-medium")}
            >
              <span className="font-normal text-tx3">{owner}/</span>
              {name}
            </span>
            {entry.repo?.private && <span className="text-micro text-tx3">privé</span>}
          </span>
          <span className="block truncate text-caption text-tx3">
            {entry.repo
              ? [entry.repo.description, entry.repo.pushedAt && formatAgo(entry.repo.pushedAt)]
                  .filter(Boolean)
                  .join(" · ") || "Sans description"
              : "Ouvert récemment"}
          </span>
        </span>
        {busy ? (
          <Spinner size={13} />
        ) : selected ? (
          <span className="flex text-clay" aria-label="Dépôt courant">
            <Icon name="check" size={15} />
          </span>
        ) : null}
        {entry.recent && !selected && (
          <button
            type="button"
            aria-label={`Oublier ${entry.fullName}`}
            title="Oublier : le retirer des récents"
            disabled={save.isPending}
            onClick={(e) => {
              e.stopPropagation();
              forget(entry.fullName);
            }}
            className="hidden size-6 shrink-0 items-center justify-center rounded-control text-tx3 group-hover:inline-flex hover:bg-p4 hover:text-tx"
          >
            <Icon name="close" size={12} />
          </button>
        )}
      </div>
    );
  };

  return (
    <div>
      <div className="flex h-11 items-center gap-2 border-b border-line px-3 text-tx3">
        <Icon name="search" size={15} />
        <input
          autoFocus
          aria-label="Rechercher un dépôt"
          placeholder="Rechercher un de vos dépôts…"
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
        className="max-h-[360px] overflow-y-auto p-1.5"
      >
        {recentEntries.length > 0 && (
          <SectionLabel className="px-2.5 pt-1 pb-1">Récents</SectionLabel>
        )}
        {recentEntries.map((entry, i) => line(entry, i))}
        <SectionLabel className="px-2.5 pt-2 pb-1">Vos dépôts GitHub</SectionLabel>
        {repos.isPending ? (
          <div className="flex h-16 items-center justify-center">
            <Spinner />
          </div>
        ) : repos.isError ? (
          <p className="px-2.5 py-2 text-caption text-red">{repos.error.message}</p>
        ) : others.length === 0 ? (
          <p className="px-2.5 py-2 text-body-sm text-tx3">
            {q ? `Aucun autre dépôt ne correspond à « ${query.trim()} ».` : "Aucun autre dépôt."}
          </p>
        ) : (
          others.map((entry, i) => line(entry, recentEntries.length + i))
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
    </div>
  );
}

/** `RepoPicker` dans un panneau ancré à `children` (bouton déclencheur). */
export function RepoPickerPopover({
  plugin,
  children,
  align = "start",
  footer,
}: {
  plugin: string;
  children: ReactNode;
  align?: "start" | "end";
  footer?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>{children}</Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          side="bottom"
          align={align}
          sideOffset={6}
          collisionPadding={12}
          className="z-50 w-[360px] overflow-hidden rounded-card border border-line2 bg-panel shadow-modal outline-none"
        >
          <RepoPicker plugin={plugin} onDone={() => setOpen(false)} footer={footer} />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
