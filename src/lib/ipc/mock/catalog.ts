/**
 * Catalogue du mode mock (maquette V2-Catalog.dc.html) : les plugins installés factices, une
 * mise à jour de Documents, et « Tests et couverture » à installer.
 * `?scenario=offline` : catalogue servi depuis le cache, dépôt injoignable.
 */
import type { CatalogEntryView, CatalogView, InstalledPluginView } from "@/lib/types";

type Base = Omit<
  CatalogEntryView,
  "installed" | "installedSource" | "updateAvailable" | "enabled" | "compatible" | "incompatibility"
>;

const entry = (
  e: Partial<Base> & Pick<Base, "id" | "name" | "version" | "category" | "color" | "railIcon">,
): Base => ({
  description: "",
  authors: ["Rémi Lhuillier"],
  iconUrl: null,
  homepage: `https://github.com/RemiSDV/cadence-plugins/tree/main/plugins/${e.id}`,
  size: 4_800_000,
  official: false,
  services: [],
  views: [],
  actions: [],
  tools: [],
  minCadence: "0.3.0",
  archive: `${e.id}-${e.version}.zip`,
  ...e,
});

export const mockCatalogEntries: Base[] = [
  entry({
    id: "gestion-projet",
    name: "Gestion de projet",
    version: "0.1.0",
    category: "projet",
    color: "blue",
    railIcon: "board",
    official: true,
    services: ["repos"],
    size: 18_234_112,
    description:
      "Issues GitHub, kanban, roadmap, sprints, versions et PR, sur un ou plusieurs dépôts, avec des boutons liés à des skills.",
    views: [
      { id: "sprint", title: "Sprint en cours", icon: "sprint" },
      { id: "issues", title: "Issues", icon: "issue" },
      { id: "roadmap", title: "Roadmap", icon: "roadmap" },
    ],
    actions: [
      { id: "preparer-issue", label: "Préparer l'issue", mode: "chat" },
      { id: "rapport-version", label: "Rapport de version", mode: "background" },
    ],
    tools: ["issues_list", "issue_get", "issue_create", "version_get", "pr_list"],
  }),
  entry({
    id: "documents",
    name: "Documents",
    version: "0.1.1",
    category: "etudes",
    color: "green",
    railIcon: "library",
    official: true,
    size: 42_100_000,
    description:
      "Cours en PDF, Word, PowerPoint et Markdown. Réponses citées à la page, QCM, questions ouvertes et flashcards.",
    views: [
      { id: "bibliotheque", title: "Bibliothèque", icon: "library" },
      { id: "flashcards", title: "Flashcards", icon: "layers" },
    ],
    actions: [{ id: "fiche", label: "Fiche de révision", mode: "chat" }],
    tools: ["documents_list", "page_read", "search", "quiz_create"],
  }),
  entry({
    id: "bibliographie",
    name: "Bibliographie",
    version: "0.2.3",
    category: "etudes",
    color: "purple",
    railIcon: "bookmark",
    authors: ["Inès Marchand"],
    description:
      "Bibliothèque Zotero ou BibTeX, citations dans le chat et bibliographie APA ou IEEE.",
  }),
  entry({
    id: "tests-couverture",
    name: "Tests et couverture",
    version: "0.3.1",
    category: "code",
    color: "clay",
    railIcon: "check",
    authors: ["Yanis Benali"],
    services: ["repos"],
    description:
      "Lance la suite de tests dans le worktree d'une tâche, lit la couverture par fichier et propose les tests qui manquent.",
    views: [
      { id: "couverture", title: "Couverture", icon: "check" },
      { id: "executions", title: "Exécutions", icon: "play" },
    ],
    actions: [
      { id: "lancer", label: "Lancer les tests", mode: "background" },
      { id: "manquants", label: "Proposer les tests manquants", mode: "chat" },
      { id: "instable", label: "Analyser un test instable", mode: "chat" },
    ],
    tools: ["lancer_suite", "lire_rapport", "couverture_fichier", "tests_instables", "historique"],
  }),
  entry({
    id: "explorateur-sql",
    name: "Explorateur SQL",
    version: "0.4.0",
    category: "donnees",
    color: "amber",
    railIcon: "database",
    authors: ["Théo Garnier"],
    description:
      "Parcourt une base SQLite ou PostgreSQL locale, requêtes en lecture seule et plans d'exécution.",
  }),
  entry({
    id: "journal",
    name: "Journal de bord",
    version: "1.0.2",
    category: "outils",
    color: "red",
    railIcon: "pen",
    authors: ["Camille Robert"],
    description:
      "Journal quotidien : ce qui a été fait, avec quelles sessions d'IA, exporté en Markdown.",
  }),
  entry({
    id: "mobile-preview",
    name: "Aperçu mobile",
    version: "0.1.0",
    category: "code",
    color: "purple",
    railIcon: "eye",
    authors: ["Léa Morin"],
    minCadence: "0.5.0",
    description: "Aperçu d'une app React Native dans un simulateur, piloté par l'IA.",
  }),
];

const newer = (a: string, b: string) => {
  const [x, y] = [a, b].map((v) => v.split(".").map(Number));
  for (let i = 0; i < 3; i++) if ((x![i] ?? 0) !== (y![i] ?? 0)) return (x![i] ?? 0) > (y![i] ?? 0);
  return false;
};

export function mockCatalogView(
  installed: InstalledPluginView[],
  offline: boolean,
  fetchedAt: string,
): CatalogView {
  const plugins = mockCatalogEntries.map((e): CatalogEntryView => {
    const local = installed.find((p) => p.id === e.id);
    const incompatibility =
      e.minCadence && newer(e.minCadence, "0.3.0")
        ? `Nécessite Cadence ${e.minCadence} ou plus récent.`
        : null;
    return {
      ...e,
      installed: local?.version ?? null,
      installedSource: local?.source ?? null,
      updateAvailable: !!local && local.source !== "dev" && newer(e.version, local.version),
      enabled: local?.enabled ?? null,
      compatible: incompatibility === null,
      incompatibility,
    };
  });
  return {
    url: "https://github.com/RemiSDV/cadence-plugins/releases/download/catalog/catalog.json",
    plugins,
    fetchedAt,
    offline,
    error: offline ? "réseau indisponible : github.com injoignable" : null,
    updates: plugins.filter((p) => p.updateAvailable).length,
  };
}
