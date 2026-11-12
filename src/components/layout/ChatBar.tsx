/**
 * Colonne des conversations du chat global (V2-Chat.dc.html) : recherche, puis les
 * conversations rangées par catégorie comme les projets de Claude Code — « Global », puis un
 * plugin par section (celui choisi dans le composeur, ou celui d'où une action l'a ouverte).
 * Chaque catégorie se replie (mémorisé) et a son « + » : nouvelle conversation dans ce plugin.
 *
 * Les conversations rattachées à un groupe (un sprint) restent dans la vue de leur plugin, même
 * en attente ; la colonne ne les montre (section « Rattachées ») que si elles sont ouvertes ou
 * répondent à la recherche.
 */
import { useMemo, useState, type ReactNode } from "react";
import { Icon } from "@/components/icons/Icon";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { chatBarSections, contextPlugin, conversationTime, reposLabel } from "@/lib/domain/chat";
import { useChatSessions, usePlugins } from "@/lib/query/queries";
import type { ChatSessionRow, InstalledPluginView } from "@/lib/types";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarHint,
  ContextBarIconButton,
  ContextBarSection,
  ConversationItem,
} from "./ContextBar";

/** Catégories repliées de la colonne (`global` ou id de plugin), gardées entre deux lancements. */
const COLLAPSED_KEY = "cadence.chat.collapsed";

function readCollapsed(): string[] {
  try {
    const raw = JSON.parse(globalThis.localStorage?.getItem(COLLAPSED_KEY) ?? "[]") as unknown;
    return Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : [];
  } catch {
    return [];
  }
}

function writeCollapsed(keys: string[]) {
  try {
    globalThis.localStorage?.setItem(COLLAPSED_KEY, JSON.stringify(keys));
  } catch {
    // Stockage indisponible : l'état vaut pour cette session seulement.
  }
}

export function ChatBar({
  active,
  onSelect,
  onNew,
  onNewIn,
}: {
  active?: string;
  onSelect: (session: string) => void;
  onNew: () => void;
  /** « + » d'une catégorie : nouvelle conversation dans ce plugin (`null` = Global). */
  onNewIn?: (plugin: string | null) => void;
}) {
  const sessions = useChatSessions();
  const plugins = usePlugins().data;
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<string[]>(readCollapsed);
  const toggle = (key: string) =>
    setCollapsed((current) => {
      const next = current.includes(key) ? current.filter((k) => k !== key) : [...current, key];
      writeCollapsed(next);
      return next;
    });

  const groups = useMemo(
    () => chatBarSections(sessions.data ?? [], query, active ?? null, plugins ?? []),
    [sessions.data, query, active, plugins],
  );
  const searching = query.trim().length > 0;

  return (
    <ContextBar
      label="Conversations"
      header={
        <ContextBarHeader
          tile={<NeutralTile icon="chat" />}
          title="Chat"
          action={
            <ContextBarIconButton
              icon="plus"
              label="Nouvelle conversation"
              size={28}
              bordered
              onClick={onNew}
            />
          }
        />
      }
      footer={
        <ContextBarHint>
          Choisissez le plugin d'une conversation dans le composeur : elle est rangée dans sa
          catégorie. Celles d'un sprint restent dans sa vue : la recherche les retrouve.
        </ContextBarHint>
      }
    >
      <label className="flex h-[30px] shrink-0 items-center gap-2 rounded-control border border-line bg-p2 px-[9px] text-tx3">
        <Icon name="search" size={14} />
        <input
          aria-label="Rechercher une conversation"
          placeholder="Rechercher…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
        />
      </label>
      {sessions.isPending && (
        <span className="flex items-center gap-2 px-[9px] text-caption text-tx3">
          <Spinner size={12} />
          Chargement…
        </span>
      )}
      {sessions.isSuccess && groups.length === 0 && (
        <p className="px-[9px] py-1 text-caption leading-[1.55] text-tx3">
          {searching ? "Aucune conversation ne correspond." : "Aucune conversation pour l'instant."}
        </p>
      )}
      {groups.map((group) => {
        const items = group.rows.map((s) => (
          <ConversationItem
            key={s.id}
            title={s.title}
            time={conversationTime(s.updatedAt)}
            active={s.id === active}
            onClick={() => onSelect(s.id)}
            context={
              <SessionContext
                session={s}
                plugins={plugins ?? []}
                group={group.attached ? s.group?.label : undefined}
                // Dans sa catégorie, la puce dit ses dépôts plutôt que le plugin, déjà en titre.
                repos={group.plugin !== undefined}
              />
            }
          />
        ));
        if (group.plugin === undefined) {
          return (
            <ContextBarSection key={group.title} title={group.title}>
              {items}
            </ContextBarSection>
          );
        }
        const key = group.plugin ?? "global";
        const plugin = plugins?.find((p) => p.id === group.plugin);
        const open =
          !collapsed.includes(key) || searching || group.rows.some((s) => s.id === active);
        const canStart = !!onNewIn && (group.plugin === null || !!plugin?.enabled);
        return (
          <CategorySection
            key={key}
            title={group.title}
            tile={
              plugin ? (
                <PluginTile color={plugin.color} icon={plugin.railIcon} size={13} />
              ) : (
                <NeutralTile icon="chat" size={13} />
              )
            }
            count={group.rows.length}
            open={open}
            onToggle={() => toggle(key)}
            onNew={canStart ? () => onNewIn(group.plugin ?? null) : undefined}
          >
            {items}
          </CategorySection>
        );
      })}
    </ContextBar>
  );
}

/** Catégorie de la colonne (Global ou un plugin) : en-tête repliable, compteur, « + ». */
function CategorySection({
  title,
  tile,
  count,
  open,
  onToggle,
  onNew,
  children,
}: {
  title: string;
  tile: ReactNode;
  count: number;
  open: boolean;
  onToggle: () => void;
  onNew?: () => void;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <div className="group/cat flex h-6 items-center gap-0.5">
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={open}
          className="flex h-6 min-w-0 grow items-center gap-1.5 rounded-control pr-1.5 pl-[5px] text-left hover:bg-p2"
        >
          <Icon
            name="chevronRight"
            size={11}
            className={cn("shrink-0 text-tx3 transition-transform", open && "rotate-90")}
          />
          {tile}
          <span className="min-w-0 grow truncate text-caption font-medium text-tx2">{title}</span>
          <span className="mono text-tiny text-tx3">{count}</span>
        </button>
        {onNew && (
          <button
            type="button"
            onClick={onNew}
            aria-label={`Nouvelle conversation · ${title}`}
            title={`Nouvelle conversation · ${title}`}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded-control text-tx3 opacity-0 group-hover/cat:opacity-100 hover:bg-p3 hover:text-tx focus-visible:opacity-100"
          >
            <Icon name="plus" size={12} />
          </button>
        )}
      </div>
      {open && children}
    </div>
  );
}

/** Puce de contexte d'une conversation (colonne et en-tête). */
export function SessionContext({
  session,
  plugins,
  group,
  repos,
}: {
  session: Pick<ChatSessionRow, "origin" | "state"> & Partial<Pick<ChatSessionRow, "repos">>;
  plugins: InstalledPluginView[];
  /** Groupe d'une conversation rattachée (« Sprint 0.2.0 · owner/name »), montré à la place. */
  group?: string;
  /** Dans une catégorie : ses dépôts (ou « Sans dépôt ») à la place du plugin. */
  repos?: boolean;
}) {
  const plugin = plugins.find((p) => p.id === contextPlugin(session.origin));
  const list = session.repos ?? [];
  if (repos && !group) {
    return (
      <>
        <Icon
          name="branch"
          size={11}
          className={cn("shrink-0", list.length > 0 && "text-green")}
        />
        <span className={cn("truncate", list.length > 0 && "mono")}>
          {list.length > 0 ? reposLabel(list) : "Sans dépôt"}
        </span>
        <SessionState state={session.state} />
      </>
    );
  }
  return (
    <>
      {plugin ? (
        <PluginTile color={plugin.color} icon={plugin.railIcon} size={13} />
      ) : (
        <NeutralTile icon="chat" size={13} />
      )}
      {group && <Icon name="sprint" size={11} className="shrink-0" />}
      <span className="truncate">
        {group ?? (plugin ? plugin.name : "Global · tous les plugins")}
      </span>
      <SessionState state={session.state} />
    </>
  );
}

/** Pastille d'état d'une conversation : en cours, accord attendu, réponse attendue, échec. */
function SessionState({ state }: { state: ChatSessionRow["state"] }) {
  if (state === "running" || state === "waiting-approval")
    return (
      <span className="ml-auto flex shrink-0 text-clay">
        {state === "running" ? <Spinner size={11} /> : <Icon name="warning" size={11} />}
      </span>
    );
  if (state === "waiting-reply")
    return (
      <span
        className="ml-auto flex shrink-0 text-clay"
        title="En attente de votre réponse"
        aria-label="En attente de votre réponse"
      >
        <Icon name="chat" size={11} />
      </span>
    );
  if (state === "failed")
    return (
      <span className="ml-auto flex shrink-0 text-red" title="Dernier tour en échec">
        <Icon name="warning" size={11} />
      </span>
    );
  return null;
}
