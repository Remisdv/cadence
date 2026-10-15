import type { ReactNode } from "react";
import { Icon } from "@/components/icons/Icon";
import { Kbd } from "@/components/ui/Kbd";
import { cn } from "@/lib/cn";
import { modKey } from "@/lib/hooks/usePlatform";
import { useUiStore } from "@/stores/ui";

export interface TopBarProps {
  title: ReactNode;
  subtitle?: ReactNode;
  /** Boutons à droite de la recherche (secondaire puis primaire). */
  actions?: ReactNode;
  /** Bouton de recherche ⌘K (affiché par défaut). */
  search?: boolean;
}

/** En-tête de page 54px : titre 15px + sous-titre, recherche, actions. */
export function TopBar({ title, subtitle, actions, search = true }: TopBarProps) {
  const openPalette = useUiStore((s) => s.setPaletteOpen);
  return (
    <header className="flex h-[54px] shrink-0 items-center gap-3.5 border-b border-line bg-panel px-5">
      <div className="min-w-0">
        <h1 className="truncate text-title">{title}</h1>
        {subtitle && <div className="mt-px truncate text-caption text-tx3">{subtitle}</div>}
      </div>
      <div className="grow" />
      <div className="flex items-center gap-2">
        {search && (
          <button
            type="button"
            onClick={() => openPalette(true)}
            className="flex h-8 w-[220px] items-center gap-2 rounded-lg border border-line bg-p2 px-2.5 text-body-sm text-tx3 hover:border-line2"
          >
            <Icon name="search" size={15} />
            <span className="grow text-left">Rechercher…</span>
            <Kbd>{modKey}K</Kbd>
          </button>
        )}
        {actions}
      </div>
    </header>
  );
}

export interface PageProps extends TopBarProps {
  children: ReactNode;
  /**
   * `default` : 18px 20px, défilement vertical (la plupart des écrans).
   * `none` : volets pleine hauteur (écrans d'agent, détail d'issue, réglages).
   */
  padding?: "default" | "none";
  className?: string;
}

/** Écran de l'app : en-tête + zone principale. */
export function Page({ children, padding = "default", className, ...header }: PageProps) {
  return (
    <div className="flex min-w-0 grow flex-col">
      <TopBar {...header} />
      <main
        className={cn(
          "min-h-0 grow",
          padding === "default" ? "overflow-y-auto px-5 py-[18px]" : "flex overflow-hidden",
          className,
        )}
      >
        {children}
      </main>
    </div>
  );
}

export interface RailProps {
  width: number;
  side?: "left" | "right";
  /** `panel` : volet de navigation ; `console` : terminal d'agent ; `none` : transparent. */
  tone?: "panel" | "console" | "none";
  children: ReactNode;
  className?: string;
}

/** Volet latéral à largeur fixe (file CTO 300, propriétés 352, console 286/392…). */
export function Rail({ width, side = "left", tone = "panel", children, className }: RailProps) {
  return (
    <aside
      className={cn(
        "flex min-h-0 shrink-0 flex-col overflow-y-auto",
        side === "left" ? "border-r border-line" : "border-l border-line",
        tone === "panel" && "bg-panel",
        tone === "console" && "bg-console",
        className,
      )}
      style={{ width }}
    >
      {children}
    </aside>
  );
}
