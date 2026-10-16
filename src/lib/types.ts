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
export type { IssueDetail } from "./bindings/IssueDetail";
export type { IssueFilter } from "./bindings/IssueFilter";
export type { IssueCreateInput } from "./bindings/IssueCreateInput";

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

// Réglages
export type { ProjectConfig } from "./bindings/ProjectConfig";
export type { VersioningConfig } from "./bindings/VersioningConfig";
export type { IssuesConfig } from "./bindings/IssuesConfig";
export type { Config } from "./bindings/Config";

// Sync
export type { SyncResource } from "./bindings/SyncResource";
export type { RateLimitMode } from "./bindings/RateLimitMode";

// Événements
export type { SyncUpdated } from "./bindings/SyncUpdated";
export type { RateLimitInfo } from "./bindings/RateLimitInfo";
export type { AuthChanged } from "./bindings/AuthChanged";
