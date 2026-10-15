import type { ReactNode } from "react";
import { OnboardingSteps } from "@/components/domain/Steps";
import { Logo } from "./Logo";

/** Le dépôt se choisit ensuite dans la barre latérale, à tout moment. */
export const onboardingSteps = ["Compte GitHub", "Agents de code"];

export interface OnboardingLayoutProps {
  /** Étape en cours (0-based). */
  step: number;
  title: ReactNode;
  intro: ReactNode;
  children: ReactNode;
  /**
   * Boutons d'étape (Continuer / Retour). Ils suivent le contenu, et restent collés en bas de
   * l'écran quand celui-ci est trop court pour tout afficher : « Continuer » est toujours visible.
   */
  actions?: ReactNode;
}

/** Gabarit des trois écrans de connexion : colonne de 420px à gauche, contenu à droite. */
export function OnboardingLayout({ step, title, intro, children, actions }: OnboardingLayoutProps) {
  return (
    <div className="flex h-full min-w-0 grow bg-bg">
      <aside className="flex w-[420px] shrink-0 flex-col overflow-y-auto border-r border-line px-10 py-11 bg-aside short:py-8">
        <Logo size="md" />
        <div className="mt-[38px] short:mt-7">
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
        <div className="mt-10 short:mt-6">
          <OnboardingSteps steps={onboardingSteps} current={step} />
        </div>
        <div className="grow" />
        <div className="mt-6 text-caption leading-[1.6] text-tx3">
          Les jetons ne transitent jamais par un serveur tiers. Ils sont écrits dans le trousseau du
          système.
        </div>
      </aside>

      <div className="flex min-w-0 grow flex-col overflow-y-auto px-[52px] pt-11 short:pt-8">
        <div className="max-w-[720px]">
          <h1 className="text-display-sm">{title}</h1>
          <p className="mt-[9px] text-body-lg leading-[1.6] text-tx2">{intro}</p>
        </div>
        <div className="mt-[30px] short:mt-5">{children}</div>
        {actions && (
          <>
            {/* Espace vide sous le contenu, de la hauteur du fondu : tant que rien ne défile,
                le fondu ne recouvre que cet espace, jamais le bas d'une carte. */}
            <div aria-hidden className="h-[22px] shrink-0 short:h-4" />
            <div className="sticky bottom-0 z-10 bg-bg pb-11 short:pb-6">
              {/* Fondu au-dessus des boutons quand le contenu défile dessous. */}
              <div
                aria-hidden
                className="pointer-events-none absolute inset-x-0 -top-[22px] h-[22px] bg-gradient-to-t from-bg to-transparent short:-top-4 short:h-4"
              />
              <div className="flex items-center gap-3.5">{actions}</div>
            </div>
          </>
        )}
        {!actions && <div className="shrink-0 pb-11 short:pb-8" />}
      </div>
    </div>
  );
}
