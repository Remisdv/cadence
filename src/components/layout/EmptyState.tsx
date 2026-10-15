import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Spinner } from "@/components/ui/Spinner";
import { CommandError } from "@/lib/ipc/errors";

export interface EmptyStateProps {
  icon?: IconName;
  title: ReactNode;
  children?: ReactNode;
  action?: ReactNode;
}

export function EmptyState({ icon = "layers", title, children, action }: EmptyStateProps) {
  return (
    <div className="flex h-full min-h-[240px] flex-col items-center justify-center gap-2 px-6 text-center">
      <span className="mb-1 flex text-tx3">
        <Icon name={icon} size={22} />
      </span>
      <div className="font-display text-title-sm font-semibold">{title}</div>
      {children && (
        <div className="max-w-[420px] text-body-sm leading-[1.6] text-tx3">{children}</div>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export function LoadingState({ label = "Chargement…" }: { label?: string }) {
  return (
    <div className="flex h-full min-h-[160px] items-center justify-center gap-2 text-body-sm text-tx3">
      <Spinner /> {label}
    </div>
  );
}

/** Erreur de commande affichée dans une vue (le `kind` choisit le texte). */
export function ErrorState({ error }: { error: unknown }) {
  const message = error instanceof Error ? error.message : String(error);
  const notImplemented = error instanceof CommandError && error.kind === "not-implemented";
  return (
    <EmptyState
      icon="warning"
      title={notImplemented ? "Pas encore branché" : "Impossible de charger"}
    >
      {message}
    </EmptyState>
  );
}
