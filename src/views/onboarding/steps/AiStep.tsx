/**
 * IA de code (V2-Onboarding-AI.dc.html) : détection des trois CLI, compte connecté, session de
 * test, choix de l'IA par défaut. Au moins une IA installée est requise pour continuer.
 */
import type { ReactNode } from "react";
import type { AiId } from "@/lib/types";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { AiCard } from "@/components/domain/AiCard";
import { OnboardingActions, OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { aiOrder } from "@/lib/domain/ai";
import { toolOfAi } from "@/lib/domain/tools";
import { stepIndex } from "@/lib/domain/onboarding";
import { useAppSettings, useRecheckTools, useSaveAppSettings, useTools } from "@/lib/query/queries";
import type { StepProps } from "../OnboardingView";

const transmitted: { icon: IconName; title: string; text: string }[] = [
  {
    icon: "wrench",
    title: "Les outils MCP des plugins activés",
    text: "Via une passerelle locale sur 127.0.0.1, avec un jeton propre à la session",
  },
  {
    icon: "sparkle",
    title: "Les skills des plugins",
    text: "Passés en option de lancement : rien n'est écrit dans vos dépôts",
  },
  {
    icon: "folder",
    title: "Un dossier de travail neutre",
    text: "L'IA est lancée dans un dossier de session de Cadence, jamais dans un dépôt. Pour modifier du code, le service Dépôts lui ouvre le worktree de la tâche, pour cette session seulement.",
  },
];

export function AiStep({ onNext, onBack }: StepProps) {
  const tools = useTools();
  const settings = useAppSettings();
  const save = useSaveAppSettings();
  const recheck = useRecheckTools();

  const frame = (children: ReactNode) => (
    <OnboardingFrame
      step={stepIndex("ai")}
      width={940}
      title="Vos IA de code"
      intro="Cadence cherche les CLI installées et vérifie qu'elles sont connectées à votre compte. Il les pilote avec vos abonnements : aucune clé API n'est demandée ni stockée."
    >
      {children}
    </OnboardingFrame>
  );

  if (tools.isPending || settings.isPending) return frame(<LoadingState />);
  if (tools.isError) return frame(<ErrorState error={tools.error} />);

  const report = tools.data;
  const byAi = (ai: AiId) => report.tools.find((t) => t.id === toolOfAi[ai])!;
  const node = report.tools.find((t) => t.id === "node");
  const installed = aiOrder.filter((ai) => byAi(ai).status !== "missing");
  const ready = aiOrder.filter((ai) => byAi(ai).status === "ready");
  const current = settings.data?.defaultAi ?? null;
  const defaultAi =
    current && installed.includes(current) ? current : (ready[0] ?? installed[0] ?? null);

  const setDefault = (ai: AiId) => save.mutate({ defaultAi: ai });
  const next = () => {
    if (defaultAi && defaultAi !== current) save.mutate({ defaultAi }, { onSuccess: onNext });
    else onNext();
  };

  return frame(
    <>
      <div className="grid grid-cols-3 gap-4">
        {aiOrder.map((ai) => (
          <AiCard
            key={ai}
            tool={byAi(ai)}
            node={node}
            platform={report.platform}
            isDefault={defaultAi === ai}
            onSetDefault={() => setDefault(ai)}
          />
        ))}
      </div>

      <Callout tone="info" icon="info" className="mt-[22px] max-w-[760px]">
        L'IA par défaut répond dans le chat et exécute les boutons d'action. Chaque action peut
        ensuite utiliser une autre IA : par exemple <span className="text-tx">« Implémenter »</span>{" "}
        avec Codex CLI et <span className="text-tx">« Compléter la description »</span> avec Claude
        Code.
      </Callout>

      <SectionLabel className="mt-[22px]">Ce que Cadence transmet à l'IA</SectionLabel>
      <div className="mt-1.5 max-w-[760px]">
        {transmitted.map((t) => (
          <div key={t.title} className="flex items-start gap-2.5 border-b border-line py-[11px]">
            <span className="mt-px flex text-clay">
              <Icon name={t.icon} size={15} />
            </span>
            <div>
              <div className="text-body-sm">{t.title}</div>
              <div className="mt-0.5 text-small leading-[1.55] text-tx3">{t.text}</div>
            </div>
          </div>
        ))}
      </div>

      <OnboardingActions className="mt-6" onBack={onBack}>
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          disabled={installed.length === 0 || save.isPending}
          onClick={next}
        >
          Continuer
        </Button>
        <Button
          variant="ghost"
          icon={recheck.isPending ? undefined : "refresh"}
          disabled={recheck.isPending}
          onClick={() => recheck.mutate()}
        >
          {recheck.isPending && <Spinner />}
          Relancer la détection
        </Button>
        {installed.length === 0 && (
          <span className="text-body-sm text-tx3">Installez au moins une IA pour continuer.</span>
        )}
      </OnboardingActions>
    </>,
  );
}
