/**
 * Barre du plugin actif (V2-Project-Issue.dc.html) : tuile et nom, options, sélecteur du
 * service Dépôts pour un plugin qui le déclare, vues groupées par section, conversations
 * ouvertes depuis le plugin.
 */
import { Icon } from "@/components/icons/Icon";
import { PluginTile } from "@/components/domain/PluginTile";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/Menu";
import { homeView, kitIcon, moveInRail, viewSections } from "@/lib/domain/plugins";
import { conversationTime, conversationOriginLabel, pluginConversations } from "@/lib/domain/chat";
import { Spinner } from "@/components/ui/Spinner";
import {
  useAppSettings,
  useChatSessions,
  usePlugins,
  useReorderPlugins,
  useRunAction,
  useSaveAppSettings,
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
  ConversationItem,
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
      {plugin.services.includes("repos") && <RepoSelector plugin={plugin.id} />}
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
      <PluginConversations plugin={plugin} />
    </ContextBar>
  );
}

/**
 * Conversations ouvertes depuis le plugin (ses actions, ou « + ») hors groupes cachés (celles
 * d'un sprint restent dans sa vue), les plus récentes d'abord ; « + » ouvre une nouvelle
 * conversation avec la puce du plugin.
 */
function PluginConversations({ plugin }: { plugin: InstalledPluginView }) {
  const navigate = useNavigation((s) => s.navigate);
  const rows = pluginConversations(useChatSessions().data ?? [], plugin.id);
  return (
    <ContextBarSection
      title="Conversations"
      action={
        <ContextBarIconButton
          icon="plus"
          label={`Nouvelle conversation avec ${plugin.name}`}
          size={20}
          onClick={() => navigate({ id: "chat", plugin: plugin.id })}
        />
      }
    >
      {rows.length === 0 && (
        <ContextBarHint>
          Les conversations ouvertes depuis {plugin.name} apparaîtront ici ; celles d'un sprint
          restent dans sa vue.
        </ContextBarHint>
      )}
      {rows.map((row) => (
        <ConversationItem
          key={row.id}
          title={row.title}
          time={conversationTime(row.updatedAt)}
          onClick={() => navigate({ id: "chat", session: row.id })}
          context={
            <>
              <span className="truncate">{conversationOriginLabel(row.origin, plugin)}</span>
              {row.state === "running" && (
                <span className="ml-auto flex shrink-0 text-clay">
                  <Spinner size={11} />
                </span>
              )}
              {row.state === "waiting-approval" && (
                <span className="ml-auto flex shrink-0 text-clay" title="Attend votre accord">
                  <Icon name="warning" size={11} />
                </span>
              )}
              {row.state === "waiting-reply" && (
                <span
                  className="ml-auto flex shrink-0 text-clay"
                  title="En attente de votre réponse"
                  aria-label="En attente de votre réponse"
                >
                  <Icon name="chat" size={11} />
                </span>
              )}
            </>
          }
        />
      ))}
    </ContextBarSection>
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

/** Valeur de « Tous les dépôts » dans le menu (un dépôt est toujours `owner/name`). */
const ALL_REPOS = "*";

/**
 * Sélecteur « un dépôt » ou « Tous les dépôts » du service `repos` (V2-Repo-Selector) : le
 * choix est gardé dans les réglages (`repoSelection`) et transmis à l'iframe du plugin
 * (`repos.selection`, événement `repos`).
 */
function RepoSelector({ plugin }: { plugin: string }) {
  const navigate = useNavigation((s) => s.navigate);
  const settings = useAppSettings().data;
  const save = useSaveAppSettings();
  const repos = settings?.trackedRepos[plugin] ?? [];
  const selected = settings?.repoSelection[plugin] ?? null;
  const choose = (value: string) =>
    save.mutate({ repoSelection: { [plugin]: value === ALL_REPOS ? null : value } });

  return (
    <Menu>
      <MenuTrigger asChild>
        <button
          type="button"
          aria-label="Choisir le dépôt"
          className="flex h-9 shrink-0 items-center gap-2 rounded-lg border border-line2 bg-p2 px-2 text-tx hover:bg-p3"
        >
          <span
            aria-hidden="true"
            className="inline-flex size-5 shrink-0 items-center justify-center rounded-md bg-p3 text-tx3"
          >
            <Icon name="github" size={12} />
          </span>
          <span className="min-w-0 grow truncate text-left text-body-sm font-medium">
            {selected ?? "Tous les dépôts"}
          </span>
          <span className="flex text-tx3">
            <Icon name="chevronDown" size={14} />
          </span>
        </button>
      </MenuTrigger>
      <MenuContent className="w-[260px]">
        <MenuLabel>Dépôts suivis</MenuLabel>
        <MenuRadioGroup value={selected ?? ALL_REPOS} onValueChange={choose}>
          <MenuRadioItem value={ALL_REPOS}>Tous les dépôts</MenuRadioItem>
          {repos.map((repo) => (
            <MenuRadioItem key={repo} value={repo}>
              <span className="min-w-0 truncate">
                <span className="text-tx3">{repo.split("/")[0]}/</span>
                {repo.split("/")[1]}
              </span>
            </MenuRadioItem>
          ))}
        </MenuRadioGroup>
        {repos.length === 0 && (
          <div className="px-2.5 py-1.5 text-caption text-tx3">Aucun dépôt suivi.</div>
        )}
        <MenuSeparator />
        <MenuItem onSelect={() => navigate({ id: "settings", section: "repos" })}>
          <Icon name="settings" size={14} /> Gérer les dépôts suivis…
        </MenuItem>
      </MenuContent>
    </Menu>
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
