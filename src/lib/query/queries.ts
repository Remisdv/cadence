/**
 * Hooks de données, un par intention métier. Les vues n'appellent jamais `call()` directement.
 */
import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { call } from "@/lib/ipc/transport";
import { buildThread } from "@/lib/domain/chat";
import { startLog, type ToolRunLog } from "@/lib/domain/tools";
import type {
  ApprovalDecision,
  AppSettingsPatch,
  ChatSendInput,
  AuthProvider,
  AuthStatus,
  CliAgentInfo,
  InstalledPluginView,
  IssueCreateInput,
  IssueFilter,
  PrFilter,
  AiId,
  RepoCreateInput,
  ToolId,
  ToolRunResult,
  VersionCloseInput,
} from "@/lib/types";
import { keys } from "./keys";

/* ---------- App et préférences de la machine ---------- */

/** Écran de départ (onboarding de Cadence 2 terminé ?) et version de l'app. */
export const useAppStatus = () =>
  useQuery({ queryKey: keys.appStatus(), queryFn: () => call("app_status") });

export const useAppSettings = () =>
  useQuery({ queryKey: keys.appSettings(), queryFn: () => call("app_settings_get") });

export function useSaveAppSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (patch: AppSettingsPatch) => call("app_settings_set", { patch }),
    onSuccess: (settings) => {
      qc.setQueryData(keys.appSettings(), settings);
      void qc.invalidateQueries({ queryKey: keys.appStatus() });
      // IA par défaut, IA ignorées, outils écartés : les demandeurs changent.
      void qc.invalidateQueries({ queryKey: keys.toolsReport() });
    },
  });
}

/* ---------- Registre d'outils ---------- */

/** Outils de la machine (détection en cache côté Rust ; « Revérifier » = `useRecheckTools`). */
export const useTools = () =>
  useQuery({
    queryKey: keys.toolsReport(),
    queryFn: () => call("tools_list", { refresh: false }),
    staleTime: 60_000,
  });

/** Relance toute la détection (« Revérifier », « Relancer la détection »). */
export function useRecheckTools() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => call("tools_list", { refresh: true }),
    onSuccess: (report) => qc.setQueryData(keys.toolsReport(), report),
  });
}

/** Journal en direct de la dernière installation ou connexion de l'outil. */
export const useToolRun = (tool: ToolId) =>
  useQuery<ToolRunLog | null>({
    queryKey: keys.toolRun(tool),
    queryFn: () => null,
    enabled: false,
    staleTime: Infinity,
    initialData: null,
  });

function useToolRunMutation<A extends { tool: ToolId; command: string }>(
  kind: "install" | "login",
  run: (args: A) => Promise<ToolRunResult>,
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: run,
    onMutate: ({ tool, command }) => {
      qc.setQueryData<ToolRunLog>(keys.toolRun(tool), startLog(kind, command));
    },
    onSettled: (result, error, { tool }) => {
      // Le résultat est aussi poussé par `tool://install` ; une erreur de lancement, non.
      if (error)
        qc.setQueryData<ToolRunLog>(keys.toolRun(tool), (prev) =>
          prev ? { ...prev, running: false } : prev,
        );
      else if (result)
        qc.setQueryData<ToolRunLog>(keys.toolRun(tool), (prev) =>
          prev ? { ...prev, running: false, result } : prev,
        );
      void qc.invalidateQueries({ queryKey: keys.toolsReport() });
      void qc.invalidateQueries({ queryKey: keys.auth });
    },
  });
}

/** Lance une recette d'installation (`recipe` null = la première utilisable). */
export const useInstallTool = () =>
  useToolRunMutation(
    "install",
    ({ tool, recipe }: { tool: ToolId; recipe: string | null; command: string }) =>
      call("tool_install", { tool, recipe }),
  );

/** « Se connecter » : commande de connexion de l'outil. */
export const useLoginTool = () =>
  useToolRunMutation("login", ({ tool }: { tool: ToolId; command: string }) =>
    call("tool_login", { tool }),
  );

export function useCancelTool() {
  return useMutation({ mutationFn: (tool: ToolId) => call("tool_cancel", { tool }) });
}

/** « Localiser le binaire… » ; `path` null = détection automatique. */
export function useSetToolPath() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ tool, path }: { tool: ToolId; path: string | null }) =>
      call("tool_set_path", { tool, path }),
    onSuccess: (report) => {
      qc.setQueryData(keys.toolsReport(), report);
      void qc.invalidateQueries({ queryKey: keys.appSettings() });
    },
  });
}

/** Session de test d'une IA ; le résultat s'affiche dans sa carte. */
export function useTestAi() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ai: AiId) => call("ai_test", { ai }),
    onSettled: () => void qc.invalidateQueries({ queryKey: keys.toolsReport() }),
  });
}

/* ---------- Plugins installés ---------- */

/** Plugins installés, dans l'ordre du rail (catégories, puis ordre choisi). */
export const usePlugins = () =>
  useQuery({ queryKey: keys.pluginsInstalled(), queryFn: () => call("plugins_installed") });

/** Remplace un plugin dans la liste en cache, sans tout relire. */
function usePatchPlugin() {
  const qc = useQueryClient();
  return (plugin: InstalledPluginView) =>
    qc.setQueryData<InstalledPluginView[]>(keys.pluginsInstalled(), (prev) =>
      prev?.map((p) => (p.id === plugin.id ? plugin : p)),
    );
}

export function useSetPluginEnabled() {
  const patch = usePatchPlugin();
  return useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      call("plugins_set_enabled", { id, enabled }),
    onSuccess: patch,
  });
}

/** Nouvel ordre du rail : optimiste, la liste réordonnée revient du back. */
export function useReorderPlugins() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (order: string[]) => call("plugins_reorder", { order }),
    onMutate: (order) =>
      qc.setQueryData<InstalledPluginView[]>(keys.pluginsInstalled(), (prev) =>
        prev ? order.map((id) => prev.find((p) => p.id === id)!).filter(Boolean) : prev,
      ),
    onSuccess: (plugins) => qc.setQueryData(keys.pluginsInstalled(), plugins),
    onError: () => qc.invalidateQueries({ queryKey: keys.pluginsInstalled() }),
  });
}

/** Réglages globaux d'un plugin, avec son schéma. */
export const usePluginSettings = (plugin: string | null) =>
  useQuery({
    queryKey: keys.pluginSettings(plugin ?? ""),
    queryFn: () => call("plugin_settings_get", { plugin: plugin! }),
    enabled: plugin !== null,
  });

/** Enregistre si la valeur respecte le schéma ; les erreurs par champ reviennent dans `data`. */
export function useSavePluginSettings(plugin: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (value: Record<string, unknown>) => call("plugin_settings_set", { plugin, value }),
    onSuccess: (view) => {
      if (view.errors.length === 0) qc.setQueryData(keys.pluginSettings(plugin), view);
    },
  });
}

/** Le secret est-il au trousseau ? (sa valeur n'est jamais lue par le front). */
export const usePluginSecret = (plugin: string, secret: string) =>
  useQuery({
    queryKey: keys.pluginSecret(plugin, secret),
    queryFn: () => call("plugin_secret_has", { plugin, secret }),
  });

/** Écrit (`value`) ou supprime (`null`) un secret, puis relit l'état du plugin. */
export function useSetPluginSecret(plugin: string, secret: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (value: string | null) =>
      value === null
        ? call("plugin_secret_delete", { plugin, secret })
        : call("plugin_secret_set", { plugin, secret, value }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.pluginSecret(plugin, secret) });
      void qc.invalidateQueries({ queryKey: keys.pluginsInstalled() });
    },
  });
}

/** Installe une archive zip locale (ou un dossier de dev), puis relit le rail et les préférences. */
export function useInstallPlugin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (
      source: { kind: "zip"; path: string; sha256?: string } | { kind: "dev"; path: string },
    ) =>
      source.kind === "zip"
        ? call("plugin_install_zip", { path: source.path, sha256: source.sha256 ?? null })
        : call("plugin_install_dev", { path: source.path }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.plugins });
      void qc.invalidateQueries({ queryKey: keys.appSettings() });
    },
  });
}

/** Catalogue en ligne, comparé aux plugins installés. */
export const useCatalog = () =>
  useQuery({
    queryKey: keys.catalog(),
    queryFn: () => call("catalog_get", { refresh: false }),
    staleTime: 5 * 60_000,
  });

/** Relit le catalogue sur le réseau (« Actualiser »). */
export function useRefreshCatalog() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => call("catalog_get", { refresh: true }),
    onSuccess: (view) => qc.setQueryData(keys.catalog(), view),
  });
}

/** Installe ou met à jour un plugin du catalogue (progression par `plugin://install`). */
export function useInstallFromCatalog() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, update }: { id: string; update: boolean }) =>
      update ? call("plugin_update", { id }) : call("plugin_install", { id }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.plugins }),
  });
}

/** Désinstalle un plugin ; ses secrets ne sont supprimés que si `deleteSecrets`. */
export function useUninstallPlugin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, deleteSecrets }: { id: string; deleteSecrets: boolean }) =>
      call("plugin_uninstall", { id, deleteSecrets }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.plugins });
      void qc.invalidateQueries({ queryKey: keys.appSettings() });
    },
  });
}

/** État des serveurs MCP des plugins (rafraîchi par `plugin://state`). */
export const usePluginStatus = () =>
  useQuery({ queryKey: keys.pluginStatus(), queryFn: () => call("plugin_status") });

/** Relance le serveur d'un plugin, même après un échec. */
export function useRestartPlugin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (plugin: string) => call("plugin_restart", { plugin }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.pluginStatus() }),
  });
}

/* ---------- Chat du cœur ---------- */

/** Conversations, la plus récemment active d'abord. */
export const useChatSessions = () =>
  useQuery({ queryKey: keys.chatSessions(), queryFn: () => call("chat_sessions") });

/** Historique d'une conversation, tenu à jour par `chat://event` (useBackendEvents). */
export const useChatSession = (id: string | null) =>
  useQuery({
    queryKey: keys.chatSession(id ?? ""),
    queryFn: () => call("chat_session_get", { id: id! }),
    enabled: id !== null,
  });

/** Fil affichable d'une conversation, textes en cours de diffusion compris. */
export function useChatThread(id: string | null) {
  const detail = useChatSession(id);
  const drafts = useQuery<Record<string, string>>({
    queryKey: keys.chatDrafts(id ?? ""),
    queryFn: () => ({}),
    enabled: false,
    initialData: {},
  }).data;
  const items = useMemo(
    () => buildThread(detail.data?.events ?? [], drafts),
    [detail.data?.events, drafts],
  );
  return { ...detail, items };
}

/** Envoie un message ; sans `session`, ouvre une conversation (renvoie son id). */
export function useSendChat() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: ChatSendInput) => call("chat_send", { input }),
    onSuccess: ({ session }) => {
      void qc.invalidateQueries({ queryKey: keys.chatSessions() });
      void qc.invalidateQueries({ queryKey: keys.chatSession(session) });
    },
  });
}

export function useInterruptChat() {
  return useMutation({ mutationFn: (session: string) => call("chat_interrupt", { session }) });
}

export function useApproveChat(session: string) {
  return useMutation({
    mutationFn: ({ request, decision }: { request: string; decision: ApprovalDecision }) =>
      call("chat_approve", { session, request, decision }),
  });
}

export function useRenameChat() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, title }: { id: string; title: string }) =>
      call("chat_session_rename", { id, title }),
    onSuccess: (row) => {
      void qc.invalidateQueries({ queryKey: keys.chatSessions() });
      void qc.invalidateQueries({ queryKey: keys.chatSession(row.id) });
    },
  });
}

export function useDeleteChat() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => call("chat_session_delete", { id }),
    onSuccess: (_, id) => {
      qc.removeQueries({ queryKey: keys.chatSession(id) });
      void qc.invalidateQueries({ queryKey: keys.chatSessions() });
    },
  });
}

/* ---------- GitHub (Cadence 1, futur plugin gestion-projet) ---------- */

/* ---------- Auth & projet ---------- */

export const useAuthStatus = () =>
  useQuery({ queryKey: keys.auth, queryFn: () => call("auth_status") });

export const useStartDeviceFlow = () =>
  useMutation({ mutationFn: () => call("auth_start_device_flow") });

/** Connexion par le navigateur : Rust ouvre la page d'autorisation et attend le retour. */
export const useStartWebFlow = () => useMutation({ mutationFn: () => call("auth_start_web_flow") });

/** Met à jour une partie de `auth_status` dans le cache, sans refaire la détection. */
function usePatchAuth() {
  const qc = useQueryClient();
  return (patch: Partial<AuthStatus>) =>
    qc.setQueryData<AuthStatus>(keys.auth, (prev) => (prev ? { ...prev, ...patch } : prev));
}

export function useSetGithubToken() {
  const patch = usePatchAuth();
  return useMutation({
    mutationFn: (token: string) => call("auth_set_token", { token }),
    onSuccess: (github) => patch({ github }),
  });
}

/** Reprend la session de la CLI GitHub (`gh`). */
export function useSignInWithGhCli() {
  const patch = usePatchAuth();
  return useMutation({
    mutationFn: () => call("auth_use_gh_cli"),
    onSuccess: (github) => patch({ github }),
  });
}

export function useSignOut() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => call("auth_sign_out"),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.auth }),
  });
}

const cliPatch = (provider: CliProvider, info: CliAgentInfo): Partial<AuthStatus> =>
  provider === "codex" ? { codex: info } : { claudeCode: info };

export type CliProvider = Exclude<AuthProvider, "github">;

export function useTestCliAgent(provider: CliProvider) {
  const patch = usePatchAuth();
  return useMutation({
    mutationFn: () => call("cli_agent_test", { provider }),
    onSuccess: (info) => patch(cliPatch(provider, info)),
  });
}

export function useSetCliPath(provider: CliProvider) {
  const patch = usePatchAuth();
  return useMutation({
    mutationFn: (path: string) => call("cli_agent_set_path", { provider, path }),
    onSuccess: (info) => patch(cliPatch(provider, info)),
  });
}

export function useSetCliEnabled(provider: CliProvider) {
  const patch = usePatchAuth();
  return useMutation({
    mutationFn: (enabled: boolean) => call("cli_agent_set_enabled", { provider, enabled }),
    onSuccess: (info) => patch(cliPatch(provider, info)),
  });
}

export const useCurrentProject = (enabled = true) =>
  useQuery({ queryKey: keys.projectCurrent(), queryFn: () => call("project_current"), enabled });

export const useRepos = (query: string) =>
  useQuery({
    queryKey: keys.repos(query),
    queryFn: () => call("project_list_repos", { query }),
    placeholderData: (previous) => previous, // pas de clignotement pendant la frappe
  });

export function useOpenProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (repo: string) => call("project_open", { repo }),
    onSuccess: (project) => {
      qc.setQueryData(keys.projectCurrent(), project);
      void qc.invalidateQueries();
    },
  });
}

/** Comptes sous lesquels créer un dépôt (chargés à l'ouverture du formulaire). */
export const useRepoOwners = (enabled: boolean) =>
  useQuery({ queryKey: keys.projectOwners(), queryFn: () => call("project_owners"), enabled });

/** Crée un dépôt sur GitHub puis l'ouvre ; la liste des dépôts est rafraîchie. */
export function useCreateRepo() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: RepoCreateInput) => call("project_create", { input }),
    onSuccess: (project) => {
      qc.setQueryData(keys.projectCurrent(), project);
      void qc.invalidateQueries();
    },
  });
}

/* ---------- Versions ---------- */

export const useVersions = () =>
  useQuery({ queryKey: keys.versionList(), queryFn: () => call("versions_list") });

export const useCurrentVersion = () =>
  useQuery({ queryKey: keys.versionCurrent(), queryFn: () => call("version_current") });

/**
 * Détail d'une version (sans titre : la version en cours) : rythme, issues reliées à leurs
 * PR, PR de la version. `live` : relit toutes les 60 s (cache côté Rust, ETag).
 */
export const useVersionDetail = (title: string | null = null, options: { live?: boolean } = {}) =>
  useQuery({
    queryKey: keys.versionDetail(title),
    queryFn: () => call("version_detail", { title }),
    refetchInterval: options.live ? 60_000 : false,
  });

export function useCloseVersion() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: VersionCloseInput) => call("version_close", { input }),
    onSuccess: () => qc.invalidateQueries(),
  });
}

/* ---------- Issues ---------- */

/**
 * Issues du dépôt ouvert. `live` : relit toutes les 60 s (côté Rust, une requête
 * conditionnelle ETag : rien n'est relu ni décompté si rien n'a changé).
 */
export const useIssues = (filter: IssueFilter = {}, options: { live?: boolean } = {}) =>
  useQuery({
    queryKey: keys.issueList(filter),
    queryFn: () => call("issues_list", { filter }),
    refetchInterval: options.live ? 60_000 : false,
  });

export const useIssue = (number: number) =>
  useQuery({ queryKey: keys.issue(number), queryFn: () => call("issue_get", { number }) });

export function useCreateIssue() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: IssueCreateInput) => call("issue_create", { input }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.issues });
      void qc.invalidateQueries({ queryKey: keys.versions });
    },
  });
}

/* ---------- Pull requests & revues ---------- */

/** PR ouvertes et récentes. `live` : relit toutes les 60 s (une requête GraphQL côté Rust). */
export const usePullRequests = (filter: PrFilter = {}, options: { live?: boolean } = {}) =>
  useQuery({
    queryKey: keys.prList(filter),
    queryFn: () => call("prs_list", { filter }),
    refetchInterval: options.live ? 60_000 : false,
  });

export const usePendingReviews = (options: { live?: boolean } = {}) =>
  useQuery({
    queryKey: keys.reviewsPending(),
    queryFn: () => call("reviews_pending"),
    refetchInterval: options.live ? 60_000 : false,
  });

/** PR, description et fichiers avec leurs patchs (écran de revue). */
export const usePrDetail = (number: number | null) =>
  useQuery({
    queryKey: keys.prDetail(number ?? 0),
    queryFn: () => call("pr_detail", { number: number! }),
    enabled: number !== null,
  });

/* ---------- Version de l'app et mise à jour automatique ---------- */

export {
  useAppUpdate,
  useAppVersion,
  useInstallUpdate,
  useRestartApp,
  useUpdateCheckOnLaunch,
} from "./updates";
