/**
 * Terminé : récapitulatif de l'onboarding (IA par défaut, plugins activés, outils, dépôts
 * suivis), puis ouverture du chat global. Marque l'onboarding comme fait (`onboarded`).
 */
import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Button } from "@/components/ui/Button";
import { Spinner } from "@/components/ui/Spinner";
import { PluginTile } from "@/components/domain/PluginTile";
import { OnboardingActions, OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { aiLabels } from "@/lib/domain/ai";
import { stepIndex } from "@/lib/domain/onboarding";
import { blocksContinue, isRequested } from "@/lib/domain/tools";
import { useAppSettings, usePlugins, useSaveAppSettings, useTools } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import type { StepProps } from "../OnboardingView";

export function DoneStep({ onBack }: StepProps) {
  const settings = useAppSettings().data;
  const plugins = usePlugins().data ?? [];
  const tools = useTools().data;
  const save = useSaveAppSettings();
  const replace = useNavigation((s) => s.replace);
  const enabled = plugins.filter((p) => p.enabled);
  const requested = tools?.tools.filter(isRequested) ?? [];
  const missing = requested.filter(blocksContinue);
  const repos = Object.values(settings?.trackedRepos ?? {}).flat();
  const toSetup = enabled.filter((p) => p.status === "needs-setup");

  const finish = () =>
    save.mutate({ onboarded: true }, { onSuccess: () => replace({ id: "chat" }) });

  return (
    <OnboardingFrame
      step={stepIndex("done")}
      title="Cadence est prêt"
      intro="Votre IA par défaut répond dans le chat, avec les outils des plugins activés. Chaque plugin a pris sa place dans le rail ; tout reste modifiable dans les réglages."
    >
      <div className="flex flex-col gap-3.5">
        <div className="grid grid-cols-2 gap-3">
          <Summary
            icon="sparkle"
            title="IA par défaut"
            value={settings?.defaultAi ? aiLabels[settings.defaultAi].name : "Aucune"}
          >
            Les actions des plugins peuvent ensuite utiliser une autre IA, dans Réglages › IA.
          </Summary>
          <Summary icon="grid" title="Plugins activés" value={`${enabled.length}`}>
            {enabled.length > 0 ? (
              <span className="flex flex-wrap gap-1.5">
                {enabled.map((p) => (
                  <span key={p.id} className="inline-flex items-center gap-1.5 text-tx2">
                    <PluginTile color={p.color} icon={p.railIcon} size={14} />
                    {p.name}
                  </span>
                ))}
              </span>
            ) : (
              "Le catalogue reste ouvert depuis le rail."
            )}
          </Summary>
          <Summary
            icon="wrench"
            title="Outils"
            value={missing.length > 0 ? `${missing.length} à finir` : "Prêts"}
          >
            {missing.length > 0
              ? `${missing.map((t) => t.name).join(", ")} : à installer ou connecter dans Réglages › Outils.`
              : "Tout ce que vos IA et vos plugins demandent est installé."}
          </Summary>
          <Summary icon="github" title="Dépôts suivis" value={`${repos.length}`}>
            {repos.length > 0
              ? repos.slice(0, 3).join(", ") + (repos.length > 3 ? "…" : "")
              : "Aucun pour l'instant : le sélecteur de la barre du plugin en ajoute."}
          </Summary>
        </div>

        {toSetup.length > 0 && (
          <div className="rounded-tile border border-amber-line bg-amber-d px-3.5 py-3 text-body-sm leading-[1.6] text-tx2">
            À terminer : {toSetup.map((p) => p.name).join(", ")}. Le plugin affiche un bandeau dans
            sa barre tant que sa configuration manque.
          </div>
        )}

        <OnboardingActions className="mt-2" onBack={onBack}>
          <Button
            variant="primary"
            size="lg"
            iconRight={save.isPending ? undefined : "arrowRight"}
            autoFocus
            disabled={save.isPending}
            onClick={finish}
          >
            {save.isPending && <Spinner className="text-clay-ink" />}
            Ouvrir Cadence
          </Button>
          {save.error && <span className="text-caption text-red">{save.error.message}</span>}
        </OnboardingActions>
      </div>
    </OnboardingFrame>
  );
}

function Summary({
  icon,
  title,
  value,
  children,
}: {
  icon: IconName;
  title: string;
  value: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 rounded-card border border-line bg-panel p-4">
      <div className="flex items-center gap-2">
        <span className="flex text-clay">
          <Icon name={icon} size={16} />
        </span>
        <h3 className="grow text-title-sm">{title}</h3>
        <span className="text-body font-semibold text-tx">{value}</span>
      </div>
      <div className="text-body-sm leading-[1.6] text-tx3">{children}</div>
    </div>
  );
}
