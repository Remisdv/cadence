/**
 * Chat global (maquette V2-Chat.dc.html) : la colonne des conversations, l'en-tête avec la
 * puce de contexte, le fil et le composeur. Le fil et l'envoi arrivent avec l'adaptateur
 * Claude Code et le chat du cœur (plan § 4.2 et § 5) ; l'écran montre déjà l'IA choisie et
 * les plugins dont elle verra les outils.
 */
import { useState } from "react";
import { ChatBar } from "@/components/layout/ChatBar";
import { Page } from "@/components/layout/Page";
import { Icon } from "@/components/icons/Icon";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Button } from "@/components/ui/Button";
import { aiLabels } from "@/lib/domain/ai";
import { useAppSettings, usePlugins } from "@/lib/query/queries";

export default function ChatView() {
  const settings = useAppSettings().data;
  const plugins = (usePlugins().data ?? []).filter((p) => p.enabled && p.status !== "invalid");
  const ai = settings?.defaultAi ? aiLabels[settings.defaultAi].name : null;
  const n = plugins.length;

  return (
    <>
      <ChatBar />
      <Page
        title={
          <span className="flex items-center gap-[9px]">
            Nouvelle conversation
            <ContextChip />
          </span>
        }
        subtitle={[
          "Chat global",
          ai ?? "aucune IA choisie",
          n > 0
            ? `voit les outils des ${n} plugin${n > 1 ? "s" : ""} activé${n > 1 ? "s" : ""}`
            : "aucun plugin activé",
        ].join(" · ")}
        actions={
          <Button icon="plus" disabled title="Arrive avec le chat">
            Nouvelle conversation
          </Button>
        }
        padding="none"
      >
        <div className="flex min-w-0 grow flex-col">
          <div className="min-h-0 grow overflow-y-auto px-[30px] pt-[22px]">
            <div className="mx-auto flex max-w-[780px] flex-col items-center gap-4 pt-16 text-center">
              <span className="inline-flex size-10 items-center justify-center rounded-tile bg-clay text-ink">
                <Icon name="sparkle" size={22} />
              </span>
              <h2 className="font-display text-title font-semibold">
                {ai ? `Discuter avec ${ai}` : "Choisissez une IA de code"}
              </h2>
              <p className="max-w-[520px] text-body-sm leading-[1.6] text-tx2">
                Le chat global voit les outils de tous les plugins activés. L'IA travaille dans un
                dossier neutre de Cadence, jamais dans un de vos dépôts. L'envoi de messages arrive
                avec l'adaptateur Claude Code.
              </p>
              {n > 0 && (
                <div className="flex flex-wrap justify-center gap-1.5">
                  {plugins.map((p) => (
                    <span
                      key={p.id}
                      className="inline-flex h-[26px] items-center gap-1.5 rounded-control border border-line bg-p2 pr-2 pl-1 text-caption text-tx2"
                    >
                      <PluginTile color={p.color} icon={p.railIcon} size={18} />
                      {p.name}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
          <Composer ai={ai} />
        </div>
      </Page>
    </>
  );
}

/** Puce de contexte « Global · tous les plugins » (un chat contextuel nomme son plugin). */
export function ContextChip() {
  return (
    <span className="inline-flex h-[22px] items-center gap-1.5 rounded-md border border-line bg-p2 pr-2 pl-1 font-sans text-tiny font-normal tracking-normal whitespace-nowrap text-tx2">
      <NeutralTile icon="chat" size={13} />
      Global · tous les plugins
    </span>
  );
}

/** Composeur (V2-Chat) : zone de texte, choix de l'IA, skills, pièce jointe, envoi. */
function Composer({ ai }: { ai: string | null }) {
  const [text, setText] = useState("");
  return (
    <div className="shrink-0 px-[30px] pt-3.5 pb-[18px]">
      <div className="mx-auto max-w-[780px] rounded-card border border-line2 bg-panel">
        <label htmlFor="chat-input" className="sr-only">
          Message pour l'IA
        </label>
        <textarea
          id="chat-input"
          rows={1}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={`Écrire à ${ai ?? "l'IA"}… (/ pour lancer un skill, @ pour viser un plugin)`}
          className="block w-full resize-none bg-transparent px-3.5 pt-3 pb-1 text-body leading-[1.55] text-tx outline-none placeholder:text-tx3"
        />
        <div className="flex items-center gap-2 px-2.5 py-2">
          <span className="inline-flex h-[30px] items-center gap-[7px] rounded-control border border-line bg-p2 pr-[9px] pl-1.5 text-body-sm text-tx">
            <span className="inline-flex size-[18px] items-center justify-center rounded-badge bg-clay text-ink">
              <Icon name="sparkle" size={11} />
            </span>
            {ai ?? "Aucune IA"}
          </span>
          <button
            type="button"
            disabled
            aria-label="Joindre un fichier"
            className="inline-flex size-[30px] items-center justify-center rounded-control border border-line text-tx3"
          >
            <Icon name="paperclip" size={15} />
          </button>
          <div className="grow" />
          <button
            type="button"
            disabled
            aria-label="Envoyer"
            title="L'envoi arrive avec l'adaptateur Claude Code"
            className="inline-flex size-8 items-center justify-center rounded-lg bg-clay text-clay-ink disabled:opacity-50"
          >
            <Icon name="send" size={15} />
          </button>
        </div>
      </div>
    </div>
  );
}
