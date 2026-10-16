/**
 * Réglages (maquette V2-Settings.dc.html) : sections dans la barre contextuelle
 * (Application, Services, Plugins), section choisie à droite.
 *
 * Socle 0.3.0 : préférences de la machine (`settings.json` v2), plugins installés (activation,
 * ordre du rail, réglages et secrets globaux) et compte GitHub de Cadence 1, repris par le
 * service Dépôts. Outils externes et IA par action viendront avec leurs étapes.
 */
import { useState, type ReactNode } from "react";
import type { AiId, CliAgentInfo, InstalledPluginView, PluginSecretInfo } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarItem,
  ContextBarSection,
  ContextBarSubItem,
  ContextBarSubItems,
} from "@/components/layout/ContextBar";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Page } from "@/components/layout/Page";
import { Icon } from "@/components/icons/Icon";
import { SettingRow } from "@/components/domain/Panels";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { SettingsForm } from "@/components/domain/SettingsForm";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { Input, Select } from "@/components/ui/Field";
import { Kbd } from "@/components/ui/Kbd";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { Switch } from "@/components/ui/Switch";
import { aiLabels, aiOrder } from "@/lib/domain/ai";
import { moveInRail, pluginCategories } from "@/lib/domain/plugins";
import { modKey } from "@/lib/hooks/usePlatform";
import {
  useAppSettings,
  useAuthStatus,
  usePlugins,
  usePluginSecret,
  usePluginSettings,
  useReorderPlugins,
  useSaveAppSettings,
  useSavePluginSettings,
  useSetPluginEnabled,
  useSetPluginSecret,
  useSignOut,
} from "@/lib/query/queries";
import { useNavigation, type SettingsSection } from "@/stores/navigation";
import { AboutSection, AppVersionLine } from "./AboutSection";

export default function SettingsView({ view }: ViewProps<"settings">) {
  const section = view.section ?? "general";
  const plugins = usePlugins().data ?? [];
  const plugin = section === "plugin" ? plugins.find((p) => p.id === view.plugin) : undefined;

  return (
    <>
      <SettingsBar section={section} plugin={view.plugin} plugins={plugins} />
      <Page title="Réglages" subtitle="Application, IA de code, services et plugins" padding="none">
        <div className="min-w-0 grow overflow-y-auto px-[26px] py-5">
          <div className="max-w-[780px]">
            {section === "general" && <GeneralSection />}
            {section === "ai" && <AiSection />}
            {section === "repos" && <ReposSection plugins={plugins} />}
            {section === "plugins" && <PluginsSection />}
            {section === "plugin" &&
              (plugin ? (
                <PluginSection key={plugin.id} plugin={plugin} />
              ) : (
                <Callout tone="warning">Le plugin « {view.plugin} » n'est pas installé.</Callout>
              ))}
            {section === "about" && <AboutSection />}
          </div>
        </div>
      </Page>
    </>
  );
}

/* ---------- Barre des sections ---------- */

function SettingsBar({
  section,
  plugin,
  plugins,
}: {
  section: SettingsSection;
  plugin?: string;
  plugins: InstalledPluginView[];
}) {
  const navigate = useNavigation((s) => s.navigate);
  const go = (s: SettingsSection, p?: string) =>
    navigate({ id: "settings", section: s, plugin: p });
  const repoUsers = plugins.filter((p) => p.enabled && p.services.includes("repos"));
  const enabled = plugins.filter((p) => p.enabled);

  return (
    <ContextBar
      label="Sections des réglages"
      header={<ContextBarHeader tile={<NeutralTile icon="settings" />} title="Réglages" />}
      footer={
        <div className="border-t border-line px-2.5 py-[9px] text-tiny leading-[1.6] text-tx3">
          <AppVersionLine onOpen={() => go("about")} /> · API plugins 1
          <br />
          Tauri 2 · React 19
        </div>
      }
    >
      <ContextBarSection title="Application">
        <ContextBarItem
          icon="settings"
          label="Général"
          active={section === "general"}
          onClick={() => go("general")}
        />
        <ContextBarItem
          icon="sparkle"
          label="IA de code"
          active={section === "ai"}
          onClick={() => go("ai")}
        />
        <ContextBarItem
          icon="rocket"
          label="À propos"
          active={section === "about"}
          onClick={() => go("about")}
        />
      </ContextBarSection>
      <ContextBarSection title="Services">
        <ContextBarItem
          icon="github"
          label="Dépôts GitHub"
          active={section === "repos"}
          onClick={() => go("repos")}
        />
        {repoUsers.length > 0 && (
          <ContextBarSubItems>
            {repoUsers.map((p) => (
              <ContextBarSubItem
                key={p.id}
                leading={<PluginTile color={p.color} icon={p.railIcon} size={14} />}
                label={`utilisé par ${p.name}`}
                onClick={() => go("repos")}
              />
            ))}
          </ContextBarSubItems>
        )}
      </ContextBarSection>
      <ContextBarSection title="Plugins">
        <ContextBarItem
          icon="grid"
          label="Plugins activés"
          active={section === "plugins"}
          onClick={() => go("plugins")}
        />
        {enabled.length > 0 && (
          <ContextBarSubItems>
            {enabled.map((p) => (
              <ContextBarSubItem
                key={p.id}
                leading={<PluginTile color={p.color} icon={p.railIcon} size={14} />}
                label={p.name}
                active={section === "plugin" && plugin === p.id}
                onClick={() => go("plugin", p.id)}
              />
            ))}
          </ContextBarSubItems>
        )}
      </ContextBarSection>
    </ContextBar>
  );
}

function SectionTitle({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <>
      <h2 className="font-display text-title font-semibold">{title}</h2>
      {children && <p className="mt-[7px] text-body-sm leading-[1.6] text-tx2">{children}</p>}
    </>
  );
}

/* ---------- Général ---------- */

const shortcuts: [string, ReactNode][] = [
  ["Palette de commandes", <Kbd key="k">{modKey}K</Kbd>],
  ["Fermer une fenêtre", <Kbd key="esc">Échap</Kbd>],
  ["Ouvrir la ligne choisie", <Kbd key="enter">Entrée</Kbd>],
];

function GeneralSection() {
  return (
    <>
      <SectionTitle title="Général">
        Réglages de l'application. Les plugins activés le sont pour toute l'app ; leurs réglages
        sont dans la section Plugins.
      </SectionTitle>
      <div className="mt-[18px]">
        <SettingRow
          title="Thème"
          description="Cadence est dessinée pour un thème sombre unique, aux couleurs des maquettes."
          control={<Badge tone="muted">Sombre</Badge>}
        />
        <SettingRow title="Langue de l'interface" control={<Badge tone="muted">Français</Badge>} />
        {shortcuts.map(([label, keys]) => (
          <SettingRow key={label} title={label} control={keys} />
        ))}
      </div>
    </>
  );
}

/* ---------- IA de code ---------- */

function AiSection() {
  const settings = useAppSettings();
  const save = useSaveAppSettings();
  const auth = useAuthStatus().data;
  const detected: Partial<Record<AiId, CliAgentInfo>> = {
    "claude-code": auth?.claudeCode,
    codex: auth?.codex,
  };
  if (settings.isPending) return <LoadingState />;
  if (settings.isError) return <ErrorState error={settings.error} />;
  const s = settings.data;

  return (
    <>
      <SectionTitle title="IA de code">
        Cadence pilote les CLI installées avec vos abonnements. L'IA par défaut sert au chat et à
        toutes les actions, sauf celles qu'un plugin ou vous réaffectez.
      </SectionTitle>
      <div className="mt-[18px]">
        <SettingRow
          title="IA par défaut"
          description="Lancée pour une nouvelle conversation et pour les actions des plugins."
          control={
            <Select
              size="sm"
              aria-label="IA par défaut"
              value={s.defaultAi ?? ""}
              disabled={save.isPending}
              onChange={(e) => save.mutate({ defaultAi: e.target.value as AiId })}
              className="w-[170px]"
            >
              {!s.defaultAi && <option value="">À choisir</option>}
              {aiOrder.map((ai) => (
                <option key={ai} value={ai}>
                  {aiLabels[ai].name}
                </option>
              ))}
            </Select>
          }
        />
      </div>
      <div className="mt-[18px] flex flex-col gap-3">
        {aiOrder.map((ai) => (
          <AiRow
            key={ai}
            ai={ai}
            info={detected[ai]}
            enabled={s.ai[ai === "claude-code" ? "claudeCode" : ai].enabled}
            path={s.ai[ai === "claude-code" ? "claudeCode" : ai].path}
            onEnabled={(enabled) => save.mutate({ ai: [{ ai, enabled }] })}
          />
        ))}
      </div>
      {save.isError && (
        <Callout tone="danger" className="mt-3">
          {save.error.message}
        </Callout>
      )}
    </>
  );
}

function AiRow({
  ai,
  info,
  enabled,
  path,
  onEnabled,
}: {
  ai: AiId;
  info?: CliAgentInfo;
  enabled: boolean;
  path: string | null;
  onEnabled: (enabled: boolean) => void;
}) {
  const ok = info?.status === "connected";
  const state = !enabled
    ? "Ignorée"
    : !info
      ? "Détection à venir"
      : ok
        ? "Connectée"
        : info.status === "not-found"
          ? "Introuvable"
          : "À vérifier";
  return (
    <Card className="flex items-center gap-3 px-4 py-3">
      <StatusDot
        tone={
          !enabled ? "neutral" : ok ? "green" : info?.status === "not-found" ? "neutral" : "amber"
        }
        size={8}
      />
      <div className="min-w-0 grow">
        <div className="text-body font-medium">
          {aiLabels[ai].name}
          {info?.version && <span className="ml-2 mono text-caption text-tx3">{info.version}</span>}
        </div>
        <div className="truncate mono text-caption text-tx3">
          {path ?? info?.binaryPath ?? "chemin détecté au lancement"}
        </div>
      </div>
      {info?.authMethod && <Badge tone="muted">{info.authMethod}</Badge>}
      <span className="text-caption text-tx2">{state}</span>
      <Switch
        checked={enabled}
        onCheckedChange={onEnabled}
        aria-label={`Proposer ${aiLabels[ai].name}`}
      />
    </Card>
  );
}

/* ---------- Dépôts GitHub (compte de Cadence 1, repris par le service) ---------- */

function ReposSection({ plugins }: { plugins: InstalledPluginView[] }) {
  const auth = useAuthStatus().data;
  const signOut = useSignOut();
  const github = auth?.github;
  const name = github?.name ?? github?.login ?? "Compte GitHub";
  const users = plugins.filter((p) => p.services.includes("repos"));
  return (
    <>
      <SectionTitle title="Dépôts GitHub">
        Service de l'app activé par les plugins qui le déclarent ({" "}
        <span className="mono">services = ["repos"]</span>) : connexion GitHub, dépôts suivis par
        plugin. Le jeton vit dans le trousseau du système, jamais dans l'interface.
      </SectionTitle>
      {users.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {users.map((p) => (
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
      <Card className="mt-[18px] px-4 py-3.5">
        <div className="flex items-center gap-3">
          <Avatar size={26} initials={initialsOf(name)} src={github?.avatarUrl} />
          <div className="min-w-0 grow">
            <div className="truncate text-body font-semibold">{name}</div>
            {github?.login && <div className="mono text-caption text-tx3">@{github.login}</div>}
          </div>
          <StatusDot tone={github?.status === "connected" ? "green" : "amber"} size={8} />
          <span className="text-caption text-tx2">
            {github?.status === "connected" ? "Connecté" : "Non connecté"}
          </span>
        </div>
        {github && github.scopes.length > 0 && (
          <div className="mt-3 flex flex-wrap items-center gap-1.5">
            <span className="text-caption text-tx3">Portées du jeton :</span>
            {github.scopes.map((s) => (
              <Badge key={s} tone="muted" mono>
                {s}
              </Badge>
            ))}
          </div>
        )}
        <div className="mt-3.5 flex items-center gap-2.5 border-t border-line pt-3">
          <span className="grow text-caption text-tx3">
            Se déconnecter retire le jeton du trousseau et efface le cache local des dépôts.
          </span>
          <Button
            variant="danger"
            size="sm"
            disabled={signOut.isPending}
            onClick={() => signOut.mutate()}
          >
            Se déconnecter
          </Button>
        </div>
      </Card>
    </>
  );
}

/* ---------- Plugins activés ---------- */

function PluginsSection() {
  const plugins = usePlugins();
  const reorder = useReorderPlugins();
  const setEnabled = useSetPluginEnabled();
  const navigate = useNavigation((s) => s.navigate);
  if (plugins.isPending) return <LoadingState />;
  if (plugins.isError) return <ErrorState error={plugins.error} />;
  const all = plugins.data;

  return (
    <>
      <SectionTitle title="Plugins activés">
        Un plugin installé est activé pour toute l'app. Le rail les range par catégorie (fixée par
        le manifeste) ; dans une catégorie, l'ordre est le vôtre.
      </SectionTitle>
      {all.length === 0 ? (
        <Callout tone="info" className="mt-[18px]">
          Aucun plugin installé. Le catalogue public arrive avec la prochaine étape.
        </Callout>
      ) : (
        <Card className="mt-[18px] overflow-hidden">
          {all.map((p, i) => {
            const up = moveInRail(all, p.id, -1);
            const down = moveInRail(all, p.id, 1);
            return (
              <div
                key={p.id}
                className={`flex items-center gap-3 px-4 py-2.5 ${i > 0 ? "border-t border-line" : ""}`}
              >
                <PluginTile color={p.color} icon={p.railIcon} size={22} />
                <button
                  type="button"
                  onClick={() => navigate({ id: "settings", section: "plugin", plugin: p.id })}
                  className="min-w-0 grow text-left"
                >
                  <div className="truncate text-body font-medium text-tx">{p.name}</div>
                  <div className="truncate text-caption text-tx3">
                    {pluginCategories[p.category].label} · v{p.version}
                    {p.source === "dev" && " · mode développement"}
                  </div>
                </button>
                {p.status === "needs-setup" && <Badge tone="amber">À configurer</Badge>}
                {p.status === "invalid" && <Badge tone="red">Invalide</Badge>}
                <Button
                  variant="quiet"
                  size="xs"
                  iconOnly
                  aria-label={`Monter ${p.name}`}
                  disabled={!up || reorder.isPending}
                  onClick={() => up && reorder.mutate(up)}
                >
                  <Icon name="arrowRight" size={13} className="-rotate-90" />
                </Button>
                <Button
                  variant="quiet"
                  size="xs"
                  iconOnly
                  aria-label={`Descendre ${p.name}`}
                  disabled={!down || reorder.isPending}
                  onClick={() => down && reorder.mutate(down)}
                >
                  <Icon name="arrowRight" size={13} className="rotate-90" />
                </Button>
                <Switch
                  checked={p.enabled}
                  disabled={p.status === "invalid"}
                  onCheckedChange={(enabled) => setEnabled.mutate({ id: p.id, enabled })}
                  aria-label={`Activer ${p.name}`}
                />
              </div>
            );
          })}
        </Card>
      )}
      {(reorder.isError || setEnabled.isError) && (
        <Callout tone="danger" className="mt-3">
          {(reorder.error ?? setEnabled.error)?.message}
        </Callout>
      )}
    </>
  );
}

/* ---------- Un plugin : activation, secrets, réglages ---------- */

function PluginSection({ plugin }: { plugin: InstalledPluginView }) {
  const setEnabled = useSetPluginEnabled();
  const settings = usePluginSettings(plugin.hasSettings ? plugin.id : null);
  const save = useSavePluginSettings(plugin.id);

  return (
    <>
      <div className="flex items-center gap-3">
        <PluginTile color={plugin.color} icon={plugin.railIcon} />
        <div className="min-w-0 grow">
          <h2 className="font-display text-title font-semibold">{plugin.name}</h2>
          <div className="mono text-caption text-tx3">
            {plugin.id} · v{plugin.version} ·{" "}
            {{ dev: "mode développement", file: "archive locale", catalog: "catalogue" }[plugin.source]}
          </div>
        </div>
        <span className="text-caption text-tx2">{plugin.enabled ? "Activé" : "Désactivé"}</span>
        <Switch
          checked={plugin.enabled}
          disabled={plugin.status === "invalid"}
          onCheckedChange={(enabled) => setEnabled.mutate({ id: plugin.id, enabled })}
          aria-label={`Activer ${plugin.name}`}
        />
      </div>
      {plugin.description && (
        <p className="mt-3 text-body-sm leading-[1.6] text-tx2">{plugin.description}</p>
      )}
      {plugin.problem && (
        <Callout tone={plugin.status === "invalid" ? "danger" : "warning"} className="mt-3">
          {plugin.problem}
        </Callout>
      )}

      {plugin.secrets.length > 0 && (
        <>
          <h3 className="mt-6 text-body font-semibold">Secrets</h3>
          <p className="mt-1 text-caption leading-normal text-tx3">
            Rangés dans le trousseau du système, transmis seulement au serveur du plugin. Cadence ne
            les réaffiche jamais.
          </p>
          <div className="mt-2">
            {plugin.secrets.map((s) => (
              <SecretRow key={s.id} plugin={plugin.id} secret={s} />
            ))}
          </div>
        </>
      )}

      <h3 className="mt-6 text-body font-semibold">Réglages</h3>
      <p className="mt-1 text-caption leading-normal text-tx3">
        Globaux : ils valent pour tout ce que fait le plugin, et lui sont transmis au démarrage.
      </p>
      <div className="mt-2">
        {!plugin.hasSettings ? (
          <p className="text-body-sm text-tx3">Ce plugin ne déclare aucun réglage.</p>
        ) : settings.isPending ? (
          <LoadingState />
        ) : settings.isError ? (
          <ErrorState error={settings.error} />
        ) : (
          <SettingsForm
            settings={settings.data}
            saving={save.isPending}
            errors={save.data?.errors}
            onSave={(value) => save.mutate(value)}
          />
        )}
        {save.isSuccess && save.data.errors.length === 0 && (
          <p className="mt-2 text-caption text-green">Réglages enregistrés.</p>
        )}
        {save.isError && (
          <Callout tone="danger" className="mt-3">
            {save.error.message}
          </Callout>
        )}
      </div>
    </>
  );
}

function SecretRow({ plugin, secret }: { plugin: string; secret: PluginSecretInfo }) {
  const has = usePluginSecret(plugin, secret.id);
  const set = useSetPluginSecret(plugin, secret.id);
  const [value, setValue] = useState("");
  const stored = has.data === true;

  return (
    <SettingRow
      title={
        <span className="flex items-center gap-2">
          {secret.label}
          {secret.required && <Badge tone={stored ? "muted" : "amber"}>requis</Badge>}
        </span>
      }
      description={
        <>
          {secret.description}
          {set.isError && <span className="block text-red">{set.error.message}</span>}
        </>
      }
      control={
        stored ? (
          <>
            <span className="inline-flex items-center gap-1.5 text-caption text-green">
              <Icon name="check" size={13} /> Enregistré
            </span>
            <Button
              variant="ghost"
              size="sm"
              disabled={set.isPending}
              onClick={() => set.mutate(null)}
            >
              Supprimer
            </Button>
          </>
        ) : (
          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              set.mutate(value, { onSuccess: () => setValue("") });
            }}
          >
            <Input
              size="sm"
              type="password"
              autoComplete="off"
              aria-label={secret.label}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              className="w-[200px]"
            />
            <Button type="submit" size="sm" disabled={!value.trim() || set.isPending}>
              {set.isPending && <Spinner />}
              Enregistrer
            </Button>
          </form>
        )
      }
    />
  );
}
