/**
 * Onboarding 1/3 — Connexion GitHub (maquette Onboarding-Welcome.dc.html).
 *
 * Méthode principale : navigateur (flux web OAuth). Rust ouvre la page d'autorisation, attend le
 * retour sur 127.0.0.1 et range le jeton dans le trousseau. Alternatives : code d'appairage
 * (device flow, sans navigateur sur la machine) et jeton personnel. L'issue arrive par `auth://changed`.
 */
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { AuthFailure, DeviceFlowStart, GithubAuth, GithubSources } from "@/lib/types";
import { OnboardingLayout } from "@/components/layout/OnboardingLayout";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { formatDuration } from "@/lib/domain/format";
import { useCountdown } from "@/lib/hooks/useCountdown";
import { CommandError } from "@/lib/ipc/errors";
import { subscribe } from "@/lib/ipc/events";
import { copyText, openExternal } from "@/lib/platform";
import {
  useAuthStatus,
  useSignInWithGhCli,
  useSignOut,
  useStartDeviceFlow,
  useStartWebFlow,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { PairingCode } from "./PairingCode";
import { PatDialog } from "./PatDialog";

export const permissions: { scope: string; description: string; required: boolean }[] = [
  {
    scope: "repo",
    description: "Lire et écrire issues, PR, milestones, branches et releases",
    required: true,
  },
  {
    scope: "read:org",
    description: "Lister les dépôts des organisations dont vous êtes membre",
    required: true,
  },
  {
    scope: "workflow",
    description: "Déclencher et lire les runs GitHub Actions",
    required: true,
  },
  { scope: "read:user", description: "Afficher votre avatar et votre nom", required: false },
];

type FailReason = AuthFailure | "config";

type Flow =
  | { kind: "starting" }
  | { kind: "waiting"; code: DeviceFlowStart; expiresAt: number }
  | { kind: "failed"; reason: FailReason; message: string | null };

export default function GithubView() {
  const auth = useAuthStatus();
  const github = auth.data?.github;
  const connected = github?.status === "connected";
  const navigate = useNavigation((s) => s.navigate);
  const [patOpen, setPatOpen] = useState(false);
  const [mode, setMode] = useState<"browser" | "code">("browser");

  return (
    <OnboardingLayout
      step={0}
      title="Connecter votre compte GitHub"
      intro="Autorisez Cadence depuis votre navigateur : GitHub vous demande de confirmer, puis vous revenez ici automatiquement. Le jeton est rangé dans le trousseau du système, jamais dans l'application."
    >
      <div className="flex max-w-[860px] gap-[26px]">
        <div className="w-[400px] shrink-0">
          {auth.isPending ? (
            <Card className="flex h-[300px] items-center justify-center">
              <Spinner />
            </Card>
          ) : connected && github ? (
            <ConnectedCard github={github} onContinue={() => navigate({ id: "sprint" })} />
          ) : mode === "browser" && auth.data ? (
            <BrowserSignInCard
              sources={auth.data.githubSources}
              onUseCode={() => setMode("code")}
              onUsePat={() => setPatOpen(true)}
            />
          ) : (
            <DeviceFlowCard onUsePat={() => setPatOpen(true)} />
          )}
          {!connected && (
            <div className="mt-3.5 text-small leading-[1.6] text-tx3">
              {mode === "code" ? (
                <>
                  <LinkButton onClick={() => setMode("browser")}>
                    Se connecter plutôt via le navigateur
                  </LinkButton>
                  <br />
                </>
              ) : auth.data?.githubSources.deviceFlow ? (
                <>
                  Pas de navigateur sur cette machine ?{" "}
                  <LinkButton onClick={() => setMode("code")}>
                    Utiliser un code d'appairage
                  </LinkButton>
                  <br />
                </>
              ) : null}
              Ou{" "}
              <LinkButton onClick={() => setPatOpen(true)}>utiliser un jeton personnel</LinkButton>{" "}
              — utile pour GitHub Enterprise Server.
            </div>
          )}
        </div>

        <div className="min-w-0 grow">
          <SectionLabel>Permissions demandées</SectionLabel>
          <div className="mt-1.5">
            {permissions.map((p) => (
              <PermissionRow key={p.scope} {...p} granted={github?.scopes.includes(p.scope)} />
            ))}
          </div>
          <Callout tone="info" className="mt-5">
            Le jeton reste dans le trousseau du système : l'interface de Cadence ne le voit jamais,
            et chaque écriture sur GitHub passe par le cœur de l'application.
          </Callout>
        </div>
      </div>

      <PatDialog open={patOpen} onOpenChange={setPatOpen} />
    </OnboardingLayout>
  );
}

export function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className="text-clay hover:text-clay-hover" onClick={onClick}>
      {children}
    </button>
  );
}

/** Carte de connexion (onboarding de Cadence 1), ou contenu nu dans la carte du service. */
function SignInFrame({ bare, children }: { bare: boolean; children: ReactNode }) {
  return bare ? <div>{children}</div> : <Card className="px-6 py-[26px]">{children}</Card>;
}

/* ---------- Connexion par le navigateur ---------- */

type BrowserFlow =
  | { kind: "idle" }
  | { kind: "opening" }
  /** `authorizeUrl` nul : Git Credential Manager pilote lui-même le navigateur. */
  | { kind: "waiting"; authorizeUrl: string | null }
  | { kind: "failed"; reason: FailReason; message: string | null };

const browserFailureLabel: Record<FailReason, string> = {
  expired: "Délai dépassé",
  denied: "Autorisation refusée",
  config: "Connexion par navigateur indisponible",
  error: "Connexion impossible",
};

export function BrowserSignInCard({
  sources,
  onUseCode,
  onUsePat,
  bare = false,
}: {
  sources: GithubSources;
  onUseCode: () => void;
  onUsePat: () => void;
  /** Sans cadre : posé dans la carte du service Dépôts GitHub (Cadence 2). */
  bare?: boolean;
}) {
  const { mutate: startWebFlow } = useStartWebFlow();
  const useGh = useSignInWithGhCli();
  const [flow, setFlow] = useState<BrowserFlow>({ kind: "idle" });
  const browserAvailable = sources.oauthApp || sources.credentialManager;

  const start = () => {
    setFlow({ kind: "opening" });
    startWebFlow(undefined, {
      onSuccess: ({ authorizeUrl }) => setFlow({ kind: "waiting", authorizeUrl }),
      onError: (error) =>
        setFlow({
          kind: "failed",
          reason: error instanceof CommandError && error.kind === "config" ? "config" : "error",
          message: error.message,
        }),
    });
  };

  // Échec côté navigateur (refus, délai dépassé). Le succès passe par `useAuthStatus`.
  useEffect(
    () =>
      subscribe("auth://changed", (event) => {
        if (event.provider !== "github" || event.status !== "error") return;
        setFlow({ kind: "failed", reason: event.reason ?? "error", message: event.message });
      }),
    [],
  );

  const failed = flow.kind === "failed" ? flow : null;
  const failureText: Record<FailReason, ReactNode> = {
    expired: "Aucune réponse du navigateur dans les 10 minutes. Relancez la connexion.",
    denied: "L'autorisation a été refusée sur github.com. Vous pouvez recommencer.",
    config: failed?.message,
    error: failed?.message ?? "GitHub n'a pas répondu comme prévu.",
  };

  return (
    <SignInFrame bare={bare}>
      <div className="flex items-center gap-[9px] text-body-sm text-tx2">
        <Icon name="github" size={17} />
        github.com
      </div>

      {failed ? (
        <div className="mt-5 flex flex-col gap-4">
          <Callout tone={failed.reason === "config" ? "warning" : "danger"}>
            {failureText[failed.reason]}
          </Callout>
          {failed.reason === "config" ? (
            <div className="flex gap-[9px]">
              {sources.deviceFlow && (
                <Button variant="primary" size="lg" className="grow" onClick={onUseCode}>
                  Utiliser un code
                </Button>
              )}
              <Button className="h-[38px] grow" icon="key" onClick={onUsePat}>
                Jeton personnel
              </Button>
            </div>
          ) : (
            <Button variant="primary" size="lg" icon="refresh" block onClick={start}>
              Relancer la connexion
            </Button>
          )}
        </div>
      ) : (
        <>
          {sources.ghCli && (
            <div className="mt-5 rounded-field border border-line bg-p2 px-3.5 py-3">
              <div className="flex items-center gap-2.5">
                <Avatar size={24} tone="neutral" initials={initialsOf(sources.ghCli)} />
                <div className="min-w-0 grow">
                  <div className="truncate text-body font-medium">@{sources.ghCli}</div>
                  <div className="text-caption text-tx3">
                    Compte de la CLI GitHub sur cette machine
                  </div>
                </div>
                <Button
                  variant="primary"
                  icon={useGh.isPending ? undefined : "check"}
                  disabled={useGh.isPending}
                  onClick={() => useGh.mutate()}
                >
                  {useGh.isPending && <Spinner className="text-clay-ink" />}
                  Utiliser
                </Button>
              </div>
              {useGh.error && <p className="mt-2 text-caption text-red">{useGh.error.message}</p>}
            </div>
          )}

          <h3 className="mt-5 text-title-sm">
            {sources.ghCli ? "Ou autorisez un autre compte" : "Autorisez Cadence sur GitHub"}
          </h3>
          <p className="mt-2 text-body-sm leading-[1.6] text-tx3">
            {browserAvailable
              ? "Votre navigateur s'ouvre sur GitHub. Une fois la connexion validée, Cadence récupère la main tout seul : aucun code à recopier."
              : "Connexion par navigateur indisponible : installez Git pour Windows (Git Credential Manager) ou connectez la CLI GitHub avec gh auth login."}
          </p>
          <div className="mt-5 flex gap-[9px]">
            {flow.kind === "waiting" ? (
              flow.authorizeUrl ? (
                <Button
                  size="lg"
                  icon="github"
                  className="grow"
                  onClick={() => flow.authorizeUrl && openExternal(flow.authorizeUrl)}
                >
                  Rouvrir la page GitHub
                </Button>
              ) : (
                <Button size="lg" className="grow" disabled>
                  <Spinner />
                  Connexion dans le navigateur…
                </Button>
              )
            ) : (
              <Button
                variant={sources.ghCli ? "secondary" : "primary"}
                size="lg"
                icon={flow.kind === "opening" ? undefined : "github"}
                className="grow"
                autoFocus={!sources.ghCli}
                disabled={flow.kind === "opening" || !browserAvailable}
                onClick={start}
              >
                {flow.kind === "opening" && <Spinner className="text-clay-ink" />}
                Se connecter avec GitHub
              </Button>
            )}
          </div>
        </>
      )}

      <div className="mt-[18px] flex items-center gap-[9px] border-t border-line pt-4">
        {flow.kind === "waiting" ? (
          <>
            <StatusDot tone="amber" size={8} pulse />
            <span className="grow text-body-sm text-tx2">
              Terminez la connexion dans le navigateur…
            </span>
            <button type="button" className="text-small text-tx3 hover:text-tx" onClick={start}>
              Recommencer
            </button>
          </>
        ) : failed ? (
          <>
            <StatusDot tone="red" size={8} />
            <span className="text-body-sm text-tx2">{browserFailureLabel[failed.reason]}</span>
          </>
        ) : (
          <>
            <StatusDot tone="neutral" size={8} />
            <span className="text-body-sm text-tx2">Non connecté</span>
          </>
        )}
      </div>
    </SignInFrame>
  );
}

/* ---------- Code d'appairage (device flow) ---------- */

export function DeviceFlowCard({
  onUsePat,
  bare = false,
}: {
  onUsePat: () => void;
  bare?: boolean;
}) {
  const { mutate: startFlow } = useStartDeviceFlow();
  const [flow, setFlow] = useState<Flow>({ kind: "starting" });
  const [copied, setCopied] = useState(false);
  const started = useRef(false);

  const requestCode = useCallback(() => {
    setFlow({ kind: "starting" });
    startFlow(undefined, {
      onSuccess: (code) =>
        setFlow({ kind: "waiting", code, expiresAt: Date.now() + code.expiresIn * 1000 }),
      onError: (error) =>
        setFlow({
          kind: "failed",
          reason: error instanceof CommandError && error.kind === "config" ? "config" : "error",
          message: error.message,
        }),
    });
  }, [startFlow]);

  // Un code par passage sur l'écran (la garde résiste au double montage du mode strict).
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    requestCode();
  }, [requestCode]);

  // Issue du polling côté Rust. Le succès passe par `useAuthStatus` (invalidé globalement).
  useEffect(
    () =>
      subscribe("auth://changed", (event) => {
        if (event.provider !== "github" || event.status !== "error") return;
        setFlow({ kind: "failed", reason: event.reason ?? "error", message: event.message });
      }),
    [],
  );

  const remaining = useCountdown(flow.kind === "waiting" ? flow.expiresAt : null);
  useEffect(() => {
    if (remaining === 0) setFlow({ kind: "failed", reason: "expired", message: null });
  }, [remaining]);

  const code = flow.kind === "waiting" ? flow.code : null;

  const copy = async () => {
    if (!code) return;
    setCopied(await copyText(code.userCode));
    window.setTimeout(() => setCopied(false), 1800);
  };

  return (
    <SignInFrame bare={bare}>
      <div className="flex items-center gap-[9px] text-body-sm text-tx2">
        <Icon name="github" size={17} />
        github.com/login/device
      </div>

      {flow.kind === "failed" ? (
        <FlowProblem flow={flow} onRetry={requestCode} onUsePat={onUsePat} />
      ) : (
        <>
          <SectionLabel className="mt-5">Votre code d'appairage</SectionLabel>
          <div className="mt-2.5">
            <PairingCode code={code?.userCode ?? null} />
          </div>
          <div className="mt-3 flex h-[18px] items-center gap-[7px] text-small text-tx3">
            {remaining !== null && (
              <>
                <Icon name="clock" size={14} />
                Expire dans {formatDuration(remaining)}
              </>
            )}
          </div>
          <div className="mt-5 flex gap-[9px]">
            <Button
              variant="primary"
              size="lg"
              icon="github"
              className="grow"
              disabled={!code}
              onClick={() => code && openExternal(code.verificationUri)}
            >
              Ouvrir GitHub
            </Button>
            <Button
              className="h-[38px]"
              icon={copied ? "check" : "file"}
              disabled={!code}
              onClick={copy}
            >
              {copied ? "Copié" : "Copier"}
            </Button>
          </div>
        </>
      )}

      <div className="mt-[18px] flex items-center gap-[9px] border-t border-line pt-4">
        <FlowStatus flow={flow} />
      </div>
    </SignInFrame>
  );
}

function FlowStatus({ flow }: { flow: Flow }) {
  if (flow.kind === "starting") {
    return (
      <>
        <Spinner />
        <span className="text-body-sm text-tx2">Demande d'un code à GitHub…</span>
      </>
    );
  }
  if (flow.kind === "waiting") {
    return (
      <>
        <StatusDot tone="amber" size={8} pulse />
        <span className="text-body-sm text-tx2">En attente de votre autorisation…</span>
      </>
    );
  }
  const label: Record<FailReason, string> = {
    expired: "Code expiré",
    denied: "Autorisation refusée",
    config: "Connexion par navigateur indisponible",
    error: "Connexion impossible",
  };
  return (
    <>
      <StatusDot tone="red" size={8} />
      <span className="text-body-sm text-tx2">{label[flow.reason]}</span>
    </>
  );
}

function FlowProblem({
  flow,
  onRetry,
  onUsePat,
}: {
  flow: Flow & { kind: "failed" };
  onRetry: () => void;
  onUsePat: () => void;
}) {
  const text: Record<FailReason, ReactNode> = {
    expired:
      "Le code a expiré avant d'être saisi sur GitHub. Générez-en un nouveau : il reste valable 15 minutes.",
    denied:
      "L'autorisation a été refusée sur github.com. Vous pouvez recommencer avec un nouveau code.",
    config: flow.message,
    error: flow.message ?? "GitHub n'a pas répondu comme prévu.",
  };
  return (
    <div className="mt-5 flex flex-col gap-4">
      <Callout tone={flow.reason === "config" ? "warning" : "danger"}>{text[flow.reason]}</Callout>
      <div className="flex gap-[9px]">
        {flow.reason === "config" ? (
          <Button variant="primary" size="lg" icon="key" className="grow" onClick={onUsePat}>
            Utiliser un jeton personnel
          </Button>
        ) : (
          <Button variant="primary" size="lg" icon="refresh" className="grow" onClick={onRetry}>
            Générer un nouveau code
          </Button>
        )}
      </div>
    </div>
  );
}

/* ---------- Compte connecté ---------- */

function ConnectedCard({ github, onContinue }: { github: GithubAuth; onContinue: () => void }) {
  const signOut = useSignOut();
  const name = github.name ?? github.login ?? "Compte GitHub";
  return (
    <Card className="px-6 py-[26px]">
      <div className="flex items-center gap-[9px] text-body-sm text-green">
        <Icon name="check" size={17} />
        Compte connecté
      </div>
      <div className="mt-5 flex items-center gap-3">
        <Avatar size={26} initials={initialsOf(name)} src={github.avatarUrl} />
        <div className="min-w-0">
          <div className="truncate text-title-sm font-semibold">{name}</div>
          {github.login && <div className="mono text-caption text-tx3">@{github.login}</div>}
        </div>
      </div>
      <div className="mt-5 flex gap-[9px]">
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          className="grow"
          autoFocus
          onClick={onContinue}
        >
          Continuer
        </Button>
      </div>
      <div className="mt-[18px] flex items-center gap-[9px] border-t border-line pt-4">
        <StatusDot tone="green" size={8} />
        <span className="grow truncate text-body-sm text-tx2">
          Jeton dans le trousseau du système
        </span>
        <button
          type="button"
          disabled={signOut.isPending}
          onClick={() => signOut.mutate()}
          className="shrink-0 text-small whitespace-nowrap text-tx3 hover:text-tx"
        >
          Changer de compte
        </button>
      </div>
    </Card>
  );
}

/* ---------- Permissions ---------- */

export function PermissionRow({
  scope,
  description,
  required,
  granted,
}: {
  scope: string;
  description: string;
  required: boolean;
  granted?: boolean;
}) {
  const icon: IconName = granted ? "check" : required ? "key" : "eye";
  return (
    <div className="flex items-start gap-2.5 border-b border-line py-[11px]">
      <span
        className={cn("mt-px flex", granted ? "text-green" : required ? "text-clay" : "text-tx3")}
      >
        <Icon name={icon} size={15} />
      </span>
      <div className="min-w-0 grow">
        <div className="mono text-body-sm text-tx">{scope}</div>
        <div className="mt-0.5 text-small text-tx3">{description}</div>
      </div>
      {granted ? (
        <Badge tone="green">accordée</Badge>
      ) : (
        <Badge tone={required ? "clay" : "neutral"}>{required ? "requis" : "optionnel"}</Badge>
      )}
    </div>
  );
}
