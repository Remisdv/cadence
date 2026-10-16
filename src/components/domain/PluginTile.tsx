import { Icon } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";
import { kitIcon, pluginTileClass } from "@/lib/domain/plugins";
import type { PluginColor } from "@/lib/types";

/**
 * Tuile colorée d'un plugin (maquettes Cadence 2) : 26px dans le rail, 22px en tête de sa
 * barre, 18px dans les listes, 14px en sous-entrée, 13px dans une puce de contexte.
 */
const sizes = {
  13: { box: "size-[13px] rounded-[4px]", icon: 8 },
  14: { box: "size-[14px] rounded-[4px]", icon: 9 },
  18: { box: "size-[18px] rounded-badge", icon: 11 },
  22: { box: "size-[22px] rounded-md", icon: 14 },
  26: { box: "size-[26px] rounded-control", icon: 16 },
} as const;

export interface PluginTileProps {
  color: PluginColor;
  /** Nom d'icône du kit (venu du manifeste). */
  icon: string;
  size?: keyof typeof sizes;
  className?: string;
}

export function PluginTile({ color, icon, size = 26, className }: PluginTileProps) {
  const s = sizes[size];
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-flex shrink-0 items-center justify-center text-ink",
        s.box,
        pluginTileClass[color],
        className,
      )}
    >
      <Icon name={kitIcon(icon)} size={s.icon} />
    </span>
  );
}

/** Tuile neutre (chat global, réglages) : fond p4, icône tx. */
export function NeutralTile({
  icon,
  size = 22,
}: {
  icon: Parameters<typeof Icon>[0]["name"];
  size?: 13 | 22;
}) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "inline-flex shrink-0 items-center justify-center bg-p4",
        size === 22 ? "size-[22px] rounded-md text-tx" : "size-[13px] rounded-[4px] text-tx2",
      )}
    >
      <Icon name={icon} size={size === 22 ? 14 : 9} />
    </span>
  );
}
