/** Bienvenue (V2-Onboarding-Welcome.dc.html) : IA de code → Cadence → plugins. */
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Button } from "@/components/ui/Button";
import { OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { Logo } from "@/components/layout/Logo";
import type { StepProps } from "../OnboardingView";

const ais: { name: string; icon: IconName; tile: string }[] = [
  { name: "Claude Code", icon: "sparkle", tile: "bg-clay" },
  { name: "Codex CLI", icon: "terminal", tile: "bg-tx" },
  { name: "OpenCode", icon: "code", tile: "bg-purple" },
];

const features: { icon: IconName; label: string }[] = [
  { icon: "chat", label: "Chat global et contextuel" },
  { icon: "wrench", label: "Passerelle MCP des plugins" },
  { icon: "sparkle", label: "Skills déclenchés par bouton" },
  { icon: "github", label: "Services partagés : dépôts GitHub" },
];

const promises: { icon: IconName; title: string; text: string }[] = [
  {
    icon: "key",
    title: "Vos abonnements, pas de clé",
    text: "Cadence lance la CLI déjà connectée sur votre machine. Aucune clé API à saisir, aucun compte Cadence.",
  },
  {
    icon: "shield",
    title: "Des plugins relus",
    text: "Le catalogue est un dépôt public. Chaque plugin y entre par une pull request relue, puis s'exécute en confiance totale.",
  },
  {
    icon: "grid",
    title: "Des plugins rangés par catégorie",
    text: "Projet, Études, Code, Données, Outils : chaque plugin activé prend sa place dans le rail, sans rien à créer au préalable.",
  },
];

export function WelcomeStep({ onNext }: StepProps) {
  return (
    <OnboardingFrame
      step={-1}
      title="Bienvenue dans Cadence"
      intro="Cadence est un hôte : il relie l'IA de code que vous utilisez déjà à des plugins qui apportent les fonctionnalités. Sans plugin, vous avez le chat ; avec eux, des vues, des boutons d'action et des services partagés."
    >
      <div className="flex flex-col gap-7">
        <div className="flex items-center">
          <Column title="IA de code" width="basis-[250px]">
            {ais.map((ai) => (
              <Item key={ai.name} icon={ai.icon} tile={ai.tile} title={ai.name}>
                CLI locale · votre abonnement
              </Item>
            ))}
          </Column>
          <Arrow />
          <div className="w-[236px] shrink-0 rounded-modal border border-clay-line bg-p2 px-[18px] pt-[18px] pb-4 shadow-[0_0_0_4px] shadow-clay/6">
            <Logo size="card" />
            <div className="mt-3.5 flex flex-col gap-2">
              {features.map((f) => (
                <div key={f.label} className="flex items-center gap-2 text-body-sm text-tx2">
                  <span className="flex text-clay">
                    <Icon name={f.icon} size={14} />
                  </span>
                  {f.label}
                </div>
              ))}
            </div>
          </div>
          <Arrow />
          <Column title="Plugins" width="basis-[270px]">
            <Item icon="board" tile="bg-blue" title="Gestion de projet">
              Issues, sprints, versions, PR
            </Item>
            <Item icon="book" tile="bg-green" title="Documents">
              Cours, citations, QCM
            </Item>
            <div className="flex items-center gap-2.5 rounded-tile border border-dashed border-line2 px-3 py-2.5">
              <span className="inline-flex size-[30px] shrink-0 items-center justify-center rounded-control bg-p3 text-tx3">
                <Icon name="plus" size={15} />
              </span>
              <div className="min-w-0">
                <div className="truncate text-body font-medium text-tx2">
                  Plugins de la communauté
                </div>
                <div className="mt-px truncate text-caption text-tx3">
                  Catalogue public, relu avant publication
                </div>
              </div>
            </div>
          </Column>
        </div>

        <div className="grid grid-cols-3 gap-3.5">
          {promises.map((p) => (
            <div
              key={p.title}
              className="flex flex-col gap-2 rounded-card border border-line bg-panel p-4"
            >
              <span className="flex text-clay">
                <Icon name={p.icon} size={18} />
              </span>
              <h3 className="text-title-sm">{p.title}</h3>
              <p className="text-body-sm leading-[1.6] text-tx2">{p.text}</p>
            </div>
          ))}
        </div>

        <div className="flex items-center gap-3.5">
          <Button variant="primary" size="lg" iconRight="arrowRight" autoFocus onClick={onNext}>
            Commencer
          </Button>
          <span className="text-body-sm text-tx3">Environ 4 minutes · 5 étapes</span>
        </div>
      </div>
    </OnboardingFrame>
  );
}

function Column({ title, width, children }: { title: string; width: string; children: ReactNode }) {
  return (
    <div className={cn("flex min-w-0 shrink grow-0 flex-col gap-2", width)}>
      <div className="px-0.5 pb-0.5 text-micro font-semibold tracking-[0.09em] text-tx3 uppercase">
        {title}
      </div>
      {children}
    </div>
  );
}

function Item({
  icon,
  tile,
  title,
  children,
}: {
  icon: IconName;
  tile: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center gap-2.5 rounded-tile border border-line bg-panel px-3 py-2.5">
      <span
        aria-hidden
        className={cn(
          "inline-flex size-[30px] shrink-0 items-center justify-center rounded-field text-ink",
          tile,
        )}
      >
        <Icon name={icon} size={16} />
      </span>
      <div className="min-w-0">
        <div className="truncate text-body font-medium">{title}</div>
        <div className="mt-px truncate text-caption text-tx3">{children}</div>
      </div>
    </div>
  );
}

function Arrow() {
  return (
    <div className="flex w-14 min-w-5 shrink items-center">
      <div className="grow border-t border-dashed border-line2" />
      <span className="-ml-1 flex text-line2">
        <Icon name="chevronRight" size={14} />
      </span>
    </div>
  );
}
