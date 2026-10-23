/**
 * Barre contextuelle de Cadence 2 (220px, V2-Chat.dc.html, V2-Project-Issue.dc.html,
 * V2-Settings.dc.html) : en-tête 54px (tuile + titre), groupes d'entrées, pied.
 * Le contenu dépend de l'écran : conversations (chat), vues du plugin, sections des réglages.
 */
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";

export function ContextBar({
  label,
  header,
  footer,
  children,
}: {
  /** Nom accessible de la colonne. */
  label: string;
  header: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <aside
      aria-label={label}
      className="flex w-[220px] shrink-0 flex-col border-r border-line bg-panel"
    >
      {header}
      <div className="flex min-h-0 grow flex-col gap-4 overflow-y-auto px-2.5 py-3">{children}</div>
      {footer && <div className="flex shrink-0 flex-col gap-1.5 px-2.5 pb-3">{footer}</div>}
    </aside>
  );
}

/** En-tête 54px : tuile 22px, titre Space Grotesk 14.5px, action à droite. */
export function ContextBarHeader({
  tile,
  title,
  action,
}: {
  tile: ReactNode;
  title: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex h-[54px] shrink-0 items-center gap-[9px] border-b border-line pr-2.5 pl-3.5">
      {tile}
      <span className="min-w-0 grow truncate font-display text-title-md font-semibold tracking-[-0.01em]">
        {title}
      </span>
      {action}
    </div>
  );
}

/** Groupe d'entrées sous un sur-titre (« Pilotage », « Conversations »). */
export function ContextBarSection({
  title,
  action,
  children,
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      {title && (
        <div className="mb-[3px] flex h-5 items-center gap-1.5 pr-1 pl-[9px]">
          <span className="grow text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">
            {title}
          </span>
          {action}
        </div>
      )}
      {children}
    </div>
  );
}

export interface ContextBarItemProps {
  icon: IconName;
  label: string;
  count?: number;
  /** Compteur clay : quelque chose attend l'utilisateur. */
  countTone?: "tx3" | "clay";
  active?: boolean;
  onClick?: () => void;
}

/** Entrée 30px : icône 15px (clay si active), libellé 13px, compteur mono. */
export function ContextBarItem({
  icon,
  label,
  count,
  countTone = "tx3",
  active,
  onClick,
}: ContextBarItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-[30px] w-full items-center gap-[9px] rounded-control px-[9px] text-left text-body transition-colors",
        active ? "bg-p3 font-medium text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      <span className={cn("flex", active ? "text-clay" : "text-tx3")}>
        <Icon name={icon} size={15} />
      </span>
      <span className="min-w-0 grow truncate">{label}</span>
      {count !== undefined && (
        <span className={cn("mono text-tiny", countTone === "clay" ? "text-clay" : "text-tx3")}>
          {count}
        </span>
      )}
    </button>
  );
}

/** Sous-entrées 27px en retrait, sous une entrée (plugins des réglages, dépôts d'un service). */
export function ContextBarSubItems({ children }: { children: ReactNode }) {
  return (
    <div className="mt-px mb-1 ml-[17px] flex flex-col gap-px border-l border-line pl-[7px]">
      {children}
    </div>
  );
}

export function ContextBarSubItem({
  leading,
  label,
  active,
  onClick,
}: {
  leading: ReactNode;
  label: string;
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-[27px] w-full items-center gap-2 rounded-control px-2 text-left text-small",
        active ? "bg-p3 text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      {leading}
      <span className="truncate">{label}</span>
    </button>
  );
}

/** Conversation : titre 12.5px + heure, puis puce de contexte (plugin, dépôt, chapitre). */
export function ConversationItem({
  title,
  time,
  context,
  active,
  onClick,
}: {
  title: string;
  time: string;
  context: ReactNode;
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex w-full flex-col gap-[3px] rounded-control px-[9px] pt-1.5 pb-[7px] text-left",
        active ? "bg-p3" : "hover:bg-p2",
      )}
    >
      <span className="flex items-center gap-2">
        <span
          className={cn(
            "min-w-0 grow truncate text-body-sm",
            active ? "font-medium text-tx" : "text-tx2",
          )}
        >
          {title}
        </span>
        <span className="text-micro whitespace-nowrap text-tx3">{time}</span>
      </span>
      <span className="flex min-w-0 items-center gap-1.5 text-tiny text-tx3">{context}</span>
    </button>
  );
}

/** Encadré d'aide du pied de barre (11px, tx3). */
export function ContextBarHint({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-field border border-line px-2.5 py-[9px] text-tiny leading-[1.55] text-tx3">
      {children}
    </div>
  );
}

/** Bouton carré discret de l'en-tête ou d'un sur-titre (« Nouvelle conversation », options). */
export function ContextBarIconButton({
  icon,
  label,
  size = 26,
  bordered,
  ...rest
}: {
  icon: IconName;
  label: string;
  size?: 20 | 26 | 28;
  bordered?: boolean;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children">) {
  const box =
    size === 20
      ? "size-5 rounded-badge"
      : size === 26
        ? "size-[26px] rounded-control"
        : "size-7 rounded-control";
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex shrink-0 items-center justify-center text-tx3 hover:bg-p2 hover:text-tx disabled:opacity-50 disabled:hover:bg-transparent",
        box,
        bordered && "border border-line",
      )}
      {...rest}
    >
      <Icon name={icon} size={size === 20 ? 13 : 15} />
    </button>
  );
}
