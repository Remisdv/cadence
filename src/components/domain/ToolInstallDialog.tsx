/**
 * Installation guidée d'un outil (maquette V2-Tool-Install.dc.html) : commande exacte, terminal
 * intégré en lecture seule avec le journal en direct, puis connexion (code d'appairage de
 * `gh auth login`) et nouvelle détection. Ouvert par un clic « Installer » ou « Se connecter » :
 * ce clic vaut lancement, rien ne démarre autrement.
 */
import { useEffect, useRef, useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import type { ToolInfo, ToolRecipe } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { StatusDot } from "@/components/ui/StatusDot";
import { CommandBox } from "./CommandBox";
import {
  formatRunDuration,
  loginUrl,
  pairingCode,
  preferredRecipe,
  recipePackage,
  requestSentence,
  runProgress,
  toolIcons,
  type ToolRunLog,
} from "@/lib/domain/tools";
import { copyText, openExternal } from "@/lib/platform";
import {
  useCancelTool,
  useInstallTool,
  useLoginTool,
  useRecheckTools,
  useToolInput,
  useToolRun,
} from "@/lib/query/queries";

type Phase = "install" | "login" | "check" | "done";

export interface ToolInstallDialogProps {
  tool: ToolInfo;
  /** `login` : l'outil est déjà installé, seule la connexion reste. */
  start: "install" | "login";
  recipe?: ToolRecipe | null;
  platform: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const isWindows = typeof navigator !== "undefined" && /Windows/.test(navigator.userAgent);

export function ToolInstallDialog({
  tool,
  start,
  recipe: chosen,
  platform,
  open,
  onOpenChange,
}: ToolInstallDialogProps) {
  const recipe = chosen ?? preferredRecipe(tool);
  const install = useInstallTool();
  const login = useLoginTool();
  const cancel = useCancelTool();
  const recheck = useRecheckTools();
  const log = useToolRun(tool.id).data ?? null;
  const [phase, setPhase] = useState<Phase>(start);
  const [error, setError] = useState<string | null>(null);
  const [installInfo, setInstallInfo] = useState<string | null>(null);
  const launched = useRef(false);

  const withLogin = tool.login !== null;
  const steps = withLogin ? 3 : 2;
  const stepIndex = phase === "install" ? 1 : phase === "login" ? 2 : steps;

  const runLogin = async () => {
    setPhase("login");
    setError(null);
    try {
      const result = await login.mutateAsync({ tool: tool.id, command: tool.login ?? "" });
      if (!result.ok && result.message !== null && !tool.loginInteractive) {
        setError(result.message);
        return;
      }
      if (tool.loginInteractive) {
        setError(null);
        setPhase("check");
        return;
      }
      await check();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const check = async () => {
    setPhase("check");
    await recheck.mutateAsync().catch(() => undefined);
    setPhase("done");
  };

  const runInstall = async () => {
    if (!recipe) return;
    setPhase("install");
    setError(null);
    try {
      const result = await install.mutateAsync({
        tool: tool.id,
        recipe: recipe.id,
        command: recipe.command,
      });
      if (!result.ok) {
        setError(result.message ?? "L'installation a échoué.");
        return;
      }
      setInstallInfo(`${recipe.id} · ${formatRunDuration(result.durationMs)}`);
      if (withLogin) await runLogin();
      else await check();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Le clic qui ouvre la boîte lance la première étape (une seule fois par ouverture).
  useEffect(() => {
    if (!open) {
      launched.current = false;
      return;
    }
    if (launched.current) return;
    launched.current = true;
    setPhase(start);
    setError(null);
    setInstallInfo(null);
    if (start === "install") void runInstall();
    else void runLogin();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const running = install.isPending || login.isPending || recheck.isPending;
  const close = () => {
    if (running) cancel.mutate(tool.id);
    onOpenChange(false);
  };

  const command =
    phase === "install" ? (recipe?.command ?? "") : phase === "login" ? (tool.login ?? "") : "";
  const sentence = requestSentence(tool);
  const binary = tool.login?.split(" ")[0] ?? tool.id;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => (next ? onOpenChange(true) : close())}
      width={720}
      aria-describedby={undefined}
    >
      <div className="flex h-[60px] shrink-0 items-center gap-3 border-b border-line pr-3.5 pl-[18px]">
        <span
          aria-hidden
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-field bg-p4 text-tx"
        >
          <Icon name={toolIcons[tool.id]} size={17} />
        </span>
        <div className="min-w-0 grow">
          <DialogPrimitive.Title className="font-display text-title font-semibold">
            {start === "install" ? `Installer ${tool.name}` : `Connecter ${tool.name}`}
          </DialogPrimitive.Title>
          {sentence && (
            <div className="mt-0.5 truncate text-caption text-tx3" title={tool.description}>
              {sentence}
            </div>
          )}
        </div>
        {phase !== "done" && (
          <Badge tone="clay">
            étape {stepIndex} sur {steps}
          </Badge>
        )}
        <button
          type="button"
          aria-label="Fermer"
          onClick={close}
          className="inline-flex size-[30px] items-center justify-center rounded-control border border-line text-tx3 hover:text-tx"
        >
          <Icon name="close" size={15} />
        </button>
      </div>

      <div className="flex h-[376px] gap-4 px-[18px] py-3.5">
        <div className="flex min-w-0 grow flex-col gap-2.5">
          {command ? (
            <>
              <div className="flex items-center gap-2">
                <SectionLabel className="grow">Commande lancée par Cadence</SectionLabel>
                <span className="text-tiny text-tx3">
                  {phase === "install" && recipe ? `${recipe.label} · ${platform}` : "connexion"}
                </span>
              </div>
              <CommandBox command={command} />
            </>
          ) : null}
          <Terminal log={log} phase={phase} prompt={isWindows ? "PS>" : "$"} tool={tool.id} />
          <PhaseProgress log={log} phase={phase} error={error} tool={tool} />
        </div>

        <div className="flex w-[260px] shrink-0 flex-col">
          <SectionLabel className="mb-1">Étapes</SectionLabel>
          {start === "install" && (
            <Step
              state={phase === "install" ? (error ? "failed" : "current") : "done"}
              title={`Installer ${tool.name}`}
              detail={
                installInfo ?? (recipe ? `${recipe.id} · ${recipePackage(recipe.command)}` : "")
              }
            />
          )}
          {start === "login" && (
            <Step state="done" title={`${tool.name} installé`} detail={tool.version ?? ""} />
          )}
          {withLogin && (
            <Step
              state={
                phase === "login"
                  ? error
                    ? "failed"
                    : "current"
                  : phase === "install"
                    ? "pending"
                    : "done"
              }
              title={tool.id === "gh" ? "Se connecter à GitHub" : "Se connecter"}
              detail={tool.login ?? ""}
            />
          )}
          <Step
            state={phase === "check" ? "current" : phase === "done" ? "done" : "pending"}
            title="Revérifier"
            detail={`${binary} --version${tool.login ? " · état du compte" : ""}`}
          />
          <SidePanel
            tool={tool}
            phase={phase}
            log={log}
            error={error}
            onRetry={phase === "login" ? runLogin : runInstall}
            onCheck={check}
          />
        </div>
      </div>

      <div className="flex h-[52px] shrink-0 items-center gap-2 border-t border-line px-[18px] text-caption text-tx3">
        <Icon name="shield" size={14} />
        <span className="min-w-0 grow">
          Cadence ne voit ni ne stocke vos identifiants :{" "}
          <span className="mono text-tx2">{binary}</span> les garde dans son propre stockage.
        </span>
        {phase === "done" ? (
          <Button variant="primary" onClick={() => onOpenChange(false)}>
            Terminé
          </Button>
        ) : (
          <Button variant="ghost" onClick={close}>
            Annuler
          </Button>
        )}
      </div>
    </Dialog>
  );
}

function Terminal({
  log,
  phase,
  prompt,
  tool,
}: {
  log: ToolRunLog | null;
  phase: Phase;
  prompt: string;
  tool: ToolInfo["id"];
}) {
  const input = useToolInput();
  const [answer, setAnswer] = useState("");
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Corps explicite : `scrollIntoView` renvoie une promesse dans les navigateurs récents.
    end.current?.scrollIntoView({ block: "end" });
  }, [log?.lines.length]);
  const running = log?.running ?? false;
  return (
    <div className="flex min-h-0 grow flex-col overflow-hidden rounded-field border border-line bg-console">
      <div className="flex h-7 shrink-0 items-center gap-[7px] border-b border-line px-2.5 text-tiny text-tx2">
        <span className="flex text-tx3">
          <Icon name="terminal" size={13} />
        </span>
        Terminal intégré
        <Badge>{log?.running ? "répond aux invites" : "lecture seule"}</Badge>
        <div className="grow" />
        <span className="inline-flex items-center gap-1.5 text-tx3">
          <StatusDot
            tone={
              running
                ? "clay"
                : log?.result?.ok === false
                  ? "red"
                  : log?.result
                    ? "green"
                    : "neutral"
            }
          />
          {running
            ? phase === "login"
              ? (log?.command.split(" ").slice(0, 3).join(" ") ?? "connexion")
              : "en cours"
            : log?.result
              ? log.result.ok
                ? "terminé"
                : "échec"
              : "prêt"}
        </span>
      </div>
      <div
        role="log"
        aria-label="Journal de l'installation"
        className="flex min-h-0 grow flex-col overflow-y-auto px-3 py-2 mono text-tiny leading-[1.6]"
      >
        <div className="grow" />
        {log?.previous && (
          <>
            <div className="truncate text-tx2">
              <span className="text-tx3">{prompt}</span>{" "}
              <span className="text-tx">{log.previous.command}</span>
            </div>
            {log.previous.lines.map((line, i) => (
              <div key={`p${i}`} className={cn("break-all whitespace-pre-wrap", lineTone(line))}>
                {line}
              </div>
            ))}
          </>
        )}
        {log?.command && (
          <div className="truncate text-tx2">
            <span className="text-tx3">{prompt}</span>{" "}
            <span className="text-tx">{log.command}</span>
          </div>
        )}
        {log?.lines.map((line, i) => (
          <div key={i} className={cn("break-all whitespace-pre-wrap", lineTone(line))}>
            {line}
          </div>
        ))}
        {running && (
          <div>
            <span className="inline-block h-[13px] w-[7px] bg-tx2 align-[-2px]" />
          </div>
        )}
        <div ref={end} />
      </div>
      {running && (
        <form
          className="flex h-8 shrink-0 items-center gap-2 border-t border-line px-2.5"
          onSubmit={(e) => {
            e.preventDefault();
            input.mutate({ tool, text: answer }, { onSuccess: (sent) => sent && setAnswer("") });
          }}
        >
          <span className="mono text-tiny text-tx3">›</span>
          <input
            aria-label="Réponse à la commande"
            placeholder="Répondre à une invite (Entrée pour envoyer)…"
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            className="min-w-0 grow bg-transparent mono text-tiny text-tx outline-none placeholder:text-tx3"
          />
          {input.data === false && (
            <span className="text-tiny text-amber">saisie impossible pour cette commande</span>
          )}
        </form>
      )}
    </div>
  );
}

function lineTone(line: string): string {
  if (/^\s*[!✗×]|error|erreur|échec|failed/i.test(line)) return "text-amber";
  if (/^\s*✓|succès|success|installé correctement|logged in/i.test(line)) return "text-green";
  return "text-tx2";
}

function PhaseProgress({
  log,
  phase,
  error,
  tool,
}: {
  log: ToolRunLog | null;
  phase: Phase;
  error: string | null;
  tool: ToolInfo;
}) {
  if (error) {
    return (
      <div className="flex items-center gap-2 text-caption text-red">
        <Icon name="warning" size={14} />
        <span className="min-w-0 grow truncate">{error}</span>
      </div>
    );
  }
  if (phase === "done") {
    return (
      <div>
        <div className="mb-[7px] flex items-center gap-2 text-caption text-tx2">
          <span className="grow">
            {tool.login ? "Installé et connecté" : "Installation terminée"}
          </span>
          {log?.result && (
            <span className="mono text-tiny text-tx3">
              {formatRunDuration(log.result.durationMs)}
            </span>
          )}
        </div>
        <ProgressBar value={100} tone="green" />
      </div>
    );
  }
  const value = phase === "install" ? runProgress(log?.lines ?? []) : null;
  const label =
    phase === "install"
      ? "Téléchargement et installation"
      : phase === "login"
        ? "Connexion en cours"
        : "Nouvelle détection";
  return (
    <div>
      <div className="mb-[7px] flex items-center gap-2 text-caption text-tx2">
        <span className="grow">
          {label}
          {value !== null ? ` · ${value} %` : "…"}
        </span>
      </div>
      <ProgressBar value={value ?? 12} className={value === null ? "animate-pulse" : undefined} />
    </div>
  );
}

type StepState = "done" | "current" | "pending" | "failed";

function Step({ state, title, detail }: { state: StepState; title: string; detail: string }) {
  return (
    <div className="flex w-full gap-2.5 py-[7px]">
      {state === "done" ? (
        <span className="inline-flex size-5 shrink-0 items-center justify-center rounded-full bg-green-d text-green">
          <Icon name="check" size={12} />
        </span>
      ) : state === "current" ? (
        <span className="inline-flex size-5 shrink-0 items-center justify-center rounded-full border border-clay bg-clay-d">
          <span className="size-[7px] rounded-full bg-clay" />
        </span>
      ) : state === "failed" ? (
        <span className="inline-flex size-5 shrink-0 items-center justify-center rounded-full bg-red-d text-red">
          <Icon name="close" size={12} />
        </span>
      ) : (
        <span className="inline-flex size-5 shrink-0 rounded-full border border-line2" />
      )}
      <span className="block min-w-0 grow">
        <span
          className={cn(
            "block text-body-sm",
            state === "current"
              ? "font-medium text-tx"
              : state === "pending"
                ? "text-tx3"
                : "text-tx2",
          )}
        >
          {title}
        </span>
        {detail && (
          <span className="mt-[3px] block truncate mono text-tiny text-tx3">{detail}</span>
        )}
      </span>
    </div>
  );
}

function SidePanel({
  tool,
  phase,
  log,
  error,
  onRetry,
  onCheck,
}: {
  tool: ToolInfo;
  phase: Phase;
  log: ToolRunLog | null;
  error: string | null;
  onRetry: () => void;
  onCheck: () => void;
}) {
  const [copied, setCopied] = useState(false);
  if (error) {
    return (
      <div className="mt-1.5 flex flex-col gap-2 rounded-tile border border-line bg-p2 p-3">
        <div className="text-caption leading-[1.6] text-tx3">
          La commande n'a pas abouti. Relancez-la, ou installez l'outil vous-même puis revérifiez.
        </div>
        <div className="flex gap-1.5">
          <Button variant="primary" size="compact" icon="refresh" onClick={onRetry}>
            Relancer
          </Button>
          <Button variant="ghost" size="compact" onClick={onCheck}>
            Revérifier
          </Button>
        </div>
      </div>
    );
  }
  if (phase === "login" && tool.loginInteractive) {
    return (
      <div className="mt-1.5 rounded-tile border border-clay-line bg-p2 p-3 text-caption leading-[1.6] text-tx3">
        Un terminal s'est ouvert avec <span className="mono text-tx2">{tool.login}</span> :
        choisissez votre fournisseur, puis revenez revérifier.
      </div>
    );
  }
  if (phase === "check" && tool.loginInteractive) {
    return (
      <div className="mt-1.5 flex flex-col gap-2 rounded-tile border border-clay-line bg-p2 p-3">
        <div className="text-caption leading-[1.6] text-tx3">
          Terminez la connexion dans le terminal ouvert par Cadence, puis revérifiez.
        </div>
        <Button variant="primary" size="compact" icon="refresh" onClick={onCheck}>
          Revérifier
        </Button>
      </div>
    );
  }
  const code = phase === "login" ? pairingCode(log?.lines ?? []) : null;
  const url = phase === "login" ? loginUrl(log?.lines ?? []) : null;
  if (code || url) {
    return (
      <div className="mt-1.5 rounded-tile border border-clay-line bg-p2 p-3">
        {code && (
          <>
            <SectionLabel>Code d'appairage</SectionLabel>
            <div className="mt-2 flex items-center gap-2">
              <span className="grow mono text-[22px] font-semibold tracking-[0.08em] text-tx">
                {code}
              </span>
              <button
                type="button"
                aria-label="Copier le code"
                onClick={async () => setCopied(await copyText(code))}
                className="inline-flex size-7 items-center justify-center rounded-control border border-line text-tx3 hover:text-tx"
              >
                <Icon name={copied ? "check" : "copy"} size={14} />
              </button>
            </div>
          </>
        )}
        {url && (
          <div className="mt-2.5 flex">
            <Button
              variant="primary"
              size="compact"
              icon="external"
              className="grow"
              onClick={() => void openExternal(url)}
            >
              Ouvrir {url.replace(/^https:\/\//, "")}
            </Button>
          </div>
        )}
        <div className="mt-[9px] flex items-center gap-[7px] text-caption text-tx3">
          <StatusDot tone="clay" pulse />
          En attente de la validation…
        </div>
      </div>
    );
  }
  if (phase === "install" && tool.login) {
    return (
      <div className="mt-1.5 rounded-tile border border-line bg-p2 p-3">
        <SectionLabel>Ensuite</SectionLabel>
        <div className="mt-[7px] text-caption leading-[1.6] text-tx3">
          Cadence lancera <span className="mono text-tx2">{tool.login}</span>
          {tool.id === "gh"
            ? ". Le code d'appairage s'affichera ici, puis github.com/login/device s'ouvrira dans votre navigateur."
            : tool.loginInteractive
              ? " dans un terminal du système."
              : ", qui ouvre votre navigateur pour valider la connexion."}
        </div>
      </div>
    );
  }
  return null;
}
