/**
 * Plugins installés : catégories du rail, couleur des tuiles, regroupement des vues dans la
 * barre du plugin (révision 1.1 du plan Cadence 2).
 */
import type { IconName } from "@/components/icons/Icon";
import { iconNames } from "@/components/icons/Icon";
import { compareVersions } from "@/lib/domain/versions";
import type {
  CatalogEntryView,
  InstalledPluginView,
  PluginCategory,
  PluginColor,
  PluginProcessView,
  PluginViewInfo,
} from "@/lib/types";

/** Liste fixe du manifeste (`category`), dans l'ordre du rail. */
export const pluginCategories: Record<PluginCategory, { label: string }> = {
  projet: { label: "Projet" },
  etudes: { label: "Études" },
  code: { label: "Code" },
  donnees: { label: "Données" },
  outils: { label: "Outils" },
};

export const categoryOrder: PluginCategory[] = ["projet", "etudes", "code", "donnees", "outils"];

/** Fond plein de la tuile du plugin (rail, en-tête de sa barre, puces de contexte). */
export const pluginTileClass: Record<PluginColor, string> = {
  clay: "bg-clay",
  blue: "bg-blue",
  green: "bg-green",
  purple: "bg-purple",
  amber: "bg-amber",
  red: "bg-red",
};

/** Icône du kit d'un nom venu d'un manifeste ; repli sur `layers` si le kit ne la connaît pas. */
export function kitIcon(name: string): IconName {
  return (iconNames as string[]).includes(name) ? (name as IconName) : "layers";
}

export interface RailGroup {
  category: PluginCategory;
  label: string;
  plugins: InstalledPluginView[];
}

/**
 * Groupes du rail : plugins activés et utilisables, par catégorie dans l'ordre fixe. L'ordre
 * de la liste reçue (ordre choisi par l'utilisateur) est gardé dans chaque catégorie.
 */
export function railGroups(plugins: InstalledPluginView[]): RailGroup[] {
  return categoryOrder
    .map((category) => ({
      category,
      label: pluginCategories[category].label,
      plugins: plugins.filter(
        (p) => p.category === category && p.enabled && p.status !== "invalid",
      ),
    }))
    .filter((g) => g.plugins.length > 0);
}

export interface ViewSection {
  /** `null` : vues sans section, en tête de la barre. */
  title: string | null;
  views: PluginViewInfo[];
}

/** Vues visibles de la barre du plugin, groupées par `section` dans l'ordre du manifeste. */
export function viewSections(views: PluginViewInfo[]): ViewSection[] {
  const sections: ViewSection[] = [];
  for (const view of views.filter((v) => !v.hidden)) {
    const title = view.section ?? null;
    const section = sections.find((s) => s.title === title);
    if (section) section.views.push(view);
    else sections.push({ title, views: [view] });
  }
  return sections;
}

/** Vue ouverte par un clic sur la tuile : l'accueil déclaré, sinon la première vue visible. */
export function homeView(plugin: InstalledPluginView): string | null {
  return plugin.views.find((v) => v.home)?.id ?? plugin.views.find((v) => !v.hidden)?.id ?? null;
}

/**
 * Déplace un plugin d'un cran dans sa catégorie (le rail ne mélange pas les catégories) ;
 * renvoie l'ordre complet à envoyer à `plugins_reorder`, ou `null` s'il ne bouge pas.
 */
export function moveInRail(
  plugins: InstalledPluginView[],
  id: string,
  delta: -1 | 1,
): string[] | null {
  const plugin = plugins.find((p) => p.id === id);
  if (!plugin) return null;
  const peers = plugins.filter((p) => p.category === plugin.category);
  const from = peers.indexOf(plugin);
  const to = from + delta;
  if (to < 0 || to >= peers.length) return null;
  const target = peers[to]!;
  const order = plugins.map((p) => p.id);
  const i = order.indexOf(id);
  const j = order.indexOf(target.id);
  [order[i], order[j]] = [order[j]!, order[i]!];
  return order;
}

/* ---------- Réglages d'un plugin : champs tirés de son JSON Schema ---------- */

export type SettingKind = "integer" | "number" | "string" | "multiline" | "boolean" | "enum";

export interface SettingField {
  key: string;
  title: string;
  description: string | null;
  kind: SettingKind;
  options: string[];
  required: boolean;
  min: number | null;
  max: number | null;
}

type SchemaProperty = {
  type?: string;
  title?: string;
  description?: string;
  enum?: unknown[];
  format?: string;
  minimum?: number;
  maximum?: number;
  "x-cadence-order"?: number;
};

/**
 * Champs du formulaire de réglages (sous-ensemble du plan § 3.6) : entiers et nombres,
 * textes (`format: "multiline"`), booléens, listes de choix. Les types hors sous-ensemble
 * (listes, objets) ne sont pas rendus ; l'ordre suit `x-cadence-order`, puis le schéma.
 */
export function schemaFields(schema: Record<string, unknown> | null): SettingField[] {
  const props = (schema?.properties ?? {}) as Record<string, SchemaProperty>;
  const required = new Set((schema?.required ?? []) as string[]);
  return Object.entries(props)
    .map(([key, p], index) => ({ key, p, index }))
    .sort(
      (a, b) =>
        (a.p["x-cadence-order"] ?? Infinity) - (b.p["x-cadence-order"] ?? Infinity) ||
        a.index - b.index,
    )
    .flatMap(({ key, p }): SettingField[] => {
      const kind: SettingKind | null = p.enum
        ? "enum"
        : p.type === "integer" || p.type === "number" || p.type === "boolean"
          ? p.type
          : p.type === "string"
            ? p.format === "multiline"
              ? "multiline"
              : "string"
            : null;
      if (!kind) return [];
      return [
        {
          key,
          title: p.title ?? key,
          description: p.description ?? null,
          kind,
          options: (p.enum ?? []).map(String),
          required: required.has(key),
          min: p.minimum ?? null,
          max: p.maximum ?? null,
        },
      ];
    });
}

/** Valeur saisie → valeur JSON du réglage (`undefined` : champ vidé, réglage retiré). */
export function parseSettingInput(field: SettingField, raw: string | boolean): unknown {
  if (field.kind === "boolean") return Boolean(raw);
  const text = String(raw);
  if (field.kind === "integer" || field.kind === "number") {
    if (text.trim() === "") return undefined;
    const n = Number(text.replace(",", "."));
    return Number.isNaN(n) ? text : n;
  }
  return text;
}

/** Carte du catalogue : entrée du catalogue en ligne, ou plugin installé qui n'y est pas
 * (dossier de dev, archive locale). */
export interface CatalogCard {
  id: string;
  name: string;
  version: string;
  description: string;
  authors: string[];
  category: PluginCategory;
  color: PluginColor;
  railIcon: string;
  official: boolean;
  services: InstalledPluginView["services"];
  /** Entrée du catalogue, si le plugin y est. */
  entry: CatalogEntryView | null;
  /** Plugin installé, s'il l'est. */
  installed: InstalledPluginView | null;
}

/** Catalogue en ligne et plugins installés, fusionnés par id. */
export function catalogCards(
  entries: CatalogEntryView[],
  installed: InstalledPluginView[],
): CatalogCard[] {
  const cards: CatalogCard[] = entries.map((e) => ({
    id: e.id,
    name: e.name,
    version: e.version,
    description: e.description,
    authors: e.authors,
    category: e.category,
    color: e.color,
    railIcon: e.railIcon,
    official: e.official,
    services: e.services,
    entry: e,
    installed: installed.find((p) => p.id === e.id) ?? null,
  }));
  for (const p of installed) {
    if (entries.some((e) => e.id === p.id)) continue;
    cards.push({
      id: p.id,
      name: p.name,
      version: p.version,
      description: p.description,
      authors: [],
      category: p.category,
      color: p.color,
      railIcon: p.railIcon,
      official: false,
      services: p.services,
      entry: null,
      installed: p,
    });
  }
  return cards;
}

const sourceLabel: Record<InstalledPluginView["source"], string> = {
  catalog: "depuis le catalogue",
  file: "depuis une archive locale",
  dev: "en développement",
};

/**
 * Version installée comparée à celle du catalogue (recette du 21/10, N03) : `null` si ce sont
 * les mêmes. Sinon une phrase pour la fiche (« v0.7.0 installée en développement, plus récente
 * que le catalogue (v0.6.0) »).
 */
export function installedVersionNote(
  catalogVersion: string,
  installed: Pick<InstalledPluginView, "version" | "source">,
): string | null {
  const order = compareVersions(installed.version, catalogVersion);
  if (order === 0) return null;
  const head = `v${installed.version} installée ${sourceLabel[installed.source]}`;
  return order > 0
    ? `${head}, plus récente que le catalogue (v${catalogVersion})`
    : `${head} · v${catalogVersion} au catalogue`;
}

/** Ce qui reste à configurer après l'installation d'un plugin. */
export type PendingSetup =
  | { kind: "github" }
  | { kind: "repos" }
  | { kind: "secrets" }
  | { kind: "view"; step: InstalledPluginView["setup"][number] };

/**
 * Configuration à proposer après l'installation (N03) : service Dépôts (compte GitHub, dépôts
 * suivis) s'il est déclaré, secrets requis manquants, puis les étapes `[[setup]]` du manifeste.
 */
export function pendingSetup(
  plugin: Pick<InstalledPluginView, "services" | "status" | "setup" | "enabled">,
  context: { githubConnected: boolean; trackedRepos: string[] },
): PendingSetup[] {
  if (!plugin.enabled || plugin.status === "invalid") return [];
  const pending: PendingSetup[] = [];
  if (plugin.services.includes("repos")) {
    if (!context.githubConnected) pending.push({ kind: "github" });
    if (context.trackedRepos.length === 0) pending.push({ kind: "repos" });
  }
  if (plugin.status === "needs-setup") pending.push({ kind: "secrets" });
  for (const step of plugin.setup) pending.push({ kind: "view", step });
  return pending;
}

/** Libellé d'une configuration restante (bouton de la fiche du plugin). */
export function pendingSetupLabel(item: PendingSetup): string {
  switch (item.kind) {
    case "github":
      return "Connecter le compte GitHub";
    case "repos":
      return "Choisir le dépôt";
    case "secrets":
      return "Renseigner les secrets requis";
    case "view":
      return item.step.title;
  }
}

/** Outils MCP montrés par la fiche d'un plugin, et d'où vient la liste. */
export interface CatalogTools {
  /** Noms sans préfixe, visibles des IA (outils `ui` exclus, comme partout). */
  names: string[];
  /** Outils réservés à l'interface du plugin, annoncés à part. */
  uiTools: number;
  /** Écart entre la liste montrée et l'autre source, s'il y en a un. */
  note: string | null;
}

/**
 * Liste d'outils d'une fiche (recette du 21/10, P02) : celle du serveur installé quand il
 * tourne (le même nombre que la session de chat), sinon celle du catalogue. Les deux comptent
 * les outils visibles des IA ; une version installée différente du catalogue est signalée.
 */
export function catalogTools(
  catalog: Pick<CatalogEntryView, "version" | "tools"> | null,
  installed: Pick<InstalledPluginView, "version"> | null,
  process: Pick<PluginProcessView, "state" | "toolNames" | "uiTools"> | null,
): CatalogTools {
  const live = installed && process?.state === "ready" ? (process.toolNames ?? []) : null;
  const uiTools = (installed && process?.uiTools) || 0;
  if (live && (live.length > 0 || !catalog)) {
    const differs =
      catalog && (catalog.tools.length !== live.length || catalog.version !== installed!.version);
    return {
      names: live,
      uiTools,
      note: differs
        ? `Outils de la v${installed!.version} installée ; le catalogue en annonce ${catalog.tools.length} pour la v${catalog.version}.`
        : null,
    };
  }
  const names = catalog?.tools ?? [];
  return {
    names,
    uiTools,
    note:
      catalog && installed && installed.version !== catalog.version
        ? `Liste du catalogue (v${catalog.version}) : celle de la v${installed.version} installée s'affiche quand son serveur tourne.`
        : null,
  };
}

/**
 * Réglages en cache après `repos://changed` (recette du 21/10, N05) : la liste suivie par un
 * plugin peut changer hors des mutations du front (pont de l'iframe, `cadence.repos.follow`) ;
 * Réglages › Dépôts GitHub et la barre du plugin la relisent ici.
 */
export function applyReposChanged<
  T extends { trackedRepos: Record<string, string[]>; repoSelection: Record<string, string> },
>(settings: T, change: { plugin: string; repos: string[]; selection: string | null }): T {
  const trackedRepos = { ...settings.trackedRepos };
  if (change.repos.length > 0) trackedRepos[change.plugin] = [...change.repos];
  else delete trackedRepos[change.plugin];
  const repoSelection = { ...settings.repoSelection };
  if (change.selection) repoSelection[change.plugin] = change.selection;
  else delete repoSelection[change.plugin];
  return { ...settings, trackedRepos, repoSelection };
}
