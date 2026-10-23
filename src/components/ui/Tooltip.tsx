/**
 * Infobulle (Radix) au style du rail de Cadence 2 (V2-Chat.dc.html) : bulle p4 bordée line2,
 * flèche à gauche, titre 12px et précision 11px. Rendue dans un portail : jamais coupée par
 * une zone qui défile.
 */
import type { ReactNode } from "react";
import { Tooltip as TooltipPrimitive } from "radix-ui";

export const TooltipProvider = TooltipPrimitive.Provider;

export interface TooltipProps {
  title: ReactNode;
  detail?: ReactNode;
  side?: "right" | "bottom";
  children: ReactNode;
}

export function Tooltip({ title, detail, side = "right", children }: TooltipProps) {
  return (
    <TooltipPrimitive.Root delayDuration={250}>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={8}
          collisionPadding={8}
          className="z-50 rounded-lg border border-line2 bg-p4 px-[11px] py-[7px] whitespace-nowrap shadow-modal"
        >
          <span className="block text-small font-semibold text-tx">{title}</span>
          {detail && <span className="mt-0.5 block text-tiny text-tx3">{detail}</span>}
          <TooltipPrimitive.Arrow width={10} height={5} className="fill-p4" />
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
