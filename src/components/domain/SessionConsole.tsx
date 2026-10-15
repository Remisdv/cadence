import type { ReactNode } from "react";
import type { AgentEventKind } from "@/lib/types";
import { cn } from "@/lib/cn";

/** Couleur de chaque nature de ligne dans le terminal d'agent. */
export const consoleLineClass: Record<AgentEventKind, string> = {
  system: "text-tx3",
  read: "text-blue",
  write: "text-clay",
  message: "text-tx2",
  success: "text-green",
  question: "text-amber",
  error: "text-red",
};

export interface ConsoleLine {
  kind: AgentEventKind;
  text: string;
}

export interface SessionConsoleProps {
  lines: ConsoleLine[];
  /** En-tête (titre + minuteur, ou onglets de session). */
  header?: ReactNode;
  /** Pied (méta du modèle, ou champ d'intervention). */
  footer?: ReactNode;
  className?: string;
}

/** Terminal de session Claude Code : fond console, mono 11px, une couleur par nature de ligne. */
export function SessionConsole({ lines, header, footer, className }: SessionConsoleProps) {
  return (
    <div className={cn("flex min-h-0 flex-col overflow-hidden bg-console", className)}>
      {header && (
        <div className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line px-[13px]">
          {header}
        </div>
      )}
      <div
        className="min-h-0 grow overflow-y-auto px-[13px] py-[11px] mono text-tiny leading-[1.85]"
        role="log"
        aria-live="polite"
      >
        {lines.map((line, i) => (
          <div key={i} className={cn("whitespace-pre-wrap", consoleLineClass[line.kind])}>
            {line.text}
          </div>
        ))}
      </div>
      {footer && (
        <div className="flex h-9 shrink-0 items-center gap-2 border-t border-line px-[13px]">
          {footer}
        </div>
      )}
    </div>
  );
}
