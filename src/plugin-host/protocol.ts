/**
 * Protocole `postMessage` entre le shell et l'iframe d'un plugin (plan § 3.3), version 1.
 * Le SDK (`public/mock-plugins/cadence-sdk.js`, futur `@cadence/plugin-sdk`) parle la même
 * enveloppe.
 */
import type { ErrorKind } from "@/lib/types";

export const PROTOCOL_VERSION = 1;

export type SdkErrorCode =
  | "invalid_params"
  | "method_not_found"
  | "tool_error"
  | "not_found"
  | "unavailable"
  | "timeout"
  | "cancelled"
  | "unauthenticated"
  | "internal";

export interface SdkError {
  code: SdkErrorCode;
  /** Lisible, en français. */
  message: string;
  data?: unknown;
}

export type Envelope =
  | { cadence: 1; kind: "req"; id: string; method: string; params?: unknown }
  | { cadence: 1; kind: "res"; id: string; result?: unknown; error?: SdkError }
  | { cadence: 1; kind: "evt"; event: string; data?: unknown };

export type Request = Extract<Envelope, { kind: "req" }>;

/** Message reçu d'une iframe : une requête bien formée, sinon `null`. */
export function asRequest(data: unknown): Request | null {
  if (typeof data !== "object" || data === null) return null;
  const m = data as Record<string, unknown>;
  return m.cadence === PROTOCOL_VERSION &&
    m.kind === "req" &&
    typeof m.id === "string" &&
    typeof m.method === "string"
    ? (m as unknown as Request)
    : null;
}

/** Réponse d'une poignée de main (`hello`). */
export interface HelloResult {
  api: 1;
  plugin: { id: string; version: string };
  route: { view: string; params: Record<string, string> };
  /** Variables CSS du shell (`--color-*`, `--text-*`, `--radius-*`, `--font-*`). */
  theme: Record<string, string>;
  locale: "fr-FR";
  /** Méthodes acceptées par cet hôte (`cadence.supports`). */
  methods: string[];
  host: { version: string };
}

/** Erreur typée levée par un handler de l'hôte. */
export class BridgeError extends Error {
  constructor(
    readonly code: SdkErrorCode,
    message: string,
    readonly data?: unknown,
  ) {
    super(message);
  }
}

/** Erreur d'une commande Rust → code du SDK. */
export function codeOfKind(kind: ErrorKind): SdkErrorCode {
  switch (kind) {
    case "validation":
      return "invalid_params";
    case "plugin-unavailable":
      return "unavailable";
    case "tool-error":
      return "tool_error";
    case "unauthenticated":
      return "unauthenticated";
    default:
      return "internal";
  }
}
