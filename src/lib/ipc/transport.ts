/**
 * Point d'entrée unique vers le back.
 *
 * - Dans Tauri : `invoke()` vers les commandes Rust.
 * - Dans un navigateur (`pnpm dev`) ou avec `VITE_MOCK_IPC=true` : handlers mockés,
 *   alimentés par les données des maquettes. Permet de développer toute l'UI sans back.
 */
import type { CommandArgs, CommandName, CommandResult } from "./commands";
import { CommandError, isIpcError } from "./errors";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const isMockIpc = !isTauri || import.meta.env.VITE_MOCK_IPC === "true";

type ArgsParam<C extends CommandName> = keyof CommandArgs<C> extends never
  ? [args?: CommandArgs<C>]
  : [args: CommandArgs<C>];

export async function call<C extends CommandName>(
  command: C,
  ...[args]: ArgsParam<C>
): Promise<CommandResult<C>> {
  try {
    if (isMockIpc) {
      const { handleMock } = await import("./mock/handlers");
      return await handleMock(command, (args ?? {}) as CommandArgs<C>);
    }
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<CommandResult<C>>(command, args as Record<string, unknown> | undefined);
  } catch (error) {
    if (isIpcError(error)) throw new CommandError(command, error);
    throw new CommandError(command, { kind: "internal", message: String(error) });
  }
}
