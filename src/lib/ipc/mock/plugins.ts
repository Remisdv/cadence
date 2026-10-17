/**
 * Plugins factices du mode mock, repris du rail de la maquette V2-Chat.dc.html : Gestion de
 * projet (Projet), Documents et Bibliographie (Études), Explorateur SQL (Données), Journal de
 * bord (Outils), plus un Minuteur installé mais désactivé.
 *
 * `?scenario=empty` : aucun plugin installé (rail réduit au chat, au catalogue et aux réglages).
 */
import type {
  AppSettings,
  InstalledPluginView,
  PluginProcessView,
  PluginSettingsView,
  PluginViewInfo,
} from "@/lib/types";

const view = (
  id: string,
  title: string,
  icon: string,
  section: string | null,
  extra: Partial<PluginViewInfo> = {},
): PluginViewInfo => ({ id, title, icon, section, hidden: false, home: false, ...extra });

const plugin = (
  p: Pick<InstalledPluginView, "id" | "name" | "category" | "color" | "railIcon" | "views"> &
    Partial<InstalledPluginView>,
): InstalledPluginView => ({
  version: "0.1.0",
  description: "",
  secrets: [],
  services: [],
  setup: [],
  hasSettings: false,
  uiEntry: null,
  enabled: true,
  source: "catalog",
  status: "ok",
  problem: null,
  ...p,
});

export const mockPlugins: InstalledPluginView[] = [
  plugin({
    id: "gestion-projet",
    name: "Gestion de projet",
    description: "Sprints, issues, pull requests et versions de vos dépôts GitHub.",
    category: "projet",
    color: "blue",
    railIcon: "board",
    services: ["repos"],
    hasSettings: true,
    views: [
      view("sprint", "Sprint en cours", "sprint", "Pilotage", { home: true }),
      view("roadmap", "Roadmap", "roadmap", "Pilotage"),
      view("versions", "Versions", "tag", "Pilotage"),
      view("issues", "Issues", "issue", "Travail"),
      view("pull-requests", "Pull requests", "pullRequest", "Travail"),
      view("reviews", "Revues à faire", "review", "Travail"),
      view("issue", "Issue", "issue", null, { hidden: true }),
    ],
  }),
  plugin({
    id: "documents",
    name: "Documents",
    description: "Cours importés, lus page par page, avec QCM, questions ouvertes et flashcards.",
    category: "etudes",
    color: "green",
    railIcon: "library",
    views: [
      view("bibliotheque", "Bibliothèque", "library", "Cours", { home: true }),
      view("entrainement", "Entraînement", "play", "Réviser"),
      view("flashcards", "Flashcards", "layers", "Réviser"),
      view("resultats", "Résultats", "check", "Réviser"),
      view("document", "Document", "file", null, { hidden: true }),
    ],
  }),
  plugin({
    id: "bibliographie",
    name: "Bibliographie",
    description: "Références BibTeX d'un mémoire : DOI manquants, doublons, citations.",
    category: "etudes",
    color: "purple",
    railIcon: "bookmark",
    secrets: [
      {
        id: "zotero_key",
        label: "Clé d'API Zotero",
        description: "Pour synchroniser votre bibliothèque Zotero.",
        required: true,
      },
    ],
    status: "needs-setup",
    views: [
      view("references", "Références", "bookmark", null, { home: true }),
      view("a-completer", "À compléter", "warning", null),
    ],
  }),
  plugin({
    id: "explorateur-sql",
    name: "Explorateur SQL",
    description: "Requêtes et schéma de vos bases SQLite et PostgreSQL.",
    category: "donnees",
    color: "amber",
    railIcon: "database",
    views: [
      view("requetes", "Requêtes", "database", null, { home: true }),
      view("schema", "Schéma", "layers", null),
    ],
  }),
  plugin({
    id: "journal",
    name: "Journal de bord",
    description: "Notes quotidiennes, relues par l'IA pour préparer la semaine.",
    category: "outils",
    color: "red",
    railIcon: "pen",
    source: "dev",
    views: [
      view("entrees", "Entrées", "pen", null, { home: true }),
      view("calendrier", "Calendrier", "calendar", null),
    ],
  }),
  plugin({
    id: "exemple",
    name: "Exemple",
    description: "Plugin de démonstration du SDK : vues, outils, événements, réglages.",
    category: "outils",
    color: "clay",
    railIcon: "grid",
    uiEntry: "index.html",
    source: "dev",
    views: [
      view("accueil", "Accueil", "grid", null, { home: true }),
      view("outils", "Outils", "wrench", null),
    ],
  }),
  plugin({
    id: "minuteur",
    name: "Minuteur",
    description: "Sessions de travail minutées.",
    category: "outils",
    color: "clay",
    railIcon: "clock",
    enabled: false,
    views: [view("minuteur", "Minuteur", "clock", null, { home: true })],
  }),
];

/** Plugin installé depuis une archive ou un dossier de dev (mode mock). */
export const mockInstalledPlugin = (
  id: string,
  version: string,
  source: InstalledPluginView["source"],
): InstalledPluginView =>
  plugin({
    id,
    name: id.charAt(0).toUpperCase() + id.slice(1).replace(/-/g, " "),
    version,
    category: "outils",
    color: "clay",
    railIcon: "layers",
    source,
    description: "Plugin installé depuis un fichier local.",
    views: [view("accueil", "Accueil", "layers", null, { home: true })],
  });

/** Serveurs MCP factices : Gestion de projet et Documents tournent, le Journal est arrêté. */
export function mockProcesses(crashed: boolean): PluginProcessView[] {
  const log = (id: string) =>
    `C:/Users/moi/AppData/Roaming/app.cadence.desktop/logs/plugins/${id}.log`;
  return [
    crashed
      ? {
          plugin: "gestion-projet",
          state: "failed",
          pid: null,
          restarts: 3,
          tools: 0,
          message:
            "le serveur s'est arrêté (code 101). 4 arrêts en 5 min : redémarrage automatique abandonné.",
          logPath: log("gestion-projet"),
        }
      : {
          plugin: "gestion-projet",
          state: "ready",
          pid: 4812,
          restarts: 0,
          tools: 24,
          message: null,
          logPath: log("gestion-projet"),
        },
    {
      plugin: "documents",
      state: "ready",
      pid: 5120,
      restarts: 0,
      tools: 11,
      message: null,
      logPath: log("documents"),
    },
    {
      plugin: "journal",
      state: "stopped",
      pid: null,
      restarts: 0,
      tools: 0,
      message: null,
      logPath: null,
    },
    {
      plugin: "exemple",
      state: "ready",
      pid: 6001,
      restarts: 0,
      tools: 4,
      message: null,
      logPath: log("exemple"),
    },
  ];
}

export const mockAppSettings: AppSettings = {
  version: 2,
  onboarded: true,
  defaultAi: "claude-code",
  ai: {
    claudeCode: { path: null, enabled: true },
    codex: { path: null, enabled: true },
    opencode: { path: null, enabled: false },
  },
  actionAi: {},
  catalogUrl: null,
  devPlugins: ["C:/Users/moi/dev/cadence-plugins/plugins/journal"],
  toolPaths: {},
  ignoredTools: [],
  trackedRepos: { "gestion-projet": ["cadence-app/desktop"] },
};

/** Schéma de réglages de Gestion de projet (extrait des réglages de `.cadence/config.toml`). */
export const mockSettingsSchema: Record<string, PluginSettingsView["schema"]> = {
  "gestion-projet": {
    type: "object",
    required: ["sprintDays"],
    properties: {
      sprintDays: {
        type: "integer",
        title: "Durée d'un sprint (jours)",
        minimum: 1,
        maximum: 60,
        default: 14,
      },
      tagPrefix: { type: "string", title: "Préfixe des tags", maxLength: 10, default: "v" },
      confirmMajorBump: {
        type: "boolean",
        title: "Confirmer un saut de version majeure",
        default: true,
      },
    },
  },
};

type JsonSchemaProperty = {
  type?: string;
  default?: unknown;
  enum?: unknown[];
  minimum?: number;
  maximum?: number;
  maxLength?: number;
};

/** Même sous-ensemble que `plugins/settings.rs` (assez pour les réglages factices). */
export function validateMockSettings(
  schema: PluginSettingsView["schema"],
  value: Record<string, unknown>,
): PluginSettingsView["errors"] {
  if (!schema) return [];
  const props = (schema.properties ?? {}) as Record<string, JsonSchemaProperty>;
  const required = (schema.required ?? []) as string[];
  const errors: PluginSettingsView["errors"] = required
    .filter((key) => value[key] === undefined || value[key] === null)
    .map((field) => ({ field, message: "Réglage obligatoire." }));
  for (const [field, v] of Object.entries(value).sort(([a], [b]) => a.localeCompare(b))) {
    const prop = props[field];
    if (!prop) {
      errors.push({ field, message: "Réglage inconnu de ce plugin." });
      continue;
    }
    if (prop.type === "integer" && !Number.isInteger(v)) {
      errors.push({ field, message: "Nombre entier attendu." });
    } else if (prop.type === "boolean" && typeof v !== "boolean") {
      errors.push({ field, message: "Oui ou non attendu." });
    } else if (prop.type === "string" && typeof v !== "string") {
      errors.push({ field, message: "Texte attendu." });
    } else if (typeof v === "number" && prop.minimum !== undefined && v < prop.minimum) {
      errors.push({ field, message: `Au moins ${prop.minimum}.` });
    } else if (typeof v === "number" && prop.maximum !== undefined && v > prop.maximum) {
      errors.push({ field, message: `Au plus ${prop.maximum}.` });
    } else if (typeof v === "string" && prop.maxLength !== undefined && v.length > prop.maxLength) {
      errors.push({ field, message: `Au plus ${prop.maxLength} caractères.` });
    }
  }
  return errors;
}

/** Valeurs complétées par les défauts du schéma. */
export function withMockDefaults(
  schema: PluginSettingsView["schema"],
  value: Record<string, unknown>,
): Record<string, unknown> {
  const props = (schema?.properties ?? {}) as Record<string, JsonSchemaProperty>;
  const out = { ...value };
  for (const [key, prop] of Object.entries(props)) {
    if (!(key in out) && prop.default !== undefined) out[key] = prop.default;
  }
  return out;
}
