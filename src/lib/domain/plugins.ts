/**
 * Plugins installés : catégories du rail, couleur des tuiles, regroupement des vues dans la
 * barre du plugin (révision 1.1 du plan Cadence 2).
 */
import type { IconName } from "@/components/icons/Icon";
import { iconNames } from "@/components/icons/Icon";
import type {
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
      plugins: plugins.filter((p) => p.category === category && p.enabled && p.status !== "invalid"),
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
  return (
    plugin.views.find((v) => v.home)?.id ?? plugin.views.find((v) => !v.hidden)?.id ?? null
  );
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
