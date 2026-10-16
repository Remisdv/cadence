/**
 * Hooks de données, un par intention métier. Les vues n'appellent jamais `call()` directement.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { call } from "@/lib/ipc/transport";
import type {
  AppSettingsPatch,
  AuthProvider,
  AuthStatus,
  CliAgentInfo,
  InstalledPluginView,
  IssueCreateInput,
  IssueFilter,
  PrFilter,
  RepoCreateInput,
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
    },
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
