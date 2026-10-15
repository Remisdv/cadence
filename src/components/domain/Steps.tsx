import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";

export type StepState = "done" | "current" | "pending";

const circle: Record<StepState, string> = {
  done: "bg-green text-ink",
  current: "bg-clay text-ink",
  pending: "bg-p3 text-tx3",
};

/* ---------- Étapes de l'onboarding (colonne de gauche) ---------- */

export interface OnboardingStepsProps {
  steps: string[];
  /** Index (0-based) de l'étape en cours. */
  current: number;
}

export function OnboardingSteps({ steps, current }: OnboardingStepsProps) {
  return (
    <ol className="flex flex-col gap-0.5">
      {steps.map((label, i) => {
        const state: StepState = i < current ? "done" : i === current ? "current" : "pending";
        return (
          <li
            key={label}
            className="flex h-11 items-center gap-3"
            aria-current={state === "current" ? "step" : undefined}
          >
            <span
              className={cn(
                "inline-flex size-[23px] items-center justify-center rounded-full text-caption font-semibold",
                circle[state],
              )}
            >
              {state === "done" ? <Icon name="check" size={13} /> : i + 1}
            </span>
            <span
              className={cn(
                "text-body-lg",
                state === "current"
                  ? "font-medium text-tx"
                  : state === "done"
                    ? "text-tx2"
                    : "text-tx3",
              )}
            >
              {label}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

/* ---------- Déroulé vertical d'une exécution d'agent ---------- */

export interface TimelineStep {
  title: ReactNode;
  detail?: ReactNode;
  state: StepState;
}

export function StepTimeline({ steps }: { steps: TimelineStep[] }) {
  return (
    <ol>
      {steps.map((step, i) => {
        const last = i === steps.length - 1;
        return (
          <li key={i} className="flex gap-[11px]">
            <div className="flex shrink-0 flex-col items-center">
              <span
                className={cn(
                  "inline-flex size-[22px] items-center justify-center rounded-full text-micro font-semibold",
                  circle[step.state],
                )}
              >
                {step.state === "done" ? <Icon name="check" size={12} /> : i + 1}
              </span>
              {!last && (
                <span
                  className={cn(
                    "min-h-4 w-[1.5px] grow",
                    step.state === "done" ? "bg-green-line" : "bg-line",
                  )}
                />
              )}
            </div>
            <div className={cn("min-w-0 grow", !last && "pb-3")}>
              <div
                className={cn(
                  "text-body-sm",
                  step.state === "current" ? "font-semibold text-tx" : "font-medium",
                  step.state === "pending" ? "text-tx3" : "text-tx",
                )}
              >
                {step.title}
              </div>
              {step.detail && (
                <div className="mt-[3px] text-caption leading-[1.55] text-tx3">{step.detail}</div>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

/* ---------- Étapes d'un pipeline (agent déploiement) ---------- */

export type StageState = "success" | "failure" | "running" | "pending";

const stageStyle: Record<StageState, { box: string; icon: string }> = {
  success: { box: "bg-p2 border-green-line", icon: "text-green" },
  failure: { box: "bg-p2 border-red-line", icon: "text-red" },
  running: { box: "bg-p3 border-amber-line", icon: "text-amber" },
  pending: { box: "bg-p2 border-line", icon: "text-tx3" },
};

export interface PipelineStage {
  title: ReactNode;
  detail?: ReactNode;
  state: StageState;
  icon?: Parameters<typeof Icon>[0]["name"];
}

export function PipelineStages({ stages }: { stages: PipelineStage[] }) {
  const defaultIcon = {
    success: "check",
    failure: "close",
    running: "clock",
    pending: "clock",
  } as const;
  return (
    <div className="flex items-stretch gap-2.5">
      {stages.map((stage, i) => {
        const s = stageStyle[stage.state];
        return (
          <div
            key={i}
            className={cn("min-w-0 flex-1 rounded-[10px] border px-3.5 py-[13px]", s.box)}
          >
            <div className="flex items-center gap-2">
              <span className={cn("flex", s.icon)}>
                <Icon name={stage.icon ?? defaultIcon[stage.state]} size={15} />
              </span>
              <span className="text-body-sm font-medium">{stage.title}</span>
            </div>
            {stage.detail && (
              <div className="mt-1.5 text-tiny leading-normal text-tx3">{stage.detail}</div>
            )}
          </div>
        );
      })}
    </div>
  );
}
