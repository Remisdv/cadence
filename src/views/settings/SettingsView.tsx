/**
 * Réglages (maquette V2-Settings.dc.html) : sections dans la barre contextuelle (Application,
 * Outils, Services, Plugins), section choisie à droite.
 *
 * Préférences de la machine (`settings.json` v2) : IA par défaut et par action, outils externes
 * (registre d'outils), service Dépôts GitHub (compte, dépôts suivis), plugins installés
 * (activation, ordre du rail, réglages et secrets globaux).
 */
import { useState } from "react";
import type { InstalledPluginView, PluginSecretInfo } from "@/lib/types";
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
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { Input } from "@/components/ui/Field";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { Switch } from "@/components/ui/Switch";
import { moveInRail, pluginCategories } from "@/lib/domain/plugins";
import { blocksContinue, isRequested } from "@/lib/domain/tools";
import {
  useAppSettings,
  useAppStatus,
  useExportConfig,
  usePlugins,
  usePluginSecret,
  usePluginSettings,
  useReorderPlugins,
  useSaveAppSettings,
  useSavePluginSettings,
  useSetPluginEnabled,
  useSetPluginSecret,
  useTools,
} from "@/lib/query/queries";
import { pickSavePath } from "@/lib/platform";
import { useNavigation, type SettingsSection } from "@/stores/navigation";
import { AboutSection, AppVersionLine } from "./AboutSection";
import { AiSettings } from "./AiSettings";
import { ReposSettings } from "./ReposSettings";
import { SectionTitle } from "./SectionTitle";
import { ToolsSettings } from "./ToolsSettings";

/** URL du catalogue public, quand aucune autre n'est réglée. */
const DEFAULT_CATALOG =
  "https://github.com/RemiSDV/cadence-plugins/releases/download/catalog/catalog.json";

export default function SettingsView({ view }: ViewProps<"settings">) {
  const section = view.section ?? "general";
  const plugins = usePlugins().data ?? [];
  const plugin = section === "plugin" ? plugins.find((p) => p.id === view.plugin) : undefined;

  return (
    <>
      <SettingsBar section={section} plugin={view.plugin} plugins={plugins} />
      <Page
        title="Réglages"
        subtitle="Application, IA de code, outils, services et plugins"
        padding="none"
      >
        <div className="min-w-0 grow overflow-y-auto px-[26px] py-5">
          {section === "ai" && <AiSettings />}
          {section === "tools" && <ToolsSettings />}
          {section === "repos" && <ReposSettings plugins={plugins} />}
          {section !== "ai" && section !== "tools" && section !== "repos" && (
            <div className="max-w-[780px]">
              {section === "general" && <GeneralSection />}
              {section === "plugins" && <PluginsSection />}
              {section === "plugin" &&
                (plugin ? (
                  <PluginSection key={plugin.id} plugin={plugin} />
                ) : (
                  <Callout tone="warning">Le plugin « {view.plugin} » n'est pas installé.</Callout>
                ))}
              {section === "about" && <AboutSection />}
            </div>
          )}
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
  const tools = useTools().data;
  const toFix = (tools?.tools ?? []).filter(
    (t) =>
      !t.ignored &&
      (blocksContinue(t) ||
        (isRequested(t) && (t.status === "not-connected" || t.status === "outdated"))),
  ).length;

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
      </ContextBarSection>
      <ContextBarSection title="Outils">
        <ContextBarItem
          icon="wrench"
          label="Outils externes"
          active={section === "tools"}
          onClick={() => go("tools")}
        />
        {toFix > 0 && (
          <ContextBarSubItems>
            <ContextBarSubItem
              leading={<StatusBullet />}
              label={`${toFix} action${toFix > 1 ? "s" : ""} proposée${toFix > 1 ? "s" : ""}`}
              onClick={() => go("tools")}
            />
          </ContextBarSubItems>
        )}
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

/* ---------- Général ---------- */

/** « Exporter la configuration » : réglages et plugins dans un fichier JSON, sans secret. */
function ExportRow() {
  const exportConfig = useExportConfig();
  const [saved, setSaved] = useState<string | null>(null);
  const run = async () => {
    setSaved(null);
    const date = new Date().toISOString().slice(0, 10);
    const path = await pickSavePath("Exporter la configuration", `cadence-config-${date}.json`);
    if (path) exportConfig.mutate(path, { onSuccess: () => setSaved(path) });
  };
  return (
    <SettingRow
      title="Exporter la configuration"
      description={
        exportConfig.error?.message ??
        (saved
          ? `Enregistrée dans ${saved}.`
          : "Réglages, plugins installés et leurs réglages, en JSON. Les secrets et jetons restent au trousseau.")
      }
      control={
        <Button
          variant="ghost"
          size="sm"
          icon="download"
          disabled={exportConfig.isPending}
          onClick={() => void run()}
        >
          Exporter…
        </Button>
      }
    />
  );
}

function GeneralSection() {
  const settings = useAppSettings().data;
  const dirs = useAppStatus().data?.dirs;
  const save = useSaveAppSettings();
  const [catalog, setCatalog] = useState<string | null>(null);
  const current = settings?.catalogUrl ?? "";
  const value = catalog ?? current;

  return (
    <>
      <SectionTitle title="Général">
        Réglages de l'application. Les plugins activés le sont pour toute l'app ; leurs réglages
        sont dans la section Plugins.
      </SectionTitle>
      <div className="mt-[18px]">
        <SettingRow title="Langue de l'interface" control={<Badge tone="muted">Français</Badge>} />
        <SettingRow
          title="Thème"
          description="Cadence est dessinée pour un thème sombre unique, aux couleurs des maquettes."
          control={<Badge tone="muted">Sombre</Badge>}
        />
        <SettingRow
          title="Mises à jour de Cadence"
          description="Version installée, recherche et installation des mises à jour signées."
          control={
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                useNavigation.getState().navigate({ id: "settings", section: "about" })
              }
            >
              À propos
            </Button>
          }
        />
        <ExportRow />
      </div>

      <SectionLabel className="mt-6">Plugins</SectionLabel>
      <div className="mt-1">
        <SettingRow
          title="Catalogue"
          description="Index des plugins publiés, régénéré par la CI du dépôt public. Vide : catalogue public."
          control={
            <form
              className="flex items-center gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                save.mutate({ catalogUrl: value.trim() }, { onSuccess: () => setCatalog(null) });
              }}
            >
              <Input
                size="sm"
                mono
                aria-label="URL du catalogue"
                placeholder={DEFAULT_CATALOG}
                value={value}
                onChange={(e) => setCatalog(e.target.value)}
                className="w-[340px] text-caption"
              />
              {catalog !== null && catalog !== current && (
                <Button type="submit" size="sm" disabled={save.isPending}>
                  Enregistrer
                </Button>
              )}
            </form>
          }
        />
        <SettingRow
          title="Mettre à jour les plugins automatiquement"
          description="Les nouvelles versions compatibles du catalogue s'installent d'elles-mêmes (vérifié toutes les 6 h). Les plugins de développement ne sont jamais touchés."
          control={
            <Switch
              aria-label="Mettre à jour les plugins automatiquement"
              checked={settings?.pluginAutoUpdate ?? false}
              disabled={!settings || save.isPending}
              onCheckedChange={(checked) => save.mutate({ pluginAutoUpdate: checked })}
            />
          }
        />
        <DirRow
          title="Dossier des plugins"
          description="Archives vérifiées (SHA-256) et extraites par version."
          value={dirs?.plugins}
        />
        <DirRow
          title="Données des plugins"
          description="Un dossier par plugin, transmis au serveur dans CADENCE_DATA_DIR."
          value={dirs?.pluginsData}
        />
      </div>
      {save.isError && (
        <Callout tone="danger" className="mt-3">
          {save.error.message}
        </Callout>
      )}

      <SectionLabel className="mt-6">Sessions de l'IA</SectionLabel>
      <div className="mt-1">
        <DirRow
          title="Dossier des sessions"
          description="Dossier neutre où chaque session d'IA est lancée. Jamais dans un de vos dépôts."
          value={dirs?.sessions}
        />
      </div>
    </>
  );
}

function DirRow({
  title,
  description,
  value,
}: {
  title: string;
  description: string;
  value: string | undefined;
}) {
  return (
    <SettingRow
      title={title}
      description={description}
      control={
        <Input
          size="sm"
          mono
          readOnly
          aria-label={title}
          value={value ?? ""}
          className="w-[340px] text-caption text-tx2"
        />
      }
    />
  );
}

function StatusBullet() {
  return <span className="ml-1 inline-block size-1.5 rounded-full bg-amber" />;
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
            {
              { dev: "mode développement", file: "archive locale", catalog: "catalogue" }[
                plugin.source
              ]
            }
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
