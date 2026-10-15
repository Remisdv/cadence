import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { ProgressBar, type ProgressTone } from "@/components/ui/ProgressBar";

const valueTones = {
  tx: "text-tx",
  clay: "text-clay",
  green: "text-green",
  amber: "text-amber",
  blue: "text-blue",
  purple: "text-purple",
} as const;

type ValueTone = keyof typeof valueTones;

export interface StatCardProps {
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
  tone?: ValueTone;
}

/** Carte KPI : libellé 11.5px, valeur mono 24px colorée, précision 11.5px. */
export function StatCard({ label, value, hint, tone = "tx" }: StatCardProps) {
  return (
    <div className="min-w-0 flex-1 rounded-[11px] border border-line bg-panel px-[15px] py-[13px]">
      <div className="text-caption text-tx3">{label}</div>
      <div className="mt-[5px] flex items-baseline gap-[7px]">
        <span className={cn("mono text-kpi font-medium tracking-[-0.02em]", valueTones[tone])}>
          {value}
        </span>
        {hint && <span className="text-caption text-tx3">{hint}</span>}
      </div>
    </div>
  );
}

export interface StatStripItem {
  label: ReactNode;
  value: ReactNode;
  suffix?: ReactNode;
  tone?: ValueTone;
}

/** Bandeau de KPI compact (détail de version) : une carte, cellules séparées par un trait. */
export function StatStrip({ items }: { items: StatStripItem[] }) {
  return (
    <div className="flex overflow-hidden rounded-card border border-line bg-panel">
      {items.map((item, i) => (
        <div
          key={i}
          className={cn(
            "min-w-0 flex-1 px-3.5 py-3",
            i < items.length - 1 && "border-r border-line",
          )}
        >
          <div className="text-tiny text-tx3">{item.label}</div>
          <div className="mt-1 flex items-baseline gap-1.5">
            <span className={cn("mono text-[20px] font-medium", valueTones[item.tone ?? "tx"])}>
              {item.value}
            </span>
            {item.suffix && <span className="text-tiny text-tx3">{item.suffix}</span>}
          </div>
        </div>
      ))}
    </div>
  );
}

export interface MeterItem {
  label: ReactNode;
  value: ReactNode;
  /** 0 → 100 */
  percent: number;
  tone: ProgressTone;
}

/** Jauges libellées (progression d'une version, consommation des agents). */
export function MeterList({ items, className }: { items: MeterItem[]; className?: string }) {
  return (
    <div className={cn("flex flex-col gap-[11px]", className)}>
      {items.map((item, i) => (
        <div key={i}>
          <div className="mb-[5px] flex justify-between text-small">
            <span className="text-tx2">{item.label}</span>
            <span className="mono text-tx3">{item.value}</span>
          </div>
          <ProgressBar value={item.percent} tone={item.tone} height={5} />
        </div>
      ))}
    </div>
  );
}
