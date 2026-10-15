/**
 * Types du contrat front / back, générés depuis Rust par ts-rs (`pnpm bindings`).
 * Importer depuis ce fichier plutôt que depuis `./bindings/*` directement.
 */

// Erreurs
export type { ErrorKind } from "./bindings/ErrorKind";
export type { IpcError } from "./bindings/IpcError";

// Commun
export type { UserRef } from "./bindings/UserRef";

// Auth
export type { AuthProvider } from "./bindings/AuthProvider";
export type { ProviderStatus } from "./bindings/ProviderStatus";
export type { GithubAuth } from "./bindings/GithubAuth";
export type { CliTest } from "./bindings/CliTest";
export type { CliAgentInfo } from "./bindings/CliAgentInfo";
export type { GithubSources } from "./bindings/GithubSources";
export type { AuthStatus } from "./bindings/AuthStatus";
export type { DeviceFlowStart } from "./bindings/DeviceFlowStart";
export type { WebFlowStart } from "./bindings/WebFlowStart";
export type { AuthFailure } from "./bindings/AuthFailure";

// Projet
export type { Repo } from "./bindings/Repo";
export type { ProjectState } from "./bindings/ProjectState";
export type { OwnerKind } from "./bindings/OwnerKind";
export type { RepoOwner } from "./bindings/RepoOwner";
export type { RepoCreateInput } from "./bindings/RepoCreateInput";

// Versions
export type { VersionKind } from "./bindings/VersionKind";
export type { VersionState } from "./bindings/VersionState";
export type { Version } from "./bindings/Version";
export type { BurndownPoint } from "./bindings/BurndownPoint";
export type { VersionDetail } from "./bindings/VersionDetail";
export type { VersionReport } from "./bindings/VersionReport";
export type { CarryOver } from "./bindings/CarryOver";
export type { VersionCloseInput } from "./bindings/VersionCloseInput";
export type { CloseReport } from "./bindings/CloseReport";

// Issues
export type { IssueType } from "./bindings/IssueType";
export type { IssueStatus } from "./bindings/IssueStatus";
export type { IssueFlag } from "./bindings/IssueFlag";
export type { IssueRow } from "./bindings/IssueRow";
export type { CtoQuestion } from "./bindings/CtoQuestion";
export type { IssueDetail } from "./bindings/IssueDetail";
export type { IssueFilter } from "./bindings/IssueFilter";
export type { IssueCreateInput } from "./bindings/IssueCreateInput";
export type { CtoAnswer } from "./bindings/CtoAnswer";

// Pull requests
export type { PrState } from "./bindings/PrState";
export type { CiState } from "./bindings/CiState";
export type { ReviewState } from "./bindings/ReviewState";
export type { PrRow } from "./bindings/PrRow";
export type { PrFile } from "./bindings/PrFile";
export type { PrDetail } from "./bindings/PrDetail";
export type { PrFilter } from "./bindings/PrFilter";
export type { ReviewAudience } from "./bindings/ReviewAudience";
export type { ReviewRow } from "./bindings/ReviewRow";
export type { AgentRunDetail } from "./bindings/AgentRunDetail";
export type { AgentRunEntry } from "./bindings/AgentRunEntry";
export type { AgentLogLine } from "./bindings/AgentLogLine";
export type { CtoItem } from "./bindings/CtoItem";
export type { CtoItemState } from "./bindings/CtoItemState";
export type { DevTask } from "./bindings/DevTask";
export type { DevTaskState } from "./bindings/DevTaskState";
export type { DevFile } from "./bindings/DevFile";

// Agents
export type { AgentKind } from "./bindings/AgentKind";
export type { Autonomy } from "./bindings/Autonomy";
export type { RunState } from "./bindings/RunState";
export type { AgentActivity } from "./bindings/AgentActivity";
export type { RunSummary } from "./bindings/RunSummary";
export type { AgentOverview } from "./bindings/AgentOverview";
export type { AgentInput } from "./bindings/AgentInput";
export type { AgentControl } from "./bindings/AgentControl";
export type { AgentAction } from "./bindings/AgentAction";
export type { ProposedAction } from "./bindings/ProposedAction";
export type { ApplyFailure } from "./bindings/ApplyFailure";
export type { ApplyReport } from "./bindings/ApplyReport";

// Réglages
export type { ProjectConfig } from "./bindings/ProjectConfig";
export type { VersioningConfig } from "./bindings/VersioningConfig";
export type { IssuesConfig } from "./bindings/IssuesConfig";
export type { AgentEngine } from "./bindings/AgentEngine";
export type { EngineChoice } from "./bindings/EngineChoice";
export type { AgentEngines } from "./bindings/AgentEngines";
export type { AgentConfig } from "./bindings/AgentConfig";
export type { AgentsConfig } from "./bindings/AgentsConfig";
export type { Config } from "./bindings/Config";

// Sync
export type { SyncResource } from "./bindings/SyncResource";
export type { RateLimitMode } from "./bindings/RateLimitMode";

// Événements
export type { SyncUpdated } from "./bindings/SyncUpdated";
export type { RateLimitInfo } from "./bindings/RateLimitInfo";
export type { AgentEvent } from "./bindings/AgentEvent";
export type { AgentEventKind } from "./bindings/AgentEventKind";
export type { AgentStateEvent } from "./bindings/AgentStateEvent";
export type { AuthChanged } from "./bindings/AuthChanged";
