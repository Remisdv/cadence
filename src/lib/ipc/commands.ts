/**
 * Contrat des commandes Tauri : nom → arguments + retour.
 * Doit refléter `src-tauri/src/commands/*` (enregistrées dans `lib.rs`).
 * Les types viennent de ts-rs : si Rust change une structure, ce fichier casse à la compilation.
 */
import type {
  ActionRunInput,
  ActionRunResult,
  AiId,
  AppSettings,
  AppSettingsPatch,
  AppStatus,
  AuthProvider,
  ApprovalDecision,
  AuthStatus,
  CatalogView,
  ChatSendInput,
  ChatSendResult,
  ChatSessionDetail,
  ChatSessionRow,
  CliAgentInfo,
  CliTest,
  CloseReport,
  DeviceFlowStart,
  GithubAuth,
  InstalledPluginView,
  IssueCreateInput,
  IssueDetail,
  IssueFilter,
  IssueRow,
  PrFilter,
  PrDetail,
  PrRow,
  PluginProcessView,
  PluginToolResult,
  PluginSettingsView,
  ProjectState,
  Repo,
  RepoCheckout,
  RepoCloneView,
  RepoCreateInput,
  RepoOwner,
  ReviewRow,
  Version,
  VersionCloseInput,
  VersionDetail,
  VersionReport,
  WebFlowStart,
  ToolId,
  ToolRunResult,
  ToolsReport,
} from "@/lib/types";

/** Commande sans argument : `keyof` vide, l'argument devient facultatif dans `call()`. */
type NoArgs = Record<never, never>;

export interface Commands {
  // App (Cadence 2)
  app_status: { args: NoArgs; result: AppStatus };
  app_settings_get: { args: NoArgs; result: AppSettings };
  app_settings_set: { args: { patch: AppSettingsPatch }; result: AppSettings };
  // Registre d'outils : rien ne s'installe sans clic, la commande exacte est affichée.
  /** Détection au premier appel puis cache ; `refresh` relance tout (« Revérifier »). */
  tools_list: { args: { refresh: boolean }; result: ToolsReport };
  /** Journal par `tool://install` ; réponse à la fin, outil redétecté. `recipe` null = la première utilisable. */
  tool_install: { args: { tool: ToolId; recipe: string | null }; result: ToolRunResult };
  /** Connexion guidée (navigateur, code d'appairage dans le journal, ou terminal du système). */
  tool_login: { args: { tool: ToolId }; result: ToolRunResult };
  tool_cancel: { args: { tool: ToolId }; result: boolean };
  /** « Localiser le binaire… » ; `null` = détection automatique. */
  tool_set_path: { args: { tool: ToolId; path: string | null }; result: ToolsReport };
  /** Session de test de quelques secondes d'une IA de code. */
  ai_test: { args: { ai: AiId }; result: CliTest };
  // Plugins installés
  plugins_installed: { args: NoArgs; result: InstalledPluginView[] };
  plugins_set_enabled: { args: { id: string; enabled: boolean }; result: InstalledPluginView };
  plugins_reorder: { args: { order: string[] }; result: InstalledPluginView[] };
  plugin_settings_get: { args: { plugin: string }; result: PluginSettingsView };
  plugin_settings_set: {
    args: { plugin: string; value: Record<string, unknown> };
    result: PluginSettingsView;
  };
  /** Écriture seule : aucune commande ne renvoie la valeur d'un secret. */
  plugin_secret_set: { args: { plugin: string; secret: string; value: string }; result: null };
  plugin_secret_has: { args: { plugin: string; secret: string }; result: boolean };
  plugin_secret_delete: { args: { plugin: string; secret: string }; result: null };
  /** Archive zip locale ; progression par `plugin://install`. */
  plugin_install_zip: {
    args: { path: string; sha256: string | null };
    result: InstalledPluginView;
  };
  /** Dossier local non zippé (mode développement), retenu dans `devPlugins`. */
  plugin_install_dev: { args: { path: string }; result: InstalledPluginView };
  plugin_uninstall: { args: { id: string; deleteSecrets: boolean }; result: null };
  /** Serveurs MCP des plugins installés qui en déclarent un ; changements par `plugin://state`. */
  plugin_status: { args: NoArgs; result: PluginProcessView[] };
  plugin_restart: { args: { plugin: string }; result: null };
  /** Catalogue en ligne (cache de 6 h, `refresh` force la relecture ; hors ligne : le cache). */
  catalog_get: { args: { refresh: boolean }; result: CatalogView };
  /** Plugin du catalogue ; progression par `plugin://install`. */
  plugin_install: { args: { id: string }; result: InstalledPluginView };
  plugin_update: { args: { id: string }; result: InstalledPluginView };
  /** Pont de l'iframe : outil du plugin (outils `ui` compris), nom sans préfixe. */
  plugin_call: {
    args: { plugin: string; tool: string; args: Record<string, unknown> | null };
    result: PluginToolResult;
  };
  // Service Dépôts GitHub
  /** Copie locale de chaque dépôt suivi (clonée, désignée ou pas encore clonée). */
  repos_status: { args: NoArgs; result: RepoCloneView[] };
  /** Pont de l'iframe (`repos.checkout`) : clone au besoin, worktree de la tâche. */
  repos_checkout: {
    args: { plugin: string; repo: string; task: string | null };
    result: RepoCheckout;
  };
  /** « Désigner un clone existant… » ; `path` nul = revenir au clone de Cadence. */
  repos_link: { args: { repo: string; path: string | null }; result: RepoCloneView };
  // Chat du cœur (réponses par `chat://event` et `chat://state`)
  chat_sessions: { args: NoArgs; result: ChatSessionRow[] };
  chat_session_get: { args: { id: string }; result: ChatSessionDetail };
  chat_send: { args: { input: ChatSendInput }; result: ChatSendResult };
  chat_interrupt: { args: { session: string }; result: null };
  /** Réponse à une question de l'IA (Codex, OpenCode). */
  chat_answer: { args: { session: string; request: string; answer: string }; result: null };
  /** Action d'un plugin : conversation (mode chat) ou exécution de fond. */
  action_run: { args: { input: ActionRunInput }; result: ActionRunResult };
  chat_approve: {
    args: { session: string; request: string; decision: ApprovalDecision };
    result: null;
  };
  chat_session_rename: { args: { id: string; title: string }; result: ChatSessionRow };
  chat_session_delete: { args: { id: string }; result: null };
  // Auth
  auth_start_device_flow: { args: NoArgs; result: DeviceFlowStart };
  auth_start_web_flow: { args: NoArgs; result: WebFlowStart };
  auth_use_gh_cli: { args: NoArgs; result: GithubAuth };
  auth_status: { args: NoArgs; result: AuthStatus };
  auth_set_token: { args: { token: string }; result: GithubAuth };
  auth_sign_out: { args: NoArgs; result: null };
  cli_agent_test: { args: { provider: AuthProvider }; result: CliAgentInfo };
  cli_agent_set_path: { args: { provider: AuthProvider; path: string }; result: CliAgentInfo };
  cli_agent_set_enabled: {
    args: { provider: AuthProvider; enabled: boolean };
    result: CliAgentInfo;
  };
  // Projet
  project_current: { args: NoArgs; result: ProjectState | null };
  project_list_repos: { args: { query: string }; result: Repo[] };
  project_open: { args: { repo: string }; result: ProjectState };
  project_owners: { args: NoArgs; result: RepoOwner[] };
  project_create: { args: { input: RepoCreateInput }; result: ProjectState };
  // Versions
  versions_list: { args: NoArgs; result: Version[] };
  version_current: { args: NoArgs; result: VersionDetail };
  version_detail: { args: { title: string | null }; result: VersionReport };
  version_close: { args: { input: VersionCloseInput }; result: CloseReport };
  // Issues
  issues_list: { args: { filter: IssueFilter }; result: IssueRow[] };
  issue_get: { args: { number: number }; result: IssueDetail };
  issue_create: { args: { input: IssueCreateInput }; result: IssueRow };
  // Pull requests & revues
  prs_list: { args: { filter: PrFilter }; result: PrRow[] };
  reviews_pending: { args: NoArgs; result: ReviewRow[] };
  pr_detail: { args: { number: number }; result: PrDetail };
}

export type CommandName = keyof Commands;
export type CommandArgs<C extends CommandName> = Commands[C]["args"];
export type CommandResult<C extends CommandName> = Commands[C]["result"];
