import { useState } from "react";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { copyText } from "@/lib/platform";

export interface CommandBoxProps {
  command: string;
  className?: string;
}

/**
 * Commande exacte que Cadence lancera (maquettes V2-Onboarding-AI / Tools, V2-Tool-Install) :
 * console 32px, invite « > », bouton de copie. Rien ne s'exécute sans le clic qui l'accompagne.
 */
export function CommandBox({ command, className }: CommandBoxProps) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    setCopied(await copyText(command));
    window.setTimeout(() => setCopied(false), 1600);
  };
  return (
    <div
      className={cn(
        "flex h-8 items-center gap-2 rounded-control border border-line2 bg-console pr-1 pl-2.5 mono text-caption text-tx",
        className,
      )}
    >
      <span className="text-tx3">&gt;</span>
      <span className="min-w-0 grow truncate" title={command}>
        {command}
      </span>
      <button
        type="button"
        aria-label={copied ? "Commande copiée" : "Copier la commande"}
        onClick={copy}
        className="inline-flex size-6 shrink-0 items-center justify-center rounded-badge text-tx3 hover:bg-p3 hover:text-tx"
      >
        <Icon name={copied ? "check" : "copy"} size={13} />
      </button>
    </div>
  );
}
