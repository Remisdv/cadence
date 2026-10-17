/**
 * Plugins installés : catégories du rail, couleur des tuiles, regroupement des vues dans la
 * barre du plugin (révision 1.1 du plan Cadence 2).
 */
import type { IconName } from "@/components/icons/Icon";
import { iconNames } from "@/components/icons/Icon";
import type {
  CatalogEntryView,
  InstalledPluginView,
  PluginCategory,
  PluginColor,
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
