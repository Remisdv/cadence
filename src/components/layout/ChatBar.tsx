/**
 * Colonne des conversations du chat global (V2-Chat.dc.html) : recherche, conversations par
 * jour avec leur puce de contexte. Les sessions de chat arrivent avec l'adaptateur Claude
 * Code ; d'ici là la colonne est vide.
 */
import { Icon } from "@/components/icons/Icon";
import { NeutralTile } from "@/components/domain/PluginTile";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarHint,
  ContextBarIconButton,
  ContextBarSection,
} from "./ContextBar";

export function ChatBar() {
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
              disabled
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
          className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
        />
      </label>
      <ContextBarSection title="Aujourd'hui">
        <p className="px-[9px] py-1 text-caption leading-[1.55] text-tx3">
          Aucune conversation pour l'instant.
        </p>
      </ContextBarSection>
    </ContextBar>
  );
}
