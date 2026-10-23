/**
 * Catalogue de plugins (maquette V2-Catalog.dc.html) : vues et catégories dans la barre,
 * cartes au centre, fiche du plugin choisi à droite. Le catalogue en ligne vient du dépôt
 * RemiSDV/cadence-plugins (`catalog_get`, cache hors ligne) ; les plugins installés hors
 * catalogue (dossier de dev, archive locale) y figurent aussi.
 */
import { useEffect, useMemo, useState } from "react";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarHint,
  ContextBarItem,
  ContextBarSection,
} from "@/components/layout/ContextBar";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { MarkdownInline } from "@/components/domain/Markdown";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
} from "@/components/ui/Dialog";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { formatAgo } from "@/lib/domain/format";
import {
  catalogCards,
  catalogTools,
  categoryOrder,
  installedVersionNote,
  kitIcon,
  pendingSetup,
  pendingSetupLabel,
  pluginCategories,
  type CatalogCard,
  type PendingSetup,
} from "@/lib/domain/plugins";
import { subscribe } from "@/lib/ipc/events";
import { openExternal, pickArchive, pickPluginFolder } from "@/lib/platform";
import {
  useAppSettings,
  useAuthStatus,
  useCatalog,
  useInstallFromCatalog,
  useInstallPlugin,
  usePlugins,
  usePluginStatus,
  useRefreshCatalog,
  useSetPluginEnabled,
  useUninstallPlugin,
} from "@/lib/query/queries";
import type { InstalledPluginView, PluginCategory, PluginInstallEvent } from "@/lib/types";
import { useNavigation } from "@/stores/navigation";

type Tab = "catalog" | "installed" | "updates";
type Origin = "all" | "official" | "community";

const megabytes = (bytes: number | null | undefined) =>
  bytes ? `${(bytes / 1_000_000).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Mo` : null;

export default function CatalogView({ view }: ViewProps<"catalog">) {
  const catalog = useCatalog();
  const plugins = usePlugins();
  const refresh = useRefreshCatalog();
  const installArchive = useInstallPlugin();
  const [tab, setTab] = useState<Tab>("catalog");
  const [category, setCategory] = useState<PluginCategory | null>(null);
  const [origin, setOrigin] = useState<Origin>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<"category" | "name">("category");
  const [selected, setSelected] = useState<string | null>(view.plugin ?? null);
  /** Plugin installé à l'instant : sa fiche propose sa configuration. */
  const [fresh, setFresh] = useState<InstalledPluginView | null>(null);

  const cards = useMemo(
    () => catalogCards(catalog.data?.plugins ?? [], plugins.data ?? []),
    [catalog.data, plugins.data],
  );
  const inTab = cards.filter((c) =>
    tab === "installed" ? c.installed : tab === "updates" ? c.entry?.updateAvailable : c.entry,
  );
  const q = query.trim().toLowerCase();
  const shown = inTab
    .filter((c) => (category ? c.category === category : true))
    .filter((c) => (origin === "all" ? true : origin === "official" ? c.official : !c.official))
    .filter(
      (c) =>
        !q ||
        [c.name, c.id, c.description, ...c.authors, ...(c.entry?.tools ?? [])].some((t) =>
          t.toLowerCase().includes(q),
        ),
    )
    .sort((a, b) =>
      sort === "name"
        ? a.name.localeCompare(b.name, "fr")
        : categoryOrder.indexOf(a.category) - categoryOrder.indexOf(b.category) ||
          a.name.localeCompare(b.name, "fr"),
    );
  const current = cards.find((c) => c.id === selected) ?? shown[0] ?? null;
  const installedCount = cards.filter((c) => c.installed).length;
  const updates = cards.filter((c) => c.entry?.updateAvailable).length;

  const subtitle = catalog.data
    ? [
        "RemiSDV/cadence-plugins",
        `${catalog.data.plugins.length} plugin${catalog.data.plugins.length > 1 ? "s" : ""}`,
        catalog.data.fetchedAt ? `actualisé ${formatAgo(catalog.data.fetchedAt)}` : null,
        catalog.data.offline ? "hors ligne" : null,
      ]
        .filter(Boolean)
        .join(" · ")
    : "RemiSDV/cadence-plugins";

  const onInstalled = (plugin: InstalledPluginView) => {
    setSelected(plugin.id);
    setFresh(plugin);
  };
  const onArchive = async () => {
    const path = await pickArchive();
    if (path) installArchive.mutate({ kind: "zip", path }, { onSuccess: onInstalled });
  };
  const onDevFolder = async () => {
    const path = await pickPluginFolder();
    if (path) installArchive.mutate({ kind: "dev", path }, { onSuccess: onInstalled });
  };

  return (
    <>
      <ContextBar
        label="Catalogue"
        header={<ContextBarHeader tile={<NeutralTile icon="grid" />} title="Plugins" />}
        footer={
          <ContextBarHint>
            La catégorie est déclarée dans le manifeste, parmi une liste fixe. Elle range le plugin
            dans le rail.
          </ContextBarHint>
        }
      >
        <ContextBarSection title="Vues">
          <ContextBarItem
            icon="grid"
            label="Catalogue"
            count={catalog.data?.plugins.length ?? 0}
            active={tab === "catalog"}
            onClick={() => setTab("catalog")}
          />
          <ContextBarItem
            icon="check"
            label="Installés"
            count={installedCount}
            active={tab === "installed"}
            onClick={() => setTab("installed")}
          />
          <ContextBarItem
            icon="download"
            label="Mises à jour"
            count={updates}
            countTone={updates > 0 ? "clay" : "tx3"}
            active={tab === "updates"}
            onClick={() => setTab("updates")}
          />
        </ContextBarSection>
        <ContextBarSection title="Catégories">
          <CategoryFilter
            label="Toutes"
            count={inTab.length}
            active={category === null}
            onClick={() => setCategory(null)}
          />
          {categoryOrder.map((c) => (
            <CategoryFilter
              key={c}
              label={pluginCategories[c].label}
              count={inTab.filter((p) => p.category === c).length}
              active={category === c}
              onClick={() => setCategory(c)}
            />
          ))}
        </ContextBarSection>
      </ContextBar>

      <Page
        title="Catalogue"
        subtitle={subtitle}
        padding="none"
        actions={
          <>
            <Button
              variant="ghost"
              icon="folder"
              disabled={installArchive.isPending}
              title="Plugin non publié : le dossier qui porte son plugin.toml, lu sur place"
              onClick={() => void onDevFolder()}
            >
              Dossier de développement…
            </Button>
            <Button
              variant="ghost"
              icon="download"
              disabled={installArchive.isPending}
              onClick={() => void onArchive()}
            >
              Installer une archive…
            </Button>
            <Button icon="refresh" disabled={refresh.isPending} onClick={() => refresh.mutate()}>
              Actualiser
            </Button>
          </>
        }
      >
        <div className="flex min-w-0 grow flex-col gap-3.5 overflow-y-auto px-[22px] pt-4 pb-[18px]">
          <div className="flex items-center gap-2">
            <label className="flex h-8 max-w-[280px] grow items-center gap-2 rounded-control border border-line bg-p2 px-2.5 text-tx3">
              <Icon name="search" size={14} />
              <input
                aria-label="Rechercher un plugin"
                placeholder="Nom, auteur, outil…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
              />
            </label>
            {(["all", "official", "community"] as const).map((o) => (
              <button
                key={o}
                type="button"
                aria-pressed={origin === o}
                onClick={() => setOrigin(o)}
                className={cn(
                  "inline-flex h-[30px] items-center rounded-control border px-[11px] text-body-sm",
                  origin === o
                    ? "border-clay-line bg-clay-d text-clay"
                    : "border-line bg-p2 text-tx2",
                )}
              >
                {{ all: "Tous", official: "Officiels", community: "Communauté" }[o]}
              </button>
            ))}
            <div className="grow" />
            <select
              aria-label="Trier"
              value={sort}
              onChange={(e) => setSort(e.target.value as "category" | "name")}
              className="h-[30px] w-[160px] rounded-control border border-line2 bg-p2 px-2.5 text-body-sm text-tx"
            >
              <option value="category">Tri : catégorie</option>
              <option value="name">Tri : nom</option>
            </select>
          </div>

          {catalog.data?.offline && (
            <Callout tone="warning" icon="warning">
              Catalogue injoignable : voici le dernier reçu
              {catalog.data.fetchedAt ? ` (${formatAgo(catalog.data.fetchedAt)})` : ""}.{" "}
              {catalog.data.error}
            </Callout>
          )}
          {installArchive.error && (
            <Callout tone="danger">
              {installArchive.variables?.kind === "dev" ? "Dossier refusé" : "Archive refusée"} :{" "}
              {installArchive.error.message}
            </Callout>
          )}
          {fresh && (fresh.installWarnings?.length ?? 0) > 0 && <InstallWarnings plugin={fresh} />}

          {catalog.isPending || plugins.isPending ? (
            <LoadingState />
          ) : catalog.isError && cards.length === 0 ? (
            <ErrorState error={catalog.error} />
          ) : shown.length === 0 ? (
            <Card className="py-6">
              <EmptyState
                icon="grid"
                title={tab === "updates" ? "Tout est à jour" : "Aucun plugin"}
              >
                {tab === "updates"
                  ? "Les plugins installés depuis le catalogue sont à leur dernière version."
                  : q || category || origin !== "all"
                    ? "Aucun plugin ne correspond à ces filtres."
                    : "Le catalogue est vide pour l'instant."}
              </EmptyState>
            </Card>
          ) : (
            <div className="grid grid-cols-2 gap-3">
              {shown.map((card) => (
                <PluginCardView
                  key={card.id}
                  card={card}
                  selected={card.id === current?.id}
                  onSelect={() => setSelected(card.id)}
                />
              ))}
            </div>
          )}

          <Callout tone="info" icon="key" className="mt-auto">
            Les plugins s'exécutent en confiance totale, avec vos droits. Ils n'entrent au catalogue
            qu'après la revue de leur pull request sur{" "}
            <span className="mono text-tx">RemiSDV/cadence-plugins</span>. Pour proposer le vôtre,
            lisez{" "}
            <button
              type="button"
              className="text-clay hover:text-clay-hover"
              onClick={() =>
                void openExternal(
                  "https://github.com/RemiSDV/cadence-plugins/blob/main/CONTRIBUTING.md",
                )
              }
            >
              CONTRIBUTING.md
            </button>
            .
          </Callout>
        </div>
        {current && (
          <PluginDetail
            key={current.id}
            card={current}
            fresh={fresh?.id === current.id}
            onInstalled={onInstalled}
          />
        )}
      </Page>
    </>
  );
}

function CategoryFilter({
  label,
  count,
  active,
  onClick,
}: {
  label: string;
  count: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex h-[30px] w-full items-center gap-[9px] rounded-control px-[9px] text-left text-body",
        active ? "bg-p3 font-medium text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      <span
        className={cn(
          "inline-block size-1.5 shrink-0 rounded-full",
          active ? "bg-clay" : "bg-line2",
        )}
      />
      <span className="grow">{label}</span>
      <span className={cn("mono text-tiny", active ? "text-clay" : "text-tx3")}>{count}</span>
    </button>
  );
}

function CategoryPill({ category }: { category: PluginCategory }) {
  return (
    <span className="inline-flex h-[19px] items-center rounded-badge border border-line2 px-[7px] text-tiny font-medium whitespace-nowrap text-tx2">
      {pluginCategories[category].label}
    </span>
  );
}

function StateBadges({ card }: { card: CatalogCard }) {
  const { entry, installed } = card;
  if (entry && !entry.compatible && !installed) return <Badge tone="red">Incompatible</Badge>;
  return (
    <>
      {!installed ? (
        <Badge tone="neutral">Non installé</Badge>
      ) : installed.status === "invalid" ? (
        <Badge tone="red">Invalide</Badge>
      ) : installed.enabled ? (
        <Badge tone="green" icon="check">
          Activé
        </Badge>
      ) : (
        <Badge tone="muted">Désactivé</Badge>
      )}
      {entry?.updateAvailable && <Badge tone="amber">{entry.version} disponible</Badge>}
      {installed?.source === "dev" && <Badge tone="purple">Développement</Badge>}
      {installed?.status === "needs-setup" && <Badge tone="amber">À configurer</Badge>}
    </>
  );
}

function PluginCardView({
  card,
  selected,
  onSelect,
}: {
  card: CatalogCard;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        "flex h-full w-full flex-col gap-2.5 rounded-card border px-4 py-[15px] text-left",
        selected ? "border-clay-line bg-p2" : "border-line bg-panel hover:border-line2",
      )}
    >
      <span className="flex w-full items-center gap-[11px]">
        <PluginTile color={card.color} icon={card.railIcon} size={34} />
        <span className="min-w-0 grow">
          <span className="flex items-center gap-[7px]">
            <span className="truncate text-body-lg font-semibold">{card.name}</span>
            {card.entry &&
              (card.official ? (
                <Badge tone="clay">Officiel</Badge>
              ) : (
                <Badge tone="muted">Communauté</Badge>
              ))}
          </span>
          <span className="mt-1 flex items-center gap-[7px] text-caption text-tx3">
            <CategoryPill category={card.category} />
            {[card.authors.join(", "), `v${card.installed?.version ?? card.version}`]
              .filter(Boolean)
              .join(" · ")}
          </span>
        </span>
      </span>
      <span className="line-clamp-2 h-[39px] text-body-sm leading-[1.55] text-tx2">
        {card.description}
      </span>
      <span className="flex w-full items-center gap-1.5">
        <StateBadges card={card} />
        {card.services.includes("repos") && (
          <span className="ml-auto inline-flex items-center gap-[5px] text-caption text-tx3">
            <Icon name="github" size={13} />
            Dépôts GitHub
          </span>
        )}
      </span>
    </button>
  );
}

/** Progression de l'installation d'un plugin (`plugin://install`). */
function useInstallProgress(id: string) {
  const [event, setEvent] = useState<PluginInstallEvent | null>(null);
  useEffect(
    () =>
      subscribe("plugin://install", (e) => {
        if (e.id === id) setEvent(e.phase === "done" ? null : e);
      }),
    [id],
  );
  return event;
}

function PluginDetail({
  card,
  fresh,
  onInstalled,
}: {
  card: CatalogCard;
  /** Installé à l'instant : la fiche propose sa configuration. */
  fresh: boolean;
  onInstalled: (plugin: InstalledPluginView) => void;
}) {
  const navigate = useNavigation((s) => s.navigate);
  const install = useInstallFromCatalog();
  const reload = useInstallPlugin();
  const setEnabled = useSetPluginEnabled();
  const [confirm, setConfirm] = useState(false);
  const progress = useInstallProgress(card.id);
  const process = (usePluginStatus().data ?? []).find((p) => p.plugin === card.id) ?? null;
  const { entry, installed } = card;
  const size = megabytes(entry?.size);
  const busy = install.isPending || reload.isPending || progress !== null;
  const percent = progress && progress.total > 0 ? (progress.received / progress.total) * 100 : 100;
  const versionNote = entry && installed ? installedVersionNote(entry.version, installed) : null;
  const tools = catalogTools(entry, installed, process);

  return (
    <aside className="flex w-[400px] shrink-0 flex-col gap-[13px] overflow-y-auto border-l border-line bg-panel px-5 py-[18px]">
      <div className="flex items-start gap-[13px]">
        <PluginTile color={card.color} icon={card.railIcon} size={46} />
        <div className="min-w-0 grow">
          <h2 className="font-display text-title-lg font-semibold">{card.name}</h2>
          <div className="mt-[3px] text-small text-tx3">
            {card.authors.length > 0 && `par ${card.authors.join(", ")} · `}
            <span className="mono">{card.id}</span>
          </div>
          <div className="mt-[7px] flex items-center gap-1.5">
            {entry &&
              (card.official ? (
                <Badge tone="clay">Officiel</Badge>
              ) : (
                <Badge tone="muted">Communauté</Badge>
              ))}
            <Badge tone="neutral" mono>
              v{installed?.version ?? entry?.version ?? card.version}
            </Badge>
            {versionNote && (
              <Badge tone="muted" mono>
                catalogue v{entry!.version}
              </Badge>
            )}
            <CategoryPill category={card.category} />
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-[9px]">
        {progress ? (
          <div className="flex flex-col gap-1.5">
            <ProgressBar value={percent} height={5} />
            <span className="text-caption text-tx3">
              {progress.phase === "downloading"
                ? `Téléchargement · ${megabytes(progress.received) ?? "0 Mo"}${progress.total ? ` sur ${megabytes(progress.total)}` : ""}`
                : progress.phase === "verifying"
                  ? "Vérification de l'empreinte sha256…"
                  : "Extraction et vérification du plugin…"}
            </span>
          </div>
        ) : entry && !installed ? (
          <Button
            variant="primary"
            size="tall"
            icon="download"
            disabled={!entry.compatible || busy}
            onClick={() =>
              install.mutate({ id: card.id, update: false }, { onSuccess: onInstalled })
            }
          >
            {entry.compatible ? `Installer et activer${size ? ` · ${size}` : ""}` : "Incompatible"}
          </Button>
        ) : entry?.updateAvailable ? (
          <Button
            variant="primary"
            size="tall"
            icon="download"
            disabled={!entry.compatible || busy}
            onClick={() =>
              install.mutate({ id: card.id, update: true }, { onSuccess: onInstalled })
            }
          >
            Mettre à jour vers {entry.version}
            {size ? ` · ${size}` : ""}
          </Button>
        ) : null}
        {installed && !progress && (
          <div className="flex flex-wrap gap-2">
            {installed.enabled && installed.views.some((v) => !v.hidden) && (
              <Button
                size="sm"
                icon="arrowRight"
                onClick={() => {
                  const view =
                    installed.views.find((v) => v.home) ?? installed.views.find((v) => !v.hidden)!;
                  navigate({ id: "plugin", plugin: installed.id, view: view.id });
                }}
              >
                Ouvrir
              </Button>
            )}
            <Button
              size="sm"
              variant="ghost"
              disabled={setEnabled.isPending || installed.status === "invalid"}
              onClick={() => setEnabled.mutate({ id: installed.id, enabled: !installed.enabled })}
            >
              {installed.enabled ? "Désactiver" : "Activer"}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              icon="settings"
              onClick={() => navigate({ id: "settings", section: "plugin", plugin: installed.id })}
            >
              Réglages
            </Button>
            {installed.source === "dev" && installed.devPath && (
              <Button
                size="sm"
                variant="ghost"
                icon="refresh"
                disabled={busy}
                title={`Relit ${installed.devPath} et redémarre le serveur du plugin`}
                onClick={() => reload.mutate({ kind: "dev", path: installed.devPath! })}
              >
                Recharger
              </Button>
            )}
            <Button size="sm" variant="danger" onClick={() => setConfirm(true)}>
              Désinstaller
            </Button>
          </div>
        )}
        <div className="text-caption leading-[1.5] text-tx3">
          {entry?.incompatibility ??
            [
              `Activé pour toute l'app, dans le groupe ${pluginCategories[card.category].label} du rail`,
              versionNote,
              entry ? (
                <span key="a" className="mono">
                  {versionNote ? `catalogue : ${entry.archive}` : entry.archive}
                </span>
              ) : null,
              entry?.minCadence ? `Cadence ${entry.minCadence} minimum` : null,
              installed?.source === "dev" && !versionNote ? "dossier de développement" : null,
              installed?.source === "file" && !versionNote ? "installé depuis une archive" : null,
            ]
              .filter(Boolean)
              .flatMap((part, i) => (i === 0 ? [part] : [" · ", part]))}
          .
        </div>
        {(install.error ?? reload.error) && (
          <p className="text-caption text-red">{(install.error ?? reload.error)!.message}</p>
        )}
        {reload.isSuccess && <p className="text-caption text-green">Plugin rechargé.</p>}
      </div>

      {installed && <SetupSection plugin={installed} fresh={fresh} />}

      {card.description && (
        <p className="text-body-sm leading-[1.6] text-tx2">{card.description}</p>
      )}
      {installed?.problem && <p className="text-caption text-amber">{installed.problem}</p>}

      {card.services.includes("repos") && (
        <DetailSection title="Services utilisés">
          <div className="mt-2 rounded-[10px] border border-line bg-p2 px-3 py-[11px]">
            <div className="flex items-center gap-[9px]">
              <span className="inline-flex size-5 items-center justify-center rounded-badge bg-clay text-clay-ink">
                <Icon name="github" size={12} />
              </span>
              <span className="grow text-body-sm font-medium">Dépôts GitHub</span>
            </div>
            <div className="mt-[7px] text-caption leading-[1.55] text-tx3">
              Fourni par Cadence. Ce plugin aura sa propre liste de dépôts suivis.
            </div>
          </div>
        </DetailSection>
      )}

      {(entry?.views.length ?? installed?.views.length ?? 0) > 0 && (
        <DetailSection title="Vues">
          <div className="mt-2 flex flex-wrap gap-1.5">
            {(entry?.views ?? installed?.views.filter((v) => !v.hidden) ?? []).map((v) => (
              <Badge key={v.id} tone="muted">
                <Icon name={kitIcon(v.icon)} size={12} />
                {v.title}
              </Badge>
            ))}
          </div>
        </DetailSection>
      )}

      {entry && entry.actions.length > 0 && (
        <DetailSection title="Actions · skills">
          <div className="mt-0.5">
            {entry.actions.map((a, i) => (
              <div
                key={a.id}
                className={cn(
                  "flex items-center gap-[9px] py-[7px]",
                  i < entry.actions.length - 1 && "border-b border-line",
                )}
              >
                <span className="flex text-clay">
                  <Icon name="bolt" size={14} />
                </span>
                <span className="grow text-body-sm">{a.label}</span>
                {a.mode === "background" ? (
                  <Badge tone="purple" icon="clock">
                    en fond
                  </Badge>
                ) : (
                  <Badge tone="blue" icon="chat">
                    chat
                  </Badge>
                )}
              </div>
            ))}
          </div>
        </DetailSection>
      )}

      {(tools.names.length > 0 || tools.uiTools > 0) && (
        <DetailSection title={`Outils MCP · ${tools.names.length}`}>
          {tools.note && <div className="mt-1 text-caption text-tx3">{tools.note}</div>}
          <div className="mt-[7px] mono text-caption leading-[1.8] break-all text-tx2">
            {tools.names.map((t) => `${card.id}__${t}`).join(" · ")}
          </div>
          {tools.uiTools > 0 && (
            <div className="mt-1 text-caption text-tx3">
              {tools.uiTools > 1
                ? `+ ${tools.uiTools} outils réservés à l'interface du plugin, jamais montrés aux IA.`
                : "+ 1 outil réservé à l'interface du plugin, jamais montré aux IA."}
            </div>
          )}
        </DetailSection>
      )}

      {entry?.homepage && (
        <div className="mt-auto flex flex-col gap-[7px] border-t border-line pt-3 text-small">
          <button
            type="button"
            onClick={() => void openExternal(entry.homepage!)}
            className="inline-flex items-center gap-[7px] text-left text-clay hover:text-clay-hover"
          >
            <Icon name="github" size={14} />
            {entry.homepage.replace("https://github.com/", "").replace("/tree/main/", " · ")}
            <Icon name="link" size={12} />
          </button>
        </div>
      )}

      {installed && (
        <UninstallDialog
          plugin={{
            id: installed.id,
            name: installed.name,
            hasSecrets: installed.secrets.length > 0,
          }}
          open={confirm}
          onClose={() => setConfirm(false)}
        />
      )}
    </aside>
  );
}

/** Avertissements de l'installation qui vient d'avoir lieu (cibles sans binaire…). */
function InstallWarnings({ plugin }: { plugin: InstalledPluginView }) {
  return (
    <Callout tone="warning" icon="warning">
      {plugin.name} {plugin.version} est installé, avec des avertissements :
      <ul className="mt-1 list-disc pl-4">
        {(plugin.installWarnings ?? []).map((w) => (
          <li key={w}>
            <MarkdownInline text={w} />
          </li>
        ))}
      </ul>
    </Callout>
  );
}

/**
 * Configuration restante d'un plugin installé (N03) : service Dépôts, secrets, étapes
 * `[[setup]]`. Montrée juste après une installation, puis tant que le service ou un secret
 * manque.
 */
function SetupSection({ plugin, fresh }: { plugin: InstalledPluginView; fresh: boolean }) {
  const navigate = useNavigation((s) => s.navigate);
  const auth = useAuthStatus().data;
  const settings = useAppSettings().data;
  const pending = pendingSetup(plugin, {
    githubConnected: auth?.github?.status === "connected",
    trackedRepos: settings?.trackedRepos[plugin.id] ?? [],
  });
  const blocking = pending.some((p) => p.kind !== "view");
  if (pending.length === 0 || (!fresh && !blocking)) return null;
  const open = (item: PendingSetup) => {
    if (item.kind === "view")
      navigate({
        id: "plugin",
        plugin: plugin.id,
        view: item.step.view,
        params: { setup: item.step.id },
      });
    else if (item.kind === "secrets")
      navigate({ id: "settings", section: "plugin", plugin: plugin.id });
    else navigate({ id: "settings", section: "repos" });
  };
  return (
    <DetailSection title="Configuration">
      <div className="mt-2 flex flex-col gap-1.5 rounded-[10px] border border-amber-line bg-amber-d px-3 py-[11px]">
        <span className="text-caption leading-[1.55] text-tx2">
          {fresh ? `${plugin.name} est installé. ` : ""}
          {pending.length > 1 ? "Étapes restantes :" : "Étape restante :"}
        </span>
        {pending.map((item) => (
          <button
            key={item.kind === "view" ? item.step.id : item.kind}
            type="button"
            onClick={() => open(item)}
            className="inline-flex items-center gap-[7px] text-left text-body-sm text-clay hover:text-clay-hover"
          >
            <Icon name="arrowRight" size={12} />
            {pendingSetupLabel(item)}
            {item.kind === "view" && !item.step.required && (
              <span className="text-caption text-tx3">(facultatif)</span>
            )}
          </button>
        ))}
      </div>
    </DetailSection>
  );
}

function DetailSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">{title}</div>
      {children}
    </div>
  );
}

function UninstallDialog({
  plugin,
  open,
  onClose,
}: {
  plugin: { id: string; name: string; hasSecrets: boolean };
  open: boolean;
  onClose: () => void;
}) {
  const uninstall = useUninstallPlugin();
  const [secrets, setSecrets] = useState(false);
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} width={460}>
      <DialogHeader title={`Désinstaller ${plugin.name} ?`} icon="warning" />
      <DialogBody>
        <p className="text-body-sm leading-[1.6] text-tx2">
          Son serveur s'arrête et ses fichiers sont supprimés. Ses réglages et ses données restent
          sur cette machine : une réinstallation les retrouve.
        </p>
        {plugin.hasSecrets && (
          <label className="flex items-center gap-2 text-body-sm text-tx2">
            <input
              type="checkbox"
              checked={secrets}
              onChange={(e) => setSecrets(e.target.checked)}
              className="size-[13px] accent-clay"
            />
            Supprimer aussi ses secrets du trousseau
          </label>
        )}
        {uninstall.error && <p className="text-caption text-red">{uninstall.error.message}</p>}
      </DialogBody>
      <DialogFooter className="justify-end">
        <DialogClose asChild>
          <Button variant="ghost">Annuler</Button>
        </DialogClose>
        <Button
          variant="danger"
          disabled={uninstall.isPending}
          onClick={() =>
            uninstall.mutate({ id: plugin.id, deleteSecrets: secrets }, { onSuccess: onClose })
          }
        >
          {uninstall.isPending ? <Spinner size={13} /> : null}
          Désinstaller
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
