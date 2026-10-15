/**
 * Console des agents (maquette Agents-Hub.dc.html) : une carte par agent (statut, autonomie,
 * actions en attente, exécution en cours, dernier passage), journal des exécutions tous
 * agents confondus, garde-fous et consommation sur 7 jours.
 *
 * Statut : `agents_overview` ; journal : `agent_runs_list` (table `agent_runs` du cache).
 * Les deux se rafraîchissent à chaque `agent://state`. La validation avant écriture se règle
 * dans `.cadence/config.toml` (même chemin que Réglages › Agents).
 */
import { useState } from "react";
import type { AgentKind, AgentOverview, AgentRunEntry, Autonomy, Config } from "@/lib/types";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { AgentStatusCard, RunLogRow } from "@/components/domain/AgentConsole";
import { TitledCard } from "@/components/domain/Panels";
import { MeterList } from "@/components/domain/Stats";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { SwitchField } from "@/components/ui/Switch";
import { agentOrder, agents, autonomyLabels } from "@/lib/domain/agents";
import {
  agentCardTone,
  agentDescriptions,
  agentFooter,
  agentNote,
  agentStatus,
  autoAgents,
  consoleSubtitle,
  formatSpan,
  isActive,
  journalTime,
  latestByAgent,
  runHeadline,
  runStatus,
  usageOver,
  withReview,
} from "@/lib/domain/agentConsole";
import { formatPercent } from "@/lib/domain/format";
import { useElapsedSeconds } from "@/lib/hooks/useElapsedSeconds";
import {
  useAgentRuns,
  useAgentsOverview,
  useControlAgents,
  useSaveSettings,
  useSettings,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";

/** Lignes du journal : récentes par défaut, « Journal complet » va plus loin. */
const RECENT = 12;
/** Exécutions lues : assez pour la consommation sur 7 jours, plus pour le journal complet. */
const FETCHED = 30;
const FULL = 150;

export default function AgentsConsoleView() {
  const overview = useAgentsOverview();
  const [full, setFull] = useState(false);
  const runs = useAgentRuns(full ? FULL : FETCHED);
  const control = useControlAgents();
  const list = overview.data ?? [];
  const entries = runs.data ?? [];

  // Exécutions qui avancent encore : celles du statut, complétées par le journal.
  const active = [
    ...new Set([
      ...list.flatMap((a) =>
        a.currentRun && isActive(a.currentRun.state) ? [a.currentRun.run] : [],
      ),
      ...entries.filter((e) => isActive(e.state)).map((e) => e.run),
    ]),
  ];
  const pausedRuns = new Set(entries.filter((e) => e.paused).map((e) => e.run));
  const allPaused = active.length > 0 && active.every((r) => pausedRuns.has(r));

  return (
    <Page
      title="Console des agents"
      subtitle={overview.data ? consoleSubtitle(list) : "Statut des agents…"}
      actions={
        <>
          <Button variant="ghost" icon="file" onClick={() => setFull((f) => !f)}>
            {full ? "Journal récent" : "Journal complet"}
          </Button>
          <Button
            icon={allPaused ? "play" : "pause"}
            disabled={active.length === 0 || control.isPending}
            title={active.length === 0 ? "Aucun agent ne travaille" : undefined}
            onClick={() =>
              control.mutate({ runs: active, cmd: { cmd: allPaused ? "resume" : "pause" } })
            }
          >
            {allPaused ? "Tout reprendre" : "Tout suspendre"}
          </Button>
        </>
      }
    >
      {overview.isPending ? (
        <LoadingState label="Statut des agents…" />
      ) : overview.isError ? (
        <ErrorState error={overview.error} />
      ) : (
        <div className="flex h-full min-h-[560px] flex-col gap-[13px]">
          {control.isError && (
            <p role="alert" className="text-caption text-red">
              {control.error.message}
            </p>
          )}
          <AgentCards overview={list} entries={entries} />
          <div className="flex min-h-0 grow gap-[13px]">
            <Journal runs={runs} entries={full ? entries : entries.slice(0, RECENT)} full={full} />
            <div className="flex w-[330px] shrink-0 flex-col gap-[13px]">
              <Guardrails />
              <Usage entries={entries} />
            </div>
          </div>
        </div>
      )}
    </Page>
  );
}

/* ---------- Cartes d'agent ---------- */

function AgentCards({ overview, entries }: { overview: AgentOverview[]; entries: AgentRunEntry[] }) {
  const latest = latestByAgent(entries);
  return (
    // Quatre colonnes à la largeur des maquettes, retour à la ligne sur une fenêtre étroite.
    <div className="grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-3">
      {agentOrder.map((kind) => {
        const agent = overview.find((a) => a.kind === kind);
        return agent ? <AgentCard key={kind} agent={agent} last={latest[kind] ?? null} /> : null;
      })}
    </div>
  );
}

function AgentCard({ agent, last }: { agent: AgentOverview; last: AgentRunEntry | null }) {
  const navigate = useNavigation((s) => s.navigate);
  const running = agent.activity === "running" ? agent.currentRun : null;
  const elapsed = useElapsedSeconds(running?.startedAt);
  const paused = running !== null && last?.run === running.run && last.paused;
  return (
    <AgentStatusCard
      agent={agent.kind}
      autonomy={autonomyLabels[agent.autonomy]}
      status={agentStatus(agent, elapsed, paused)}
      description={agentDescriptions[agent.kind]}
      note={agentNote(agent, last)}
      footer={agentFooter(agent, last)}
      highlight={agentCardTone(agent)}
      onOpen={() => navigate({ id: "agent", kind: agent.kind })}
    />
  );
}

/* ---------- Journal ---------- */

function Journal({
  runs,
  entries,
  full,
}: {
  runs: ReturnType<typeof useAgentRuns>;
  entries: AgentRunEntry[];
  full: boolean;
}) {
  const navigate = useNavigation((s) => s.navigate);
  return (
    <TitledCard
      className="min-w-0 grow"
      title="Journal d'exécution"
      subtitle={full ? `${entries.length} exécution${entries.length > 1 ? "s" : ""}` : undefined}
      actions={<Badge tone="neutral">Toutes les écritures GitHub sont tracées</Badge>}
    >
      {runs.isPending ? (
        <LoadingState label="Lecture du journal…" />
      ) : runs.isError ? (
        <ErrorState error={runs.error} />
      ) : entries.length === 0 ? (
        <EmptyState icon="bot" title="Aucune exécution sur ce dépôt">
          Lancez un agent depuis son écran : chaque exécution, ce qu'elle a proposé et ce qu'elle a
          écrit sur GitHub apparaissent ici.
        </EmptyState>
      ) : (
        entries.map((e) => (
          <RunLogRow
            key={e.run}
            agent={e.kind}
            time={journalTime(e.startedAt)}
            status={runStatus(e)}
            onClick={() => navigate({ id: "agent", kind: e.kind })}
          >
            {runHeadline(e)}
          </RunLogRow>
        ))
      )}
    </TitledCard>
  );
}

/* ---------- Garde-fous ---------- */

function Guardrails() {
  const settings = useSettings();
  const save = useSaveSettings();
  const config = settings.data;
  const autonomies = config && autonomiesOf(config);
  const autos = autonomies ? autoAgents(autonomies) : [];
  const review = autos.length === 0;

  const setReview = (on: boolean) => {
    if (!config || !autonomies) return;
    const next = withReview(autonomies, on);
    const agentsConfig = { ...config.agents };
    for (const kind of agentOrder) agentsConfig[kind] = { ...agentsConfig[kind], autonomy: next[kind] };
    save.mutate({ ...config, agents: agentsConfig });
  };

  return (
    <Card className="px-[15px] py-3.5">
      <CardHeader title="Garde-fous" />
      <div className="mt-[13px] flex flex-col gap-3">
        <SwitchField
          label="Validation avant écriture GitHub"
          hint={
            !config
              ? "Lecture de .cadence/config.toml…"
              : review
                ? "Vaut pour les 4 agents"
                : `Automatique : ${autos.map((k) => agents[k].short).join(", ")}`
          }
          checked={review}
          disabled={!config || save.isPending}
          onCheckedChange={setReview}
        />
        <SwitchField
          label="Jamais de push sur main"
          hint="Branche + PR obligatoires · toujours actif"
          checked
          disabled
          onCheckedChange={() => {}}
        />
        <SwitchField
          label="Autoriser la fusion automatique"
          hint="Si CI verte et 1 approbation · pas encore disponible"
          checked={false}
          disabled
          onCheckedChange={() => {}}
        />
        <SwitchField
          label="Limiter au dépôt du projet"
          hint="Aucun accès aux autres dépôts · toujours actif"
          checked
          disabled
          onCheckedChange={() => {}}
        />
        {save.isError && (
          <p role="alert" className="text-caption text-red">
            {save.error.message}
          </p>
        )}
      </div>
    </Card>
  );
}

function autonomiesOf(config: Config): Record<AgentKind, Autonomy> {
  return {
    cto: config.agents.cto.autonomy,
    ceo: config.agents.ceo.autonomy,
    dev: config.agents.dev.autonomy,
    deploy: config.agents.deploy.autonomy,
  };
}

/* ---------- Consommation ---------- */

function Usage({ entries }: { entries: AgentRunEntry[] }) {
  const usage = usageOver(entries);
  return (
    <Card className="px-[15px] py-3.5">
      <CardHeader title="Consommation · 7 jours" />
      <MeterList
        className="mt-3 gap-2.5"
        items={usage.agents.map((a) => ({
          label: agents[a.kind].name,
          value: formatPercent(a.percent),
          percent: a.percent,
          tone: agents[a.kind].tone,
        }))}
      />
      <div className="mt-3 border-t border-line pt-[11px] text-caption leading-[1.6] text-tx3">
        {usage.runs === 0
          ? "Aucune exécution ces 7 derniers jours."
          : `${usage.runs} exécution${usage.runs > 1 ? "s" : ""} · durée moyenne ${formatSpan(usage.averageSeconds)}.`}{" "}
        Rien n'est facturé à l'usage : les agents passent par votre abonnement.
      </div>
    </Card>
  );
}
