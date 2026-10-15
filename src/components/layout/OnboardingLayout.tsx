import type { ReactNode } from "react";
import { OnboardingSteps } from "@/components/domain/Steps";
import { Logo } from "./Logo";

export const onboardingSteps = ["Compte GitHub", "Agents de code", "Projet et mapping"];

export interface OnboardingLayoutProps {
  /** Étape en cours (0-based). */
  step: number;
  title: ReactNode;
  intro: ReactNode;
  children: ReactNode;
}

/** Gabarit des trois écrans de connexion : colonne de 420px à gauche, contenu à droite. */
export function OnboardingLayout({ step, title, intro, children }: OnboardingLayoutProps) {
  return (
    <div className="flex h-full min-w-0 grow bg-bg">
      <aside className="flex w-[420px] shrink-0 flex-col border-r border-line px-10 py-11 bg-aside">
        <Logo size="md" />
        <div className="mt-[38px]">
          <h2 className="text-display leading-[1.22]">
            GitHub est votre base de données.
            <br />
            Cadence est votre cockpit.
          </h2>
          <p className="mt-3.5 text-body-lg leading-[1.6] text-tx2">
            Un sprint est une version. Une version est un milestone. Tout ce que vous faites ici
            reste lisible par n'importe qui sur github.com — sans base de données parallèle.
          </p>
        </div>
        <div className="mt-10">
          <OnboardingSteps steps={onboardingSteps} current={step} />
        </div>
        <div className="grow" />
        <div className="text-caption leading-[1.6] text-tx3">
          Les jetons ne transitent jamais par un serveur tiers. Ils sont écrits dans le trousseau du
          système.
        </div>
      </aside>

      <div className="flex min-w-0 grow flex-col overflow-y-auto px-[52px] py-11">
        <div className="max-w-[720px]">
          <h1 className="text-display-sm">{title}</h1>
          <p className="mt-[9px] text-body-lg leading-[1.6] text-tx2">{intro}</p>
        </div>
        <div className="mt-[30px] min-h-0 grow">{children}</div>
      </div>
    </div>
  );
}
