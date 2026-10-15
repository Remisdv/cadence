import type { ErrorKind, IpcError } from "@/lib/types";

/** Erreur levée par `call()` : porte le `kind` renvoyé par Rust. */
export class CommandError extends Error implements IpcError {
  readonly kind: ErrorKind;
  readonly command: string;

  constructor(command: string, error: IpcError) {
    super(error.message);
    this.name = "CommandError";
    this.kind = error.kind;
    this.command = command;
  }
}

export function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as IpcError).message === "string"
  );
}

/** Erreurs qu'il est inutile de rejouer automatiquement. */
export function isPermanent(error: unknown): boolean {
  return (
    error instanceof CommandError &&
    ["validation", "not-implemented", "no-project", "unauthenticated", "config"].includes(
      error.kind,
    )
  );
}
