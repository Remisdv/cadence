/**
 * Colonne des conversations du chat global (V2-Chat.dc.html) : recherche, conversations par
 * jour avec leur puce de contexte (« Global · tous les plugins », ou le plugin d'où une action
 * l'a ouverte), nouvelle conversation.
 */
import { useMemo, useState } from "react";
import { Icon } from "@/components/icons/Icon";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Spinner } from "@/components/ui/Spinner";
import { conversationGroup, conversationTime, originPlugin } from "@/lib/domain/chat";
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

const GROUPS = ["Aujourd'hui", "Hier", "Cette semaine", "Plus ancien"];

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

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const rows = (sessions.data ?? []).filter((s) => !q || s.title.toLowerCase().includes(q));
    const now = new Date();
    // Exécutions de fond des actions : leur propre groupe, en tête (panneau Activité).
    const background = rows.filter((s) => s.mode === "background");
    const chats = rows.filter((s) => s.mode !== "background");
    return [
      { title: "En fond", rows: background },
      ...GROUPS.map((title) => ({
        title,
        rows: chats.filter((s) => conversationGroup(s.updatedAt, now) === title),
      })),
    ].filter((g) => g.rows.length > 0);
  }, [sessions.data, query]);

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
          contexte.
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
              context={<SessionContext session={s} plugins={plugins} />}
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
}: {
  session: Pick<ChatSessionRow, "origin" | "state">;
  plugins: InstalledPluginView[];
}) {
  const plugin = plugins.find((p) => p.id === originPlugin(session.origin));
  return (
    <>
      {plugin ? (
        <PluginTile color={plugin.color} icon={plugin.railIcon} size={13} />
      ) : (
        <NeutralTile icon="chat" size={13} />
      )}
      <span className="truncate">{plugin ? plugin.name : "Global · tous les plugins"}</span>
      {(session.state === "running" || session.state === "waiting-approval") && (
        <span className="ml-auto flex shrink-0 text-clay">
          {session.state === "running" ? <Spinner size={11} /> : <Icon name="warning" size={11} />}
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
