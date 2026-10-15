/**
 * Hooks de données, un par intention métier. Les vues n'appellent jamais `call()` directement.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { call } from "@/lib/ipc/transport";
import type {
  AgentControl,
  AgentEngines,
  AgentInput,
  AgentKind,
  AuthProvider,
  AuthStatus,
  CliAgentInfo,
  Config,
  CtoAnswer,
  IssueCreateInput,
  IssueFilter,
  PrFilter,
  RepoCreateInput,
  VersionCloseInput,
} from "@/lib/types";
import { keys } from "./keys";

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

export const useAgentEngines = () =>
  useQuery({ queryKey: keys.agentEngines(), queryFn: () => call("agent_engines_get") });

export function useSaveAgentEngines() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (engines: AgentEngines) => call("agent_engines_set", { engines }),
    // Optimiste : le select reflète le choix tout de suite.
    onMutate: (engines) => qc.setQueryData(keys.agentEngines(), engines),
    onSuccess: (engines) => qc.setQueryData(keys.agentEngines(), engines),
    onError: () => qc.invalidateQueries({ queryKey: keys.agentEngines() }),
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

export function useAnswerCto(number: number) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (answers: CtoAnswer[]) => call("issue_answer_cto", { number, answers }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.issue(number) });
      void qc.invalidateQueries({ queryKey: keys.agents });
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

/* ---------- Agents ---------- */

export const useAgentsOverview = () =>
  useQuery({ queryKey: keys.agentsOverview(), queryFn: () => call("agents_overview") });

export function useStartAgent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ kind, input }: { kind: AgentKind; input: AgentInput }) =>
      call("agent_start", { kind, input }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

/** Dernière exécution d'un agent (file, journal) ; rafraîchie à chaque `agent://state`. */
export const useAgentLatestRun = (kind: AgentKind) =>
  useQuery({ queryKey: keys.agentLatestRun(kind), queryFn: () => call("agent_latest_run", { kind }) });

/** Pause (après l'étape en cours), reprise, ou intervention dans le terminal de l'agent. */
export function useControlAgent(run: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (cmd: AgentControl) => call("agent_control", { run, cmd }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

export function useStopAgent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (run: string) => call("agent_control", { run, cmd: { cmd: "stop" } }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

/** Passe des issues de la file sans rien écrire. */
export function useRejectItems(run: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (numbers: number[]) => call("agent_reject", { run, numbers }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

/** Remplace la description proposée (« Éditer avant d'appliquer »). */
export function useEditProposal(run: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ number, body }: { number: number; body: string }) =>
      call("agent_edit_body", { run, number, body }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

/** Journal des exécutions récentes, tous agents confondus ; rafraîchi à chaque `agent://state`. */
export const useAgentRuns = (limit: number) =>
  useQuery({
    queryKey: keys.agentRunsList(limit),
    queryFn: () => call("agent_runs_list", { limit }),
    placeholderData: (previous) => previous,
  });

/** Même commande pour plusieurs exécutions (« Tout suspendre » de la console). */
export function useControlAgents() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ runs, cmd }: { runs: string[]; cmd: AgentControl }) =>
      Promise.all(runs.map((run) => call("agent_control", { run, cmd }))),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.agents }),
  });
}

export const useAgentActions = (run: string | null) =>
  useQuery({
    queryKey: keys.agentActions(run ?? ""),
    queryFn: () => call("agent_actions", { run: run! }),
    enabled: run !== null,
  });

export function useApplyActions(run: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => call("agent_apply", { run, ids }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.agents });
      void qc.invalidateQueries({ queryKey: keys.issues });
    },
  });
}

/* ---------- Réglages ---------- */

export const useSettings = () =>
  useQuery({ queryKey: keys.settings, queryFn: () => call("settings_get") });

export function useSaveSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (config: Config) => call("settings_set", { config }),
    onSuccess: (config) => {
      qc.setQueryData(keys.settings, config);
      // `hasConfig` du projet passe à vrai au premier enregistrement.
      void qc.invalidateQueries({ queryKey: keys.project });
      // L'autonomie des agents (rail, console) vient du fichier.
      void qc.invalidateQueries({ queryKey: keys.agents });
    },
  });
}
