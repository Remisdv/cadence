import { Icon } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";

const sizes = {
  sm: { box: "size-[26px] rounded-lg", icon: 16, text: "text-[16px]" },
  md: { box: "size-8 rounded-field", icon: 19, text: "text-[19px]" },
};

/** Logo Cadence : pastille clay + mot-symbole Space Grotesk. */
export function Logo({
  size = "sm",
  className,
}: {
  size?: keyof typeof sizes;
  className?: string;
}) {
  const s = sizes[size];
  return (
    <div className={cn("flex items-center", size === "sm" ? "gap-[9px]" : "gap-2.5", className)}>
      <span className={cn("inline-flex items-center justify-center bg-clay text-clay-ink", s.box)}>
        <Icon name="sprint" size={s.icon} />
      </span>
      <span className={cn("font-display font-semibold tracking-[-0.02em]", s.text)}>Cadence</span>
    </div>
  );
}
