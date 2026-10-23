/**
 * Réglages › Outils externes (V2-Settings.dc.html) : tous les outils du registre, leur chemin,
 * leur état ; installation, connexion et chemin personnalisé ; rappel des garanties et de la
 * déclaration `[[requires]]` d'un manifeste.
 */
import type { IconName } from "@/components/icons/Icon";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { ToolsTable } from "@/components/domain/ToolsTable";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { formatAgo } from "@/lib/domain/format";
import { usePlugins, useRecheckTools, useTools } from "@/lib/query/queries";
import { SectionTitle } from "./SectionTitle";

const guarantees: { icon: IconName; title: string; text: string }[] = [
  {
    icon: "terminal",
    title: "Commande exacte, affichée avant d'être lancée",
    text: "Elle tourne dans un terminal intégré en lecture seule, avec le journal en direct, puis Cadence revérifie.",
  },
  {
    icon: "shield",
    title: "Rien sans votre clic",
    text: "Aucune installation silencieuse. Si l'outil en a besoin, le système demande lui-même l'élévation (UAC, mot de passe).",
  },
  {
    icon: "key",
    title: "Identifiants laissés aux outils",
    text: "gh, claude et codex gardent leur connexion dans leur propre stockage : Cadence ne la voit pas.",
  },
];

export function ToolsSettings() {
  const tools = useTools();
  const plugins = usePlugins();
  const recheck = useRecheckTools();

  if (tools.isPending) return <LoadingState />;
  if (tools.isError) return <ErrorState error={tools.error} />;
  const report = tools.data;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-end gap-4">
        <div className="max-w-[760px] min-w-0 grow">
          <SectionTitle title="Outils externes">
            Programmes de votre machine dont ont besoin les IA de code, les plugins et les services.
            Cadence les détecte au lancement, à l'activation d'un plugin et sur demande ; il
            n'installe ni ne connecte rien sans votre clic.
          </SectionTitle>
        </div>
        <div className="grow" />
        <span className="text-caption whitespace-nowrap text-tx3">
          Vérifié {formatAgo(new Date(tools.dataUpdatedAt).toISOString())}
        </span>
        <Button
          icon={recheck.isPending ? undefined : "refresh"}
          disabled={recheck.isPending}
          onClick={() => recheck.mutate()}
        >
          {recheck.isPending && <Spinner />}
          Revérifier
        </Button>
      </div>

      <ToolsTable
        variant="settings"
        tools={report.tools}
        plugins={plugins.data ?? []}
        platform={report.platform}
      />

      <div className="grid grid-cols-2 gap-4">
        <div className="rounded-card border border-line bg-panel px-4 pt-1.5 pb-3">
          {guarantees.map((g, i) => (
            <div
              key={g.title}
              className={
                i < guarantees.length - 1
                  ? "flex gap-2.5 border-b border-line py-2.5"
                  : "flex gap-2.5 py-2.5"
              }
            >
              <span className="mt-px flex text-clay">
                <Icon name={g.icon} size={15} />
              </span>
              <div>
                <div className="text-body-sm">{g.title}</div>
                <div className="mt-[3px] text-caption leading-[1.55] text-tx3">{g.text}</div>
              </div>
            </div>
          ))}
          <div className="mt-1 flex flex-wrap items-center gap-1.5 text-caption text-tx3">
            Gestionnaires détectés
            {report.managers.length === 0 && <span>aucun</span>}
            {report.managers.map((m) => (
              <Badge key={m} tone="muted" mono>
                {m}
              </Badge>
            ))}
          </div>
        </div>
        <div>
          <SectionLabel>Déclaré dans le manifeste</SectionLabel>
          <pre className="mt-2 rounded-[11px] border border-line bg-panel px-3.5 py-3 mono text-caption leading-[1.7] whitespace-pre-wrap text-tx">
            <span className="text-tx3">
              # déclaré par le service repos, pour les plugins qui l'activent
            </span>
            {"\n[[requires]]\ntool = "}
            <span className="text-green">"gh"</span>
            {"\noptional = "}
            <span className="text-amber">true</span>
            {"\nauth = "}
            <span className="text-amber">true</span>
            {"\nreason = "}
            <span className="text-green">"Réutiliser votre connexion GitHub existante"</span>
          </pre>
          <div className="mt-2.5 text-caption leading-[1.6] text-tx3">
            Le serveur d'un plugin reçoit <span className="mono text-tx2">CADENCE_TOOLS</span> : le
            chemin absolu de chaque outil détecté, chemins personnalisés compris.
          </div>
        </div>
      </div>
    </div>
  );
}
