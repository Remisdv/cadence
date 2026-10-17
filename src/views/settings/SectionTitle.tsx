import type { ReactNode } from "react";

/** Titre d'une section des réglages : h2 16px, puis son texte d'introduction (12.5px, tx2). */
export function SectionTitle({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <>
      <h2 className="font-display text-section font-semibold">{title}</h2>
      {children && <p className="mt-[7px] text-body-sm leading-[1.6] text-tx2">{children}</p>}
    </>
  );
}
