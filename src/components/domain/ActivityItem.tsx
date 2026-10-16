import type { ReactNode } from "react";

export interface ActivityItemProps {
  /** Pastille de tête (avatar, icône de plugin…), 21–22px. */
  badge: ReactNode;
  children: ReactNode;
  time: ReactNode;
}

/** Ligne d'un fil d'activité : pastille, texte tx2 12.5px, horodatage. */
export function ActivityItem({ badge, children, time }: ActivityItemProps) {
  return (
    <div className="flex gap-2.5 border-b border-line py-2.5 last:border-b-0">
      {badge}
      <div className="min-w-0 grow">
        <div className="text-body-sm leading-normal text-tx2">{children}</div>
        <div className="mt-[3px] text-tiny text-tx3">{time}</div>
      </div>
    </div>
  );
}

/** Mise en avant dans un texte d'activité : identifiant mono ou fragment tx. */
export function Em({ mono, children }: { mono?: boolean; children: ReactNode }) {
  return <span className={mono ? "mono text-tx" : "text-tx"}>{children}</span>;
}
