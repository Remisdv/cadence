/**
 * Chat global (maquette V2-Chat.dc.html) : la colonne des conversations, l'en-tête avec la
 * puce de contexte, le fil, le composeur et le panneau de la session. Les réponses arrivent
 * par `chat://event` (useBackendEvents tient le cache à jour).
 */
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { ChatBar, SessionContext } from "@/components/layout/ChatBar";
import { Page } from "@/components/layout/Page";
import { Icon } from "@/components/icons/Icon";
import { ChatThread } from "@/components/domain/ChatThread";
import { PluginTile } from "@/components/domain/PluginTile";
import { Button } from "@/components/ui/Button";
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
} from "@/components/ui/Dialog";
import { Field, Input } from "@/components/ui/Field";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "@/components/ui/Menu";
import { aiLabels } from "@/lib/domain/ai";
import { formatDuration, parseCadenceLink, type ChatItem } from "@/lib/domain/chat";
import {
  useAppSettings,
  useApproveChat,
  useChatThread,
  useDeleteChat,
  useInterruptChat,
  usePluginStatus,
  usePlugins,
  useRenameChat,
  useSendChat,
} from "@/lib/query/queries";
import type { ChatSessionRow, InstalledPluginView } from "@/lib/types";
import { navigate, useNavigation } from "@/stores/navigation";
import type { ViewProps } from "@/views/registry";

export default function ChatView({ view }: ViewProps<"chat">) {
  const sessionId = view.session ?? null;
  const replace = useNavigation((s) => s.replace);
  const settings = useAppSettings().data;
  const plugins = (usePlugins().data ?? []).filter((p) => p.enabled && p.status !== "invalid");
  const thread = useChatThread(sessionId);
  const session = sessionId ? thread.data?.session : undefined;
  const send = useSendChat();
  const interrupt = useInterruptChat();
  const approve = useApproveChat(sessionId ?? "");
  const [dialog, setDialog] = useState<"rename" | "delete" | null>(null);

  const aiId = session?.ai ?? settings?.defaultAi ?? null;
  const ai = aiId ? aiLabels[aiId].name : null;
  const running = session?.state === "running" || session?.state === "waiting-approval";
  const n = plugins.length;

  const open = (id: string | null) => navigate(id ? { id: "chat", session: id } : { id: "chat" });
  const onSend = async (text: string) => {
    const { session: id } = await send.mutateAsync({ session: sessionId, text, ai: null });
    if (id !== sessionId) replace({ id: "chat", session: id });
  };
  const onLink = (href: string) => {
    const link = parseCadenceLink(href);
    if (!link) return false;
    navigate({ id: "plugin", plugin: link.plugin, view: link.view, params: link.params });
    return true;
  };

  // Échap arrête le tour en cours.
  useEffect(() => {
    if (!running || !sessionId) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) interrupt.mutate(sessionId);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [running, sessionId, interrupt]);

  return (
    <>
      <ChatBar active={sessionId ?? undefined} onSelect={open} onNew={() => open(null)} />
      <Page
        title={
          <span className="flex items-center gap-[9px]">
            <span className="truncate">{session?.title ?? "Nouvelle conversation"}</span>
            <ContextChip session={session} plugins={plugins} />
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
          <>
            {session && (
              <Menu>
                <MenuTrigger asChild>
                  <Button
                    variant="ghost"
                    iconOnly
                    icon="more"
                    aria-label="Options de la conversation"
                  />
                </MenuTrigger>
                <MenuContent align="end">
                  <MenuItem onSelect={() => setDialog("rename")}>Renommer</MenuItem>
                  <MenuItem onSelect={() => setDialog("delete")}>Supprimer</MenuItem>
                </MenuContent>
              </Menu>
            )}
            <Button icon="plus" onClick={() => open(null)}>
              Nouvelle conversation
            </Button>
          </>
        }
        padding="none"
      >
        <div className="flex min-w-0 grow flex-col">
          <Thread
            key={sessionId ?? "nouvelle"}
            items={sessionId ? thread.items : []}
            loading={sessionId !== null && thread.isPending}
            error={thread.error?.message}
            ai={ai}
            model={session?.model}
            running={running}
            plugins={plugins}
            onApprove={(request, decision) => approve.mutate({ request, decision })}
            onLink={onLink}
          />
          <Composer
            ai={ai}
            running={running}
            sending={send.isPending}
            error={send.error?.message ?? interrupt.error?.message ?? approve.error?.message}
            onSend={onSend}
            onStop={() => sessionId && interrupt.mutate(sessionId)}
          />
        </div>
        <SessionPanel
          session={session}
          ai={ai}
          items={sessionId ? thread.items : []}
          plugins={plugins}
        />
      </Page>
      {session && (
        <>
          <RenameDialog
            session={session}
            open={dialog === "rename"}
            onClose={() => setDialog(null)}
          />
          <DeleteDialog
            session={session}
            open={dialog === "delete"}
            onClose={() => setDialog(null)}
            onDeleted={() => replace({ id: "chat" })}
          />
        </>
      )}
    </>
  );
}

/** Puce de contexte : « Global · tous les plugins », ou le plugin d'où vient la conversation. */
export function ContextChip({
  session,
  plugins = [],
}: {
  session?: Pick<ChatSessionRow, "origin" | "state">;
  plugins?: InstalledPluginView[];
}) {
  return (
    <span className="inline-flex h-[22px] items-center gap-1.5 rounded-md border border-line bg-p2 pr-2 pl-1 font-sans text-tiny font-normal tracking-normal whitespace-nowrap text-tx2">
      <SessionContext
        session={{ origin: session?.origin ?? "user", state: "idle" }}
        plugins={plugins}
      />
    </span>
  );
}

function Thread({
  items,
  loading,
  error,
  ai,
  model,
  running,
  plugins,
  onApprove,
  onLink,
}: {
  items: ChatItem[];
  loading: boolean;
  error?: string;
  ai: string | null;
  model?: string | null;
  running: boolean;
  plugins: InstalledPluginView[];
  onApprove: (request: string, decision: "allow" | "allow-session" | "deny") => void;
  onLink: (href: string) => boolean;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const last = items.at(-1);
  const tail = last ? `${items.length}:${last.kind === "assistant" ? last.text.length : 0}` : "";
  // Suit la réponse, sauf si vous êtes remonté lire plus haut.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  }, [tail, running]);

  return (
    <div
      ref={scroller}
      onScroll={(e) => {
        const el = e.currentTarget;
        pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
      }}
      className="min-h-0 grow overflow-y-auto px-[30px] pt-[22px] pb-2"
    >
      <div className="mx-auto max-w-[780px]">
        {error ? (
          <p className="pt-16 text-center text-body-sm text-red">{error}</p>
        ) : loading ? null : items.length === 0 ? (
          <Welcome ai={ai} plugins={plugins} />
        ) : (
          <ChatThread
            items={items}
            ai={ai ?? "IA"}
            model={model}
            running={running}
            onApprove={onApprove}
            onLink={onLink}
          />
        )}
      </div>
    </div>
  );
}

function Welcome({ ai, plugins }: { ai: string | null; plugins: InstalledPluginView[] }) {
  return (
    <div className="flex flex-col items-center gap-4 pt-16 text-center">
      <span className="inline-flex size-10 items-center justify-center rounded-tile bg-clay text-ink">
        <Icon name="sparkle" size={22} />
      </span>
      <h2 className="font-display text-title font-semibold">
        {ai ? `Discuter avec ${ai}` : "Choisissez une IA de code"}
      </h2>
      <p className="max-w-[520px] text-body-sm leading-[1.6] text-tx2">
        Le chat global voit les outils de tous les plugins activés. L'IA travaille dans un dossier
        neutre de Cadence, jamais dans un de vos dépôts.
      </p>
      {plugins.length > 0 && (
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
  );
}

/** Composeur (V2-Chat) : Entrée envoie, Maj+Entrée va à la ligne ; Arrêter pendant un tour. */
function Composer({
  ai,
  running,
  sending,
  error,
  onSend,
  onStop,
}: {
  ai: string | null;
  running: boolean;
  sending: boolean;
  error?: string;
  onSend: (text: string) => Promise<void>;
  onStop: () => void;
}) {
  const [text, setText] = useState("");
  const area = useRef<HTMLTextAreaElement>(null);
  const canSend = text.trim().length > 0 && !running && !sending && ai !== null;

  // Hauteur ajustée au texte, 8 lignes au plus.
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 8 * 20 + 16)}px`;
  }, [text]);

  const submit = async () => {
    if (!canSend) return;
    const value = text;
    setText("");
    try {
      await onSend(value);
    } catch {
      setText(value);
    }
  };
  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void submit();
    }
  };

  return (
    <div className="shrink-0 px-[30px] pt-3.5 pb-[18px]">
      <div className="mx-auto max-w-[780px]">
        <div className="rounded-card border border-line2 bg-panel">
          <label htmlFor="chat-input" className="sr-only">
            Message pour l'IA
          </label>
          <textarea
            id="chat-input"
            ref={area}
            rows={1}
            value={text}
            autoFocus
            onChange={(e) => setText(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={`Écrire à ${ai ?? "l'IA"}… (Entrée pour envoyer, Maj+Entrée pour aller à la ligne)`}
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
              title="Pièces jointes : bientôt"
              className="inline-flex size-[30px] items-center justify-center rounded-control border border-line text-tx3 disabled:opacity-50"
            >
              <Icon name="paperclip" size={15} />
            </button>
            <div className="grow" />
            {running && <span className="text-caption text-tx3">Échap pour arrêter</span>}
            {running ? (
              <button
                type="button"
                onClick={onStop}
                aria-label="Arrêter"
                title="Arrêter le tour (Échap)"
                className="inline-flex size-8 items-center justify-center rounded-lg border border-line2 bg-p3 text-tx hover:bg-p4"
              >
                <Icon name="stop" size={14} />
              </button>
            ) : (
              <button
                type="button"
                onClick={() => void submit()}
                disabled={!canSend}
                aria-label="Envoyer"
                className="inline-flex size-8 items-center justify-center rounded-lg bg-clay text-clay-ink hover:bg-clay-hover disabled:opacity-50"
              >
                <Icon name="send" size={15} />
              </button>
            )}
          </div>
        </div>
        {error && <p className="mt-2 text-caption text-red">{error}</p>}
      </div>
    </div>
  );
}

/** Panneau droit (290px) : la session, les outils visibles, l'accès aux dépôts. */
function SessionPanel({
  session,
  ai,
  items,
  plugins,
}: {
  session?: ChatSessionRow;
  ai: string | null;
  items: ChatItem[];
  plugins: InstalledPluginView[];
}) {
  const status = usePluginStatus().data ?? [];
  const duration = items.reduce((sum, i) => sum + (i.kind === "turn" ? i.durationMs : 0), 0);
  const cost = items.reduce((sum, i) => sum + (i.kind === "turn" ? (i.costUsd ?? 0) : 0), 0);
  const withTools = plugins.map((p) => ({
    plugin: p,
    tools: status.find((s) => s.plugin === p.id),
  }));
  const total = withTools.reduce((sum, { tools }) => sum + (tools?.tools ?? 0), 0);

  return (
    <aside className="flex w-[290px] shrink-0 flex-col gap-[18px] overflow-y-auto border-l border-line bg-panel px-4 py-[18px]">
      <div>
        <PanelLabel>Session</PanelLabel>
        <div className="mt-2">
          <PanelRow
            label="IA"
            value={ai ? `${ai}${session?.model ? ` · ${session.model}` : ""}` : "—"}
          />
          <PanelRow label="Dossier" value={session ? `sessions/${session.id}/` : "—"} mono />
          <PanelRow label="Passerelle MCP" value="127.0.0.1 · locale" mono />
          <PanelRow label="Durée" value={duration > 0 ? formatDuration(duration) : "—"} />
          <PanelRow label="Coût estimé" value={cost > 0 ? `${cost.toFixed(2)} $` : "—"} last />
        </div>
        <p className="mt-1.5 text-caption leading-[1.55] text-tx3">
          Dossier neutre de Cadence : l'IA n'est jamais lancée dans un de vos dépôts.
        </p>
      </div>
      <div>
        <div className="flex items-center">
          <PanelLabel>Outils visibles</PanelLabel>
          <div className="grow" />
          {total > 0 && <span className="mono text-tiny text-tx3">{total}</span>}
        </div>
        <div className="mt-1">
          {withTools.length === 0 && (
            <p className="py-[7px] text-caption text-tx3">
              Aucun plugin activé : seuls les outils de Cadence.
            </p>
          )}
          {withTools.map(({ plugin, tools }, i) => (
            <div
              key={plugin.id}
              className={
                i < withTools.length - 1
                  ? "flex items-center gap-[9px] border-b border-line py-[7px] text-body-sm"
                  : "flex items-center gap-[9px] py-[7px] text-body-sm"
              }
            >
              <PluginTile color={plugin.color} icon={plugin.railIcon} size={18} />
              <span className="grow text-tx2">{plugin.name}</span>
              <span className="mono text-tiny text-tx3">
                {tools?.state === "ready"
                  ? tools.tools
                  : tools?.state === "failed"
                    ? "en échec"
                    : "—"}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div>
        <PanelLabel>Accès aux dépôts</PanelLabel>
        <div className="mt-2 rounded-field border border-line bg-p2 px-3 py-2.5">
          <div className="flex items-center gap-2 text-body-sm text-tx2">
            <Icon name="branch" size={14} />
            Aucun worktree ouvert
          </div>
          <p className="mt-[5px] text-caption leading-[1.55] text-tx3">
            Un skill qui modifie du code reçoit le worktree de sa tâche, pour sa session seulement.
          </p>
        </div>
      </div>
    </aside>
  );
}

function PanelLabel({ children }: { children: string }) {
  return (
    <div className="text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">{children}</div>
  );
}

function PanelRow({
  label,
  value,
  mono,
  last,
}: {
  label: string;
  value: string;
  mono?: boolean;
  last?: boolean;
}) {
  return (
    <div
      className={`flex justify-between gap-3 py-[7px] text-body-sm ${last ? "" : "border-b border-line"}`}
    >
      <span className="text-tx3">{label}</span>
      <span className={`text-right text-tx2 ${mono ? "mono" : ""}`}>{value}</span>
    </div>
  );
}

function RenameDialog({
  session,
  open,
  onClose,
}: {
  session: ChatSessionRow;
  open: boolean;
  onClose: () => void;
}) {
  const rename = useRenameChat();
  const [title, setTitle] = useState(session.title);
  useEffect(() => {
    if (!open) return;
    setTitle(session.title);
    // Radix place le focus sur le premier bouton (Fermer) : on le rend au champ, texte sélectionné.
    const frame = requestAnimationFrame(() =>
      (document.getElementById("chat-title") as HTMLInputElement | null)?.select(),
    );
    return () => cancelAnimationFrame(frame);
  }, [open, session.title]);
  const save = () => rename.mutate({ id: session.id, title }, { onSuccess: onClose });
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} width={440}>
      <DialogHeader title="Renommer la conversation" icon="pen" />
      <DialogBody>
        <Field label="Titre" htmlFor="chat-title" error={rename.error?.message}>
          <Input
            id="chat-title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && save()}
          />
        </Field>
      </DialogBody>
      <DialogFooter className="justify-end">
        <DialogClose asChild>
          <Button variant="ghost">Annuler</Button>
        </DialogClose>
        <Button variant="primary" onClick={save} disabled={!title.trim() || rename.isPending}>
          Renommer
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function DeleteDialog({
  session,
  open,
  onClose,
  onDeleted,
}: {
  session: ChatSessionRow;
  open: boolean;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const remove = useDeleteChat();
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} width={440}>
      <DialogHeader title="Supprimer la conversation ?" icon="warning" />
      <DialogBody>
        <p className="text-body-sm leading-[1.6] text-tx2">
          « {session.title} » et son historique seront supprimés de cet ordinateur, avec le dossier
          de travail de l'IA. Les données des plugins ne sont pas touchées.
        </p>
        {remove.error && <p className="text-caption text-red">{remove.error.message}</p>}
      </DialogBody>
      <DialogFooter className="justify-end">
        <DialogClose asChild>
          <Button variant="ghost">Annuler</Button>
        </DialogClose>
        <Button
          variant="danger"
          disabled={remove.isPending}
          onClick={() =>
            remove.mutate(session.id, {
              onSuccess: () => {
                onClose();
                onDeleted();
              },
            })
          }
        >
          Supprimer
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
