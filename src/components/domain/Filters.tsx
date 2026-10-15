import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, Ref } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";

/** Champ de filtre 30px avec loupe (barres de filtres Issues / PR). */
export function FilterInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label
      className={cn(
        "flex h-[30px] max-w-[280px] grow items-center gap-2 rounded-control border border-line bg-p2 px-2.5 text-tx3 focus-within:border-line2",
        className,
      )}
    >
      <Icon name="search" size={14} />
      <input
        className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
        {...rest}
      />
    </label>
  );
}

export interface FilterChipProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "value"> {
  icon: IconName;
  label: ReactNode;
  /** Valeur active : la puce passe en clay. */
  value?: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

/**
 * « Version : 0.1.2 et 0.1.3 ⌄ » — neutre au repos, clay quand un filtre est posé.
 * Les props restantes sont transmises au bouton : utilisable comme déclencheur de menu (Radix).
 */
export function FilterChip({ icon, label, value, className, ...rest }: FilterChipProps) {
  const active = value !== undefined && value !== null && value !== "";
  return (
    <button
      type="button"
      {...rest}
      className={cn(
        "inline-flex h-[30px] items-center gap-1.5 rounded-control border px-2.5 text-body-sm data-[state=open]:border-line2",
        active
          ? "border-clay-line bg-clay-d text-clay"
          : "border-line bg-p2 text-tx2 hover:text-tx",
        className,
      )}
    >
      <Icon name={icon} size={14} />
      {label}
      {active && <> : {value}</>}
      <Icon name="chevronDown" size={13} />
    </button>
  );
}

export interface TabItem<T extends string> {
  value: T;
  label: ReactNode;
}

export interface PillTabsProps<T extends string> {
  items: TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  /** `mono` : onglets de session du terminal (#142, #147). */
  variant?: "default" | "mono";
  "aria-label"?: string;
}

/** Onglets en pastilles, sans bordure : actif sur fond p3. */
export function PillTabs<T extends string>({
  items,
  value,
  onChange,
  variant = "default",
  ...rest
}: PillTabsProps<T>) {
  const mono = variant === "mono";
  return (
    <div
      role="tablist"
      className={cn("flex", mono ? "gap-1" : "gap-1.5")}
      aria-label={rest["aria-label"]}
    >
      {items.map((item) => {
        const selected = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => onChange(item.value)}
            className={cn(
              mono
                ? "h-6 rounded-md px-[9px] mono text-tiny"
                : "h-8 rounded-control px-[13px] text-body-sm",
              selected
                ? cn("bg-p3 text-tx", !mono && "font-medium")
                : cn("bg-transparent hover:text-tx", mono ? "text-tx3" : "text-tx2"),
            )}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
