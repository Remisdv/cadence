import { useId } from "react";
import type { CtoQuestion } from "@/lib/types";
import { Icon } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";

export interface AgentQuestionProps {
  question: CtoQuestion;
  value: string;
  onChange: (value: string) => void;
  /** Réponse suggérée par l'agent, en placeholder. */
  suggestion?: string;
  /** `rail` : ligne séparée par un trait ; `boxed` : encadré p2 (écran agent CTO). */
  variant?: "rail" | "boxed";
}

export function AgentQuestion({
  question,
  value,
  onChange,
  suggestion,
  variant = "rail",
}: AgentQuestionProps) {
  const id = useId();
  const boxed = variant === "boxed";
  return (
    <div
      className={cn(
        boxed
          ? "rounded-field border border-line bg-p2 px-3 py-[11px]"
          : "border-b border-line py-3",
      )}
    >
      <label htmlFor={id} className="flex gap-2 text-body-sm leading-[1.55] text-tx">
        <span className="mt-0.5 flex text-blue">
          <Icon name="review" size={boxed ? 13 : 14} />
        </span>
        {question.question}
      </label>
      <input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={suggestion ?? "Votre réponse…"}
        className={cn(
          "w-full rounded-control border border-line2 px-2.5 text-body-sm text-tx outline-none placeholder:text-tx3 focus:border-clay",
          boxed ? "mt-2 h-[30px] bg-panel" : "mt-[9px] h-8 bg-p2",
        )}
      />
    </div>
  );
}
