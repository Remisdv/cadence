/**
 * Barre du plugin actif (V2-Project-Issue.dc.html) : tuile et nom, options, sélecteur du
 * service Dépôts pour un plugin qui le déclare, vues groupées par section, conversations
 * ouvertes depuis le plugin.
 */
import { Icon } from "@/components/icons/Icon";
import { PluginTile } from "@/components/domain/PluginTile";
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "@/components/ui/Menu";
import { homeView, kitIcon, moveInRail, viewSections } from "@/lib/domain/plugins";
import {
  usePlugins,
  useReorderPlugins,
  useRunAction,
  useSetPluginEnabled,
} from "@/lib/query/queries";
import { useUiStore } from "@/stores/ui";
import type { InstalledPluginView } from "@/lib/types";
import { useNavigation } from "@/stores/navigation";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarHint,
  ContextBarIconButton,
  ContextBarItem,
  ContextBarSection,
} from "./ContextBar";

export function PluginBar({ plugin, view }: { plugin: InstalledPluginView; view: string }) {
  const navigate = useNavigation((s) => s.navigate);
  const sections = viewSections(plugin.views);

  return (
    <ContextBar
      label="Barre du plugin"
      header={
        <ContextBarHeader
          tile={<PluginTile color={plugin.color} icon={plugin.railIcon} size={22} />}
          title={plugin.name}
          action={<PluginOptions plugin={plugin} />}
        />
      }
    >
      {plugin.services.includes("repos") && <RepoSelector />}
      {plugin.status === "needs-setup" && (
        <button
          type="button"
          onClick={() => navigate({ id: "settings", section: "plugin", plugin: plugin.id })}
          className="flex items-start gap-2 rounded-field border border-amber-line bg-amber-d px-2.5 py-2 text-left text-caption leading-[1.5] text-amber"
        >
          <Icon name="warning" size={14} className="mt-px" />
          <span>
            À configurer :{" "}
            {plugin.secrets
              .filter((s) => s.required)
              .map((s) => s.label)
              .join(", ")}
          </span>
        </button>
      )}

      {sections.map((section) => (
        <ContextBarSection key={section.title ?? ""} title={section.title}>
          {section.views.map((v) => (
            <ContextBarItem
              key={v.id}
              icon={kitIcon(v.icon)}
              label={v.title}
              active={v.id === view}
              onClick={() => navigate({ id: "plugin", plugin: plugin.id, view: v.id })}
            />
          ))}
        </ContextBarSection>
      ))}

      <PluginActions plugin={plugin} />

      <ContextBarSection
        title="Conversations"
        action={
          <ContextBarIconButton
            icon="plus"
            label="Nouvelle conversation (arrive avec le chat)"
            size={20}
            disabled
          />
        }
      >
        <ContextBarHint>
          Les conversations ouvertes depuis {plugin.name} apparaîtront ici, avec leur contexte.
        </ContextBarHint>
      </ContextBarSection>
    </ContextBar>
  );
}

/**
 * Actions globales du plugin (`[[actions]]` sans contexte) : un clic lance le skill, en
 * conversation (mode chat) ou en fond (visible dans le groupe « En fond » du chat).
 */
function PluginActions({ plugin }: { plugin: InstalledPluginView }) {
  const run = useRunAction();
  const navigate = useNavigation((s) => s.navigate);
  const actions = plugin.actions.filter((a) => a.context === null);
  if (actions.length === 0) return null;
  const launch = (action: (typeof actions)[number]) => {
    if (action.confirm && !window.confirm(action.confirm)) return;
    run.mutate(
      { plugin: plugin.id, action: action.id, context: null },
      {
        onSuccess: (result) => {
          if (result.mode === "chat") navigate({ id: "chat", session: result.session });
          else
            useUiStore.getState().pushToast({
              message: `« ${action.label} » tourne en fond : suivez-la dans le chat.`,
              kind: "info",
              source: plugin.name,
            });
        },
        onError: (error) =>
          useUiStore
            .getState()
            .pushToast({ message: error.message, kind: "error", source: plugin.name }),
      },
    );
  };
  return (
    <ContextBarSection title="Actions">
      {actions.map((action) => (
        <ContextBarItem
          key={action.id}
          icon={action.icon ? kitIcon(action.icon) : "sparkle"}
          label={action.mode === "background" ? `${action.label} · en fond` : action.label}
          onClick={() => launch(action)}
        />
      ))}
    </ContextBarSection>
  );
}

/**
 * Sélecteur « un dépôt » ou « Tous les dépôts » du service `repos` (V2-Repo-Selector) : en
 * attente du service, il montre seulement sa place dans la barre.
 */
function RepoSelector() {
  return (
    <button
      type="button"
      disabled
      aria-label="Choisir le dépôt"
      title="Le service Dépôts arrive avec la connexion GitHub de Cadence 2."
      className="flex h-9 shrink-0 items-center gap-2 rounded-lg border border-line2 bg-p2 px-2 text-tx disabled:cursor-default"
    >
      <span
        aria-hidden="true"
        className="inline-flex size-5 shrink-0 items-center justify-center rounded-md bg-p3 text-tx3"
      >
        <Icon name="github" size={12} />
      </span>
      <span className="min-w-0 grow truncate text-left text-body-sm font-medium">
        Tous les dépôts
      </span>
      <span className="flex text-tx3">
        <Icon name="chevronDown" size={14} />
      </span>
    </button>
  );
}

function PluginOptions({ plugin }: { plugin: InstalledPluginView }) {
  const navigate = useNavigation((s) => s.navigate);
  const plugins = usePlugins().data ?? [];
  const reorder = useReorderPlugins();
  const setEnabled = useSetPluginEnabled();
  const up = moveInRail(plugins, plugin.id, -1);
  const down = moveInRail(plugins, plugin.id, 1);
  const home = homeView(plugin);

  return (
    <Menu>
      <MenuTrigger asChild>
        <ContextBarIconButton icon="more" label={`Options de ${plugin.name}`} />
      </MenuTrigger>
      <MenuContent align="end">
        {home && (
          <MenuItem onSelect={() => navigate({ id: "plugin", plugin: plugin.id, view: home })}>
            <Icon name="sprint" size={14} /> Vue d'accueil
          </MenuItem>
        )}
        <MenuItem disabled={!up} onSelect={() => up && reorder.mutate(up)}>
          <Icon name="arrowRight" size={14} className="-rotate-90" /> Monter dans le rail
        </MenuItem>
        <MenuItem disabled={!down} onSelect={() => down && reorder.mutate(down)}>
          <Icon name="arrowRight" size={14} className="rotate-90" /> Descendre dans le rail
        </MenuItem>
        <MenuSeparator />
        <MenuItem
          onSelect={() => navigate({ id: "settings", section: "plugin", plugin: plugin.id })}
        >
          <Icon name="settings" size={14} /> Réglages du plugin
        </MenuItem>
        <MenuItem
          onSelect={() =>
            setEnabled.mutate(
              { id: plugin.id, enabled: false },
              { onSuccess: () => navigate({ id: "chat" }) },
            )
          }
        >
          <Icon name="close" size={14} /> Désactiver le plugin
        </MenuItem>
      </MenuContent>
    </Menu>
  );
}
