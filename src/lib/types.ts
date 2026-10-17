/**
 * Types du contrat front / back, générés depuis Rust par ts-rs (`pnpm bindings`).
 * Importer depuis ce fichier plutôt que depuis `./bindings/*` directement.
 */

// Erreurs
export type { ErrorKind } from "./bindings/ErrorKind";
export type { IpcError } from "./bindings/IpcError";

// Commun
export type { UserRef } from "./bindings/UserRef";

// App et préférences de la machine (Cadence 2)
export type { AppStatus } from "./bindings/AppStatus";
export type { ActionRunInput } from "./bindings/ActionRunInput";
export type { ActionRunResult } from "./bindings/ActionRunResult";
export type { AppDirs } from "./bindings/AppDirs";
export type { AiId } from "./bindings/AiId";
export type { AiCliSettings } from "./bindings/AiCliSettings";
export type { AiSettings } from "./bindings/AiSettings";
export type { AppSettings } from "./bindings/AppSettings";
export type { AiCliPatch } from "./bindings/AiCliPatch";
export type { AppSettingsPatch } from "./bindings/AppSettingsPatch";

// Registre d'outils (Git, GitHub CLI, IA de code, Node.js)
export type { ToolId } from "./bindings/ToolId";
export type { ToolStatus } from "./bindings/ToolStatus";
export type { ToolRequesterKind } from "./bindings/ToolRequesterKind";
export type { ToolRequester } from "./bindings/ToolRequester";
export type { ToolRecipe } from "./bindings/ToolRecipe";
export type { ToolInfo } from "./bindings/ToolInfo";
export type { ToolsReport } from "./bindings/ToolsReport";
export type { ToolRunKind } from "./bindings/ToolRunKind";
export type { ToolRunResult } from "./bindings/ToolRunResult";
export type { ToolRunEvent } from "./bindings/ToolRunEvent";

// Plugins
export type { PluginCategory } from "./bindings/PluginCategory";
export type { PluginColor } from "./bindings/PluginColor";
export type { PluginService } from "./bindings/PluginService";
export type { PluginSource } from "./bindings/PluginSource";
export type { PluginStatus } from "./bindings/PluginStatus";
export type { PluginViewInfo } from "./bindings/PluginViewInfo";
export type { PluginSecretInfo } from "./bindings/PluginSecretInfo";
export type { InstalledPluginView } from "./bindings/InstalledPluginView";
export type { ActionMode } from "./bindings/ActionMode";
export type { PluginActionInfo } from "./bindings/PluginActionInfo";
export type { PluginSetupStep } from "./bindings/PluginSetupStep";
export type { ProcessState } from "./bindings/ProcessState";
export type { PluginStateEvent } from "./bindings/PluginStateEvent";
export type { PluginProcessView } from "./bindings/PluginProcessView";
export type { InstallPhase } from "./bindings/InstallPhase";
export type { PluginInstallEvent } from "./bindings/PluginInstallEvent";
export type { PluginToolResult } from "./bindings/PluginToolResult";
export type { PluginEventPayload } from "./bindings/PluginEventPayload";
export type { CatalogView } from "./bindings/CatalogView";
export type { CatalogEntryView } from "./bindings/CatalogEntryView";
export type { CatalogViewInfo } from "./bindings/CatalogViewInfo";
export type { CatalogActionInfo } from "./bindings/CatalogActionInfo";
export type { CatalogUpdated } from "./bindings/CatalogUpdated";
export type { OpenViewRequest } from "./bindings/OpenViewRequest";
export type { RunReport } from "./bindings/RunReport";

// IA et chat du cœur
export type { AiEvent } from "./bindings/AiEvent";
export type { AiErrorKind } from "./bindings/AiErrorKind";
export type { ApprovalDecision } from "./bindings/ApprovalDecision";
export type { ApprovalKind } from "./bindings/ApprovalKind";
export type { FileChange } from "./bindings/FileChange";
export type { PlanItem } from "./bindings/PlanItem";
export type { PlanStatus } from "./bindings/PlanStatus";
export type { ToolSource } from "./bindings/ToolSource";
export type { ChatState } from "./bindings/ChatState";
export type { ChatMode } from "./bindings/ChatMode";
export type { ChatSessionRow } from "./bindings/ChatSessionRow";
export type { ChatEventRow } from "./bindings/ChatEventRow";
export type { ChatSessionDetail } from "./bindings/ChatSessionDetail";
export type { ChatSendInput } from "./bindings/ChatSendInput";
export type { ChatSendResult } from "./bindings/ChatSendResult";
export type { ChatEventPayload } from "./bindings/ChatEventPayload";
export type { ChatStatePayload } from "./bindings/ChatStatePayload";
export type { SettingsFieldError } from "./bindings/SettingsFieldError";
export type { PluginSettingsView } from "./bindings/PluginSettingsView";

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
export type { CloneState } from "./bindings/CloneState";
export type { SkillInfo } from "./bindings/SkillInfo";
export type { RepoCheckout } from "./bindings/RepoCheckout";
export type { RepoCloneView } from "./bindings/RepoCloneView";
export type { ReposChanged } from "./bindings/ReposChanged";
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
