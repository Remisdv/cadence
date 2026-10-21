/**
 * Colonne des conversations du chat global (V2-Chat.dc.html) : recherche, conversations par
 * jour avec leur puce de contexte (« Global · tous les plugins », ou le plugin d'où une action
 * l'a ouverte), nouvelle conversation.
 *
 * Les conversations rattachées à un groupe (un sprint) restent dans la vue de leur plugin ; la
 * colonne ne les montre (section « Rattachées ») que si elles sont ouvertes, attendent votre
 * réponse ou répondent à la recherche.
 */
import { useMemo, useState } from "react";
import { Icon } from "@/components/icons/Icon";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Spinner } from "@/components/ui/Spinner";
import { chatBarSections, contextPlugin, conversationTime } from "@/lib/domain/chat";
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

export function ChatBar({
  active,
  onSelect,
  onNew,
}: {
  active?: string;
  onSelect: (session: string) => void;
  onNew: () => void;
}) {
  const sessions = useChatSessions();
  const plugins = usePlugins().data ?? [];
  const [query, setQuery] = useState("");

  const groups = useMemo(
    () => chatBarSections(sessions.data ?? [], query, active ?? null),
    [sessions.data, query, active],
  );

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
          Les conversations ouvertes depuis un plugin apparaissent aussi ici, avec leur puce de
          contexte. Celles d'un sprint restent dans sa vue : la recherche les retrouve.
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
          {query.trim()
            ? "Aucune conversation ne correspond."
            : "Aucune conversation pour l'instant."}
        </p>
      )}
      {groups.map((group) => (
        <ContextBarSection key={group.title} title={group.title}>
          {group.rows.map((s) => (
            <ConversationItem
              key={s.id}
              title={s.title}
              time={conversationTime(s.updatedAt)}
              active={s.id === active}
              onClick={() => onSelect(s.id)}
              context={
                <SessionContext
                  session={s}
                  plugins={plugins}
                  group={group.attached ? s.group?.label : undefined}
                />
              }
            />
          ))}
        </ContextBarSection>
      ))}
    </ContextBar>
  );
}

/** Puce de contexte d'une conversation (colonne et en-tête). */
export function SessionContext({
  session,
  plugins,
  group,
}: {
  session: Pick<ChatSessionRow, "origin" | "state">;
  plugins: InstalledPluginView[];
  /** Groupe d'une conversation rattachée (« Sprint 0.2.0 · owner/name »), montré à la place. */
  group?: string;
}) {
  const plugin = plugins.find((p) => p.id === contextPlugin(session.origin));
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
      {(session.state === "running" || session.state === "waiting-approval") && (
        <span className="ml-auto flex shrink-0 text-clay">
          {session.state === "running" ? <Spinner size={11} /> : <Icon name="warning" size={11} />}
        </span>
      )}
      {session.state === "waiting-reply" && (
        <span
          className="ml-auto flex shrink-0 text-clay"
          title="En attente de votre réponse"
          aria-label="En attente de votre réponse"
        >
          <Icon name="chat" size={11} />
        </span>
      )}
      {session.state === "failed" && (
        <span className="ml-auto flex shrink-0 text-red" title="Dernier tour en échec">
          <Icon name="warning" size={11} />
        </span>
      )}
    </>
  );
}
