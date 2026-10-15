/**
 * Structures de panneaux récurrentes : carte à barre de titre, liste de propriétés,
 * ligne de réglage, encadrés de note.
 */
import type { ReactNode } from "react";
import { Card } from "@/components/ui/Card";
import { cn } from "@/lib/cn";

/* ---------- Carte à barre de titre (Portée, Journal d'exécution, Tests…) ---------- */

export interface TitledCardProps {
  title: ReactNode;
  subtitle?: ReactNode;
  /** Avant le titre (carré d'agent). */
  leading?: ReactNode;
  /** À droite (badges, petit bouton). */
  actions?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
}

export function TitledCard({
  title,
  subtitle,
  leading,
  actions,
  footer,
  children,
  className,
  bodyClassName,
}: TitledCardProps) {
  return (
    <Card className={cn("flex min-h-0 flex-col overflow-hidden", className)}>
      <div className="flex h-10 shrink-0 items-center gap-2.5 border-b border-line px-3.5">
        {leading}
        <h3 className="text-body">{title}</h3>
        {subtitle && <span className="text-caption text-tx3">{subtitle}</span>}
        <div className="grow" />
        {actions}
      </div>
      <div className={cn("min-h-0 grow overflow-y-auto", bodyClassName)}>{children}</div>
      {footer}
    </Card>
  );
}

/* ---------- Propriétés d'une issue (rail de droite) ---------- */

export function PropertyList({ children }: { children: ReactNode }) {
  return <dl className="flex flex-col">{children}</dl>;
}

export function PropertyRow({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 border-b border-line py-[9px]">
      <dt className="w-[86px] shrink-0 text-small text-tx3">{label}</dt>
      <dd className="flex min-w-0 grow flex-wrap items-center gap-1.5">{children}</dd>
    </div>
  );
}

/** Valeur vide d'une propriété : « aucune ». */
export function EmptyValue({ children = "aucune" }: { children?: ReactNode }) {
  return <span className="mono text-small text-tx3">{children}</span>;
}

/* ---------- Ligne de réglage ---------- */

export interface SettingRowProps {
  title: ReactNode;
  description?: ReactNode;
  /** Un seul contrôle : select, switch, valeur mono, badge « verrouillé ». */
  control: ReactNode;
}

export function SettingRow({ title, description, control }: SettingRowProps) {
  return (
    <div className="flex items-center gap-3.5 border-b border-line py-[13px]">
      <div className="min-w-0 grow">
        <div className="text-body-sm">{title}</div>
        {description && (
          <div className="mt-[3px] text-caption leading-normal text-tx3">{description}</div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">{control}</div>
    </div>
  );
}

/* ---------- Encadrés de note ---------- */

const noteTones = {
  neutral: "rounded-lg border-line bg-p2 px-[11px] py-[9px]",
  purple: "rounded-field border-purple-line bg-purple-d px-3 py-[11px] leading-[1.6]",
  green: "rounded-[10px] border-green-line bg-p2 px-4 py-3.5",
};

/** Bandeau d'information compact (neutre), note teintée (violet) ou bloc ajouté par un agent (vert). */
export function Note({
  tone = "neutral",
  children,
  className,
}: {
  tone?: keyof typeof noteTones;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("border text-caption text-tx2", noteTones[tone], className)}>{children}</div>
  );
}

/* ---------- Pied d'action (barre de validation) ---------- */

export interface FooterBarProps {
  /** Explication à gauche. */
  hint?: ReactNode;
  /** Boutons à droite, du moins au plus engageant (ghost → secondary → primary). */
  children?: ReactNode;
  /** 44 (tableau), 50 ou 52 (écrans d'agent). */
  height?: 42 | 44 | 50 | 52;
  className?: string;
}

export function FooterBar({ hint, children, height = 52, className }: FooterBarProps) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center gap-[9px] border-t border-line bg-p2",
        height <= 44 ? "px-3.5" : "px-5",
        className,
      )}
      style={{ height }}
    >
      {hint && <span className="text-caption text-tx3">{hint}</span>}
      <div className="grow" />
      {children}
    </div>
  );
}
