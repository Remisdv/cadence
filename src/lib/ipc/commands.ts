/**
 * Contrat des commandes Tauri : nom → arguments + retour.
 * Doit refléter `src-tauri/src/commands/*` (enregistrées dans `lib.rs`).
 * Les types viennent de ts-rs : si Rust change une structure, ce fichier casse à la compilation.
 */
import type {
  AuthProvider,
  AuthStatus,
  CliAgentInfo,
  CloseReport,
  DeviceFlowStart,
  GithubAuth,
  IssueCreateInput,
  IssueDetail,
  IssueFilter,
  IssueRow,
  PrFilter,
  PrDetail,
  PrRow,
  ProjectState,
  Repo,
  RepoCreateInput,
  RepoOwner,
  ReviewRow,
  Version,
  VersionCloseInput,
  VersionDetail,
  VersionReport,
  WebFlowStart,
} from "@/lib/types";

/** Commande sans argument : `keyof` vide, l'argument devient facultatif dans `call()`. */
type NoArgs = Record<never, never>;

export interface Commands {
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
