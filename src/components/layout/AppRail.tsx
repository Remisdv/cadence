/**
 * Rail de navigation de Cadence 2 (56px, V2-Chat.dc.html) : logo, chat global, plugins
 * activés groupés par catégorie, puis catalogue, réglages et IA par défaut en pied.
 */
import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { PluginTile } from "@/components/domain/PluginTile";
import { Tooltip } from "@/components/ui/Tooltip";
import { cn } from "@/lib/cn";
import { aiLabels, aiRailState } from "@/lib/domain/ai";
import { toolOfAi } from "@/lib/domain/tools";
import { homeView, pluginCategories, railGroups } from "@/lib/domain/plugins";
import { useAppSettings, useChatSessions, usePlugins, useTools } from "@/lib/query/queries";
import type { InstalledPluginView } from "@/lib/types";
import { useNavigation, type View } from "@/stores/navigation";

/** Entrée du rail allumée pour la vue courante. */
export function railKey(view: View): string {
  switch (view.id) {
    case "chat":
      return "chat";
    case "plugin":
      return `plugin:${view.plugin}`;
    case "catalog":
      return "catalog";
    case "settings":
      return "settings";
    default:
      return "";
  }
}

export function AppRail() {
  const view = useNavigation((s) => s.view);
  const navigate = useNavigation((s) => s.navigate);
  const plugins = usePlugins().data ?? [];
  const active = railKey(view);
  // Une conversation (même rangée dans un sprint) attend un accord ou une réponse.
  const waiting = (useChatSessions().data ?? []).filter((s) => s.state === "waiting-approval");

  const openPlugin = (p: InstalledPluginView) => {
    const home = homeView(p);
    if (home) navigate({ id: "plugin", plugin: p.id, view: home });
    else navigate({ id: "settings", section: "plugin", plugin: p.id });
  };

  return (
    <nav
      aria-label="Navigation principale"
      className="relative z-[2] flex w-14 shrink-0 flex-col items-center gap-1 border-r border-line bg-bg py-3"
    >
      <div className="flex h-[34px] w-14 shrink-0 items-start justify-center">
        <span
          aria-hidden="true"
          className="inline-flex size-7 items-center justify-center rounded-lg bg-clay text-clay-ink"
        >
          <Icon name="sprint" size={17} />
        </span>
      </div>

      <div className="flex min-h-0 w-14 grow [scrollbar-width:none] flex-col items-center gap-1 overflow-y-auto">
        <RailButton
          label="Chat"
          detail={
            waiting.length > 0
              ? `${waiting.length} conversation${waiting.length > 1 ? "s attendent" : " attend"} votre réponse`
              : "Chat global · tous les plugins"
          }
          active={active === "chat"}
          onClick={() =>
            navigate(
              waiting.length === 1 ? { id: "chat", session: waiting[0]!.id } : { id: "chat" },
            )
          }
          dot={waiting.length > 0 ? "amber" : undefined}
        >
          <Icon name="chat" size={19} />
        </RailButton>

        {railGroups(plugins).map((group) => (
          <div key={group.category} className="flex flex-col items-center gap-1">
            <RailCategory label={group.label} />
            {group.plugins.map((p) => (
              <RailButton
                key={p.id}
                label={p.name}
                detail={pluginDetail(p)}
                active={active === `plugin:${p.id}`}
                onClick={() => openPlugin(p)}
                dot={p.status === "needs-setup" ? "amber" : undefined}
              >
                <PluginTile color={p.color} icon={p.railIcon} />
              </RailButton>
            ))}
          </div>
        ))}
      </div>

      <div className="flex flex-col items-center gap-1">
        <RailButton
          label="Catalogue de plugins"
          active={active === "catalog"}
          onClick={() => navigate({ id: "catalog" })}
        >
          <Icon name="grid" size={18} />
        </RailButton>
        <RailButton
          label="Réglages"
          active={active === "settings"}
          onClick={() => navigate({ id: "settings" })}
        >
          <Icon name="settings" size={18} />
        </RailButton>
        <div aria-hidden="true" className="flex w-14 justify-center py-1">
          <span className="w-6 border-t border-line2" />
        </div>
        <DefaultAiButton onClick={() => navigate({ id: "settings", section: "ai" })} />
      </div>
    </nav>
  );
}

function pluginDetail(p: InstalledPluginView): string {
  const category = pluginCategories[p.category].label;
  if (p.status === "needs-setup") return `${category} · à configurer`;
  if (p.source === "dev") return `${category} · mode développement`;
  return category;
}

/** Séparateur de catégorie : trait de 24px et libellé 8.5px en capitales. */
export function RailCategory({ label }: { label: string }) {
  return (
    <div
      aria-hidden="true"
      className="flex w-14 shrink-0 flex-col items-center gap-[5px] pt-[7px] pb-0.5"
    >
      <span className="w-6 border-t border-line2" />
      <span className="text-nano font-semibold tracking-[0.06em] text-tx3 uppercase">{label}</span>
    </div>
  );
}

export interface RailButtonProps {
  label: string;
  /** Seconde ligne de l'infobulle (catégorie, état). */
  detail?: string;
  active?: boolean;
  onClick?: () => void;
  /** Pastille en bas à droite (plugin à configurer, exécution en fond). */
  dot?: "amber" | "purple" | "green";
  children: ReactNode;
}

/** Bouton carré 40px du rail ; l'entrée active porte une barre clay de 3px à gauche. */
export function RailButton({ label, detail, active, onClick, dot, children }: RailButtonProps) {
  return (
    <div className="relative flex h-10 w-14 shrink-0 justify-center">
      {active && (
        <span
          aria-hidden="true"
          className="absolute top-2.5 left-0 h-5 w-[3px] rounded-r-[3px] bg-clay"
        />
      )}
      <Tooltip title={label} detail={detail}>
        <button
          type="button"
          aria-label={detail ? `${label} · ${detail}` : label}
          aria-current={active ? "page" : undefined}
          onClick={onClick}
          className={cn(
            "inline-flex size-10 items-center justify-center rounded-tile border transition-colors",
            active
              ? "border-line2 bg-p3 text-tx"
              : "border-transparent text-tx2 hover:border-line hover:bg-p2",
          )}
        >
          {children}
        </button>
      </Tooltip>
      {dot && (
        <span
          aria-hidden="true"
          className={cn(
            "absolute right-1.5 -bottom-px size-2.5 rounded-full border-2 border-bg",
            dotClass[dot],
          )}
        />
      )}
    </div>
  );
}

const dotClass = { amber: "bg-amber", purple: "bg-purple", green: "bg-green" } as const;

const aiIcon: IconName = "sparkle";

/**
 * IA par défaut, avec son état (pastille verte : connectée ; ambre : à installer, connecter ou
 * mettre à jour), lu dans le rapport d'outils comme l'onboarding.
 */
function DefaultAiButton({ onClick }: { onClick: () => void }) {
  const ai = useAppSettings().data?.defaultAi ?? null;
  const report = useTools().data;
  const tool = ai ? report?.tools.find((t) => t.id === toolOfAi[ai]) : undefined;
  const state = aiRailState(ai, tool);
  const name = ai ? aiLabels[ai].name : "Aucune IA choisie";
  return (
    <RailButton label={`IA par défaut : ${name}`} detail={state.label} onClick={onClick}>
      <span className="relative inline-flex">
        <span
          aria-hidden="true"
          className={cn(
            "inline-flex size-[26px] items-center justify-center rounded-control text-ink",
            ai ? "bg-clay" : "bg-p4 text-tx3",
          )}
        >
          <Icon name={aiIcon} size={16} />
        </span>
        <span
          aria-hidden="true"
          className={cn(
            "absolute -right-[3px] -bottom-[3px] size-2.5 rounded-full border-2 border-bg",
            state.tone === "green" ? "bg-green" : state.tone === "amber" ? "bg-amber" : "bg-tx3",
          )}
        />
      </span>
    </RailButton>
  );
}
