import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { OnboardingSteps } from "@/components/domain/Steps";
import { onboardingSteps } from "@/lib/domain/onboarding";
import { Logo } from "./Logo";

export interface OnboardingFrameProps {
  /** Étape en cours dans la colonne de gauche (−1 : bienvenue, aucune étape commencée). */
  step: number;
  title: ReactNode;
  intro: ReactNode;
  /** Largeur maximale du contenu (900 à 940px selon les maquettes). */
  width?: 900 | 916 | 940;
  children: ReactNode;
}

const widths = { 900: "max-w-[900px]", 916: "max-w-[916px]", 940: "max-w-[940px]" } as const;

/**
 * Gabarit de l'onboarding de Cadence 2 (V2-Onboarding-*.dc.html) : colonne de 420px à gauche
 * (logo, promesse, cinq étapes), contenu à droite (titre 24px, introduction, écran).
 */
export function OnboardingFrame({
  step,
  title,
  intro,
  width = 900,
  children,
}: OnboardingFrameProps) {
  return (
    <div className="flex h-full min-w-0 grow bg-bg text-body">
      <aside className="flex w-[420px] shrink-0 flex-col overflow-y-auto border-r border-line px-10 py-11 bg-aside max-[1320px]:w-[340px] max-[1320px]:px-8 short:py-8">
        <Logo size="md" />
        <div className="mt-[38px] short:mt-7">
          <h2 className="text-display leading-[1.22]">
            Vos IA de code.
            <br />
            Vos plugins.
            <br />
            Une seule app.
          </h2>
          <p className="mt-3.5 text-body-lg leading-[1.6] text-tx2">
            Cadence pilote les IA déjà installées sur votre machine et leur donne les outils des
            plugins que vous choisissez : gestion de projet, cours, bases de données, et ce que la
            communauté publie.
          </p>
        </div>
        <div className="mt-10 short:mt-6">
          <OnboardingSteps steps={onboardingSteps.map((s) => s.label)} current={step} />
        </div>
        <div className="grow" />
        <div className="mt-6 text-caption leading-[1.6] text-tx3">
          Tout ce que vous choisissez ici reste modifiable ensuite dans les réglages.
        </div>
      </aside>

      <div className="flex min-w-0 grow flex-col overflow-y-auto px-[52px] py-11 max-[1320px]:px-10 short:py-8">
        <div className="max-w-[720px]">
          <h1 className="text-display-sm">{title}</h1>
          <p className="mt-[9px] text-body-lg leading-[1.6] text-tx2">{intro}</p>
        </div>
        <div className={cn("mt-[30px] short:mt-5", widths[width])}>{children}</div>
      </div>
    </div>
  );
}

/** Ligne de boutons d'une étape : action principale, secondaires, « Retour » à droite. */
export function OnboardingActions({
  children,
  onBack,
  className,
}: {
  children: ReactNode;
  onBack?: () => void;
  className?: string;
}) {
  return (
    <div className={cn("flex items-center gap-3.5", className)}>
      {children}
      {onBack && (
        <>
          <div className="grow" />
          <button type="button" onClick={onBack} className="text-body text-tx3 hover:text-tx">
            Retour
          </button>
        </>
      )}
    </div>
  );
}
