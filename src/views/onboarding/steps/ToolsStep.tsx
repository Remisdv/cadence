/**
 * Outils nécessaires (V2-Onboarding-Tools.dc.html) : programmes de la machine demandés par
 * les IA choisies, les services et les plugins activés. « Continuer » reste grisé tant qu'un
 * outil requis manque ; rien ne s'installe sans un clic.
 */
import { useState, type ReactNode } from "react";
import type { ToolInfo } from "@/lib/types";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Spinner } from "@/components/ui/Spinner";
import { ToolInstallDialog } from "@/components/domain/ToolInstallDialog";
import { ToolsTable } from "@/components/domain/ToolsTable";
import { OnboardingActions, OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { stepIndex } from "@/lib/domain/onboarding";
import { blocksContinue, installable, isRequested, toolsSummary } from "@/lib/domain/tools";
import { usePlugins, useRecheckTools, useTools } from "@/lib/query/queries";
import type { StepProps } from "../OnboardingView";

export function ToolsStep({ onNext, onBack }: StepProps) {
  const tools = useTools();
  const plugins = usePlugins();
  const recheck = useRecheckTools();
  /** « Tout installer » : file des outils restants, installés l'un après l'autre. */
  const [queue, setQueue] = useState<ToolInfo[]>([]);

  const frame = (children: ReactNode) => (
    <OnboardingFrame
      step={stepIndex("tools")}
      width={916}
      title="Outils nécessaires"
      intro="Vos IA et vos plugins s'appuient sur des programmes de votre machine. Cadence les a cherchés : installez ou connectez ceux qui manquent, en un clic chacun. « Continuer » reste grisé tant qu'un outil requis manque."
    >
      {children}
    </OnboardingFrame>
  );

  if (tools.isPending || plugins.isPending) return frame(<LoadingState />);
  if (tools.isError) return frame(<ErrorState error={tools.error} />);

  const report = tools.data;
  const requested = report.tools.filter(isRequested);
  const summary = toolsSummary(requested.filter((t) => !t.ignored));
  const blocking = requested.filter(blocksContinue);
  const toInstall = installable(requested);
  const optionalMissing = requested.filter(
    (t) => !t.required && !t.ignored && t.status !== "ready",
  );
  const current = queue[0] ?? null;

  return frame(
    <div className="flex flex-col gap-3.5">
      <div className="flex items-center gap-[7px]">
        <Badge tone="green" icon="check">
          {summary.ready} prêt{summary.ready > 1 ? "s" : ""}
        </Badge>
        {summary.toConnect > 0 && <Badge tone="amber">{summary.toConnect} à connecter</Badge>}
        {summary.missingRequired > 0 && (
          <Badge tone="red">
            {summary.missingRequired} requis manquant{summary.missingRequired > 1 ? "s" : ""}
          </Badge>
        )}
        {summary.missingOptional > 0 && (
          <Badge tone="muted">
            {summary.missingOptional} optionnel{summary.missingOptional > 1 ? "s" : ""} manquant
            {summary.missingOptional > 1 ? "s" : ""}
          </Badge>
        )}
        <div className="grow" />
        <span className="mr-1 mono text-tiny text-tx3">
          {[report.platform, ...report.managers].join(" · ")}
        </span>
        <Button
          variant="ghost"
          size="compact"
          icon={recheck.isPending ? undefined : "refresh"}
          disabled={recheck.isPending}
          onClick={() => recheck.mutate()}
        >
          {recheck.isPending && <Spinner />}
          Revérifier
        </Button>
      </div>

      {requested.length > 0 ? (
        <ToolsTable tools={requested} plugins={plugins.data ?? []} platform={report.platform} />
      ) : (
        <div className="rounded-card border border-dashed border-line2 px-4 py-6 text-center text-body-sm text-tx3">
          Ni vos IA ni vos plugins ne demandent d'outil supplémentaire.
        </div>
      )}

      <Callout tone="info" icon="info">
        Rien n'est installé sans votre clic. Cadence affiche la commande exacte avant de la lancer,
        puis revérifie. Les connexions restent dans le stockage de chaque outil (
        <span className="mono text-tx">gh</span>, <span className="mono text-tx">codex</span>) :
        Cadence ne voit ni ne garde vos identifiants.
      </Callout>

      <OnboardingActions onBack={onBack}>
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          disabled={blocking.length > 0}
          onClick={onNext}
        >
          Continuer
        </Button>
        {toInstall.length > 0 && (
          <Button size="lg" icon="download" onClick={() => setQueue(toInstall)}>
            Tout installer · {toInstall.length} outil{toInstall.length > 1 ? "s" : ""}
          </Button>
        )}
        <span className="text-body-sm text-tx3">
          {blocking.length > 0
            ? `Requis : ${blocking.map((t) => t.name).join(", ")}.`
            : optionalMissing.length > 0
              ? `Aucun outil requis ne manque : ${optionalMissing.map((t) => t.name).join(", ")} ${
                  optionalMissing.length > 1 ? "sont optionnels" : "est optionnel"
                }.`
              : "Tout est prêt."}
        </span>
      </OnboardingActions>

      {current && (
        <ToolInstallDialog
          key={current.id}
          tool={report.tools.find((t) => t.id === current.id) ?? current}
          start="install"
          platform={report.platform}
          open
          onOpenChange={(open) => !open && setQueue((q) => q.slice(1))}
        />
      )}
    </div>,
  );
}
