/**
 * Onboarding 2/3 — Agents de code (maquette Onboarding-Agents.dc.html).
 *
 * Rust détecte Claude Code et Codex CLI (chemin, version) ; une courte session de test
 * vérifie que Claude Code répond et qu'il est authentifié. L'affectation des rôles est une
 * préférence de la machine, reprise dans la configuration du projet à l'étape suivante.
 */
import { useEffect, useRef } from "react";
import type { AgentEngines, AgentKind, CliAgentInfo, EngineChoice } from "@/lib/types";
import { OnboardingLayout } from "@/components/layout/OnboardingLayout";
import { Icon } from "@/components/icons/Icon";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Select } from "@/components/ui/Field";
import { KeyValue } from "@/components/ui/KeyValue";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { agentOrder, agents } from "@/lib/domain/agents";
import { formatDecimal } from "@/lib/domain/format";
import { pickFile } from "@/lib/platform";
import {
  type CliProvider,
  useAgentEngines,
  useAuthStatus,
  useSaveAgentEngines,
  useSetCliEnabled,
  useSetCliPath,
  useTestCliAgent,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";

export default function AgentsView() {
  const auth = useAuthStatus().data;
  const navigate = useNavigation((s) => s.navigate);
  // Affectable dès que le binaire est là et non écarté, même si le test échoue (login à refaire).
  const usable = (info?: CliAgentInfo) =>
    Boolean(info?.binaryPath) && info?.status !== "disconnected";
  const anyAgent = usable(auth?.claudeCode) || usable(auth?.codex);

  // Précharge l'étape suivante.
  useEffect(() => {
    void import("./ProjectView");
  }, []);

  return (
    <OnboardingLayout
      step={1}
      title="Brancher vos agents de code"
      intro="Les agents tournent en local, sur votre machine, via le binaire de votre assistant. Cadence orchestre : il prépare le contexte, lance la session en mode headless et lit le flux d'événements."
    >
      <div className="max-w-[880px]">
        <div className="grid grid-cols-2 gap-4">
          <CliAgentCard
            provider="claude-code"
            name="Claude Code"
            role="Agent par défaut"
            binary="claude"
            info={auth?.claudeCode}
          />
          <CliAgentCard
            provider="codex"
            name="Codex CLI"
            role="Agent secondaire"
            binary="codex"
            info={auth?.codex}
          />
        </div>

        <SectionLabel className="mt-[22px]">Affectation des rôles</SectionLabel>
        <RoleAssignment
          claudeAvailable={usable(auth?.claudeCode)}
          codexAvailable={usable(auth?.codex)}
        />

        <div className="mt-[22px] flex items-center gap-3.5">
          <Button
            variant="primary"
            size="lg"
            iconRight="arrowRight"
            onClick={() => navigate({ id: "onboarding-project" })}
          >
            Continuer
          </Button>
          <button
            type="button"
            className="text-body text-tx3 hover:text-tx"
            onClick={() => navigate({ id: "onboarding-github" })}
          >
            Retour
          </button>
          {auth && !anyAgent && (
            <span className="text-caption text-tx3">
              Aucun agent n'est requis pour commencer : vous pourrez en brancher un plus tard.
            </span>
          )}
        </div>
      </div>
    </OnboardingLayout>
  );
}

/* ---------- Carte d'un agent de code ---------- */

interface CliAgentCardProps {
  provider: CliProvider;
  name: string;
  role: string;
  binary: string;
  info?: CliAgentInfo;
}

function CliAgentCard({ provider, name, role, binary, info }: CliAgentCardProps) {
  const test = useTestCliAgent(provider);
  const setPath = useSetCliPath(provider);
  const setEnabled = useSetCliEnabled(provider);
  const supportsTest = provider === "claude-code";
  const autoTested = useRef(false);

  // Premier passage : on teste dès que le binaire est détecté.
  useEffect(() => {
    if (supportsTest && info?.status === "pending" && !autoTested.current) {
      autoTested.current = true;
      test.mutate();
    }
  }, [supportsTest, info?.status, test]);

  const locate = async () => {
    const path = await pickFile(`Localiser le binaire ${binary}`);
    if (path) setPath.mutate(path);
  };

  const status = info?.status;
  const found = Boolean(info?.binaryPath);
  const active = status === "connected" || status === "pending";
  const error = setPath.error ?? setEnabled.error ?? test.error;

  return (
    <Card className="h-full">
      <div className="flex h-full flex-col p-5">
        <div className="flex items-center gap-[11px]">
          <span
            className={cn(
              "inline-flex size-[34px] items-center justify-center rounded-field text-ink",
              active || status === "error" ? "bg-clay" : "bg-tx3",
            )}
          >
            <Icon name="sparkle" size={19} />
          </span>
          <div className="min-w-0 grow">
            <div className="text-title-sm font-semibold">{name}</div>
            <div className="mt-px text-caption text-tx3">{role}</div>
          </div>
          <CliStatusBadge info={info} testing={test.isPending} />
        </div>

        <div className="mt-4 grow">
          {!info ? (
            <div className="flex h-full items-center justify-center">
              <Spinner />
            </div>
          ) : found ? (
            <>
              <KeyValue label="Binaire" value={<PathValue path={info.binaryPath!} />} />
              <KeyValue label="Version" value={info.version ?? "—"} />
              <KeyValue label="Authentification" value={info.authMethod ?? "—"} />
              <KeyValue
                label="Session de test"
                value={supportsTest ? testLabel(info, test.isPending) : "non disponible"}
              />
              {info.test && !info.test.ok && info.test.message && (
                <TestFailure message={info.test.message} binary={binary} />
              )}
            </>
          ) : (
            <p className="text-body-sm leading-[1.65] text-tx3">
              {status === "disconnected" ? (
                <>Agent écarté. Réactivez-le pour relancer la détection.</>
              ) : (
                <>
                  Aucun binaire <span className="mono text-tx2">{binary}</span> trouvé dans le PATH
                  ni aux emplacements usuels. Vous pourrez l'ajouter plus tard : un agent peut être
                  réaffecté à tout moment sans perdre son historique.
                </>
              )}
            </p>
          )}
          {error && <p className="mt-3 text-caption text-red">{error.message}</p>}
        </div>

        <div className="mt-4 flex gap-2">
          {status === "disconnected" ? (
            <Button
              icon="refresh"
              disabled={setEnabled.isPending}
              onClick={() => setEnabled.mutate(true)}
            >
              Réactiver
            </Button>
          ) : found ? (
            <>
              {supportsTest && (
                <Button
                  icon={test.isPending ? undefined : "refresh"}
                  disabled={test.isPending}
                  onClick={() => test.mutate()}
                >
                  {test.isPending && <Spinner />}
                  {info?.test ? "Relancer le test" : "Lancer le test"}
                </Button>
              )}
              <Button variant="ghost" icon="folder" onClick={locate}>
                Autre binaire…
              </Button>
              <Button
                variant="ghost"
                disabled={setEnabled.isPending}
                onClick={() => setEnabled.mutate(false)}
              >
                Déconnecter
              </Button>
            </>
          ) : (
            <>
              <Button icon="folder" disabled={setPath.isPending} onClick={locate}>
                Localiser le binaire…
              </Button>
              <Button
                variant="ghost"
                disabled={setEnabled.isPending}
                onClick={() => setEnabled.mutate(false)}
              >
                Ignorer
              </Button>
            </>
          )}
        </div>
      </div>
    </Card>
  );
}

function CliStatusBadge({ info, testing }: { info?: CliAgentInfo; testing: boolean }) {
  if (!info) return null;
  if (testing)
    return (
      <Badge tone="blue" dot>
        Test en cours
      </Badge>
    );
  switch (info.status) {
    case "connected":
      return <Badge tone="green">Connecté</Badge>;
    case "pending":
      return <Badge tone="blue">Détecté</Badge>;
    case "error":
      return <Badge tone="red">Échec du test</Badge>;
    case "disconnected":
      return <Badge tone="neutral">{info.binaryPath ? "Déconnecté" : "Ignoré"}</Badge>;
    case "not-found":
      return <Badge tone="neutral">Introuvable</Badge>;
  }
}

function testLabel(info: CliAgentInfo, testing: boolean): string {
  if (testing) return "en cours…";
  if (!info.test) return "pas encore lancée";
  const seconds = `${formatDecimal(info.test.latencyMs / 1000)} s`;
  return info.test.ok ? `OK · ${seconds}` : `échec · ${seconds}`;
}

/** Chemin long : on garde la fin (le nom du binaire), le reste en infobulle. */
function PathValue({ path }: { path: string }) {
  return (
    <span title={path} className="block max-w-[230px] truncate [direction:rtl]">
      <bdi>{path}</bdi>
    </span>
  );
}

function TestFailure({ message, binary }: { message: string; binary: string }) {
  const needsLogin = /auth|login|expired|api key/i.test(message);
  return (
    <div className="mt-3 rounded-field border border-red-line bg-red-d px-3 py-2.5 text-caption leading-[1.55] text-tx2">
      <div className="text-red">{message}</div>
      {needsLogin && (
        <div className="mt-1">
          Ouvrez un terminal, lancez <span className="mono text-tx">{binary}</span> puis{" "}
          <span className="mono text-tx">/login</span>, et relancez le test.
        </div>
      )}
    </div>
  );
}

/* ---------- Affectation des rôles ---------- */

const engineOptions: { value: string; label: string; choice: EngineChoice }[] = [
  {
    value: "claude-code:opus",
    label: "Claude Code · Opus",
    choice: { engine: "claude-code", model: "opus" },
  },
  {
    value: "claude-code:sonnet",
    label: "Claude Code · Sonnet",
    choice: { engine: "claude-code", model: "sonnet" },
  },
  { value: "codex:", label: "Codex CLI", choice: { engine: "codex", model: null } },
];

const valueOf = (choice: EngineChoice) => `${choice.engine}:${choice.model ?? ""}`;

function RoleAssignment({
  claudeAvailable,
  codexAvailable,
}: {
  claudeAvailable: boolean;
  codexAvailable: boolean;
}) {
  const engines = useAgentEngines().data;
  const save = useSaveAgentEngines();

  const change = (kind: AgentKind, value: string) => {
    const option = engineOptions.find((o) => o.value === value);
    if (engines && option)
      save.mutate({ ...engines, [kind]: option.choice } satisfies AgentEngines);
  };

  return (
    <Card className="mt-2 px-[18px] py-1">
      {agentOrder.map((kind, i) => {
        const meta = agents[kind];
        const current = engines?.[kind];
        return (
          <div
            key={kind}
            className={cn(
              "flex items-center gap-3 py-[13px]",
              i < agentOrder.length - 1 && "border-b border-line",
            )}
          >
            <AgentBadge agent={kind} size={28} />
            <div className="min-w-0 grow">
              <div className="text-body font-medium">{meta.name}</div>
              <div className="mt-px text-caption text-tx3">{meta.role}</div>
            </div>
            <Select
              size="sm"
              aria-label={`Moteur pour ${meta.name}`}
              disabled={!current}
              value={current ? valueOf(current) : ""}
              onChange={(e) => change(kind, e.target.value)}
            >
              {engineOptions.map((option) => {
                const available =
                  option.choice.engine === "codex" ? codexAvailable : claudeAvailable;
                const selected = current && valueOf(current) === option.value;
                return (
                  <option
                    key={option.value}
                    value={option.value}
                    disabled={!available && !selected}
                  >
                    {option.label}
                    {available ? "" : " (indisponible)"}
                  </option>
                );
              })}
            </Select>
          </div>
        );
      })}
    </Card>
  );
}
