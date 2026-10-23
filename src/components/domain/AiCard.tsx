/**
 * Carte d'une IA de code (maquette V2-Onboarding-AI.dc.html, reprise dans Réglages › IA) :
 * binaire, version, compte, session de test, choix de l'IA par défaut, et selon l'état
 * « Relancer le test », « Se connecter » ou « Installer » / « Localiser le binaire… ».
 */
import { useState } from "react";
import type { AiId, ToolInfo } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { CommandBox } from "./CommandBox";
import { ToolInstallDialog } from "./ToolInstallDialog";
import { aiLabels } from "@/lib/domain/ai";
import { aiOfTool, preferredRecipe, toolIcons, toolStatus } from "@/lib/domain/tools";
import { pickFile } from "@/lib/platform";
import { useSetToolPath, useTestAi } from "@/lib/query/queries";

/** Fond de la tuile, comme dans la maquette (Bienvenue, IA de code). */
export const aiTileClass: Record<AiId, string> = {
  "claude-code": "bg-clay text-ink",
  codex: "bg-tx text-ink",
  opencode: "bg-purple text-ink",
};

const vendors: Record<AiId, string> = {
  "claude-code": "Anthropic",
  codex: "OpenAI",
  opencode: "Open source",
};

/** `C:\Users\moi\.local\bin\claude.exe` → `~\.local\bin\claude.exe`. */
export function shortPath(path: string): string {
  return path
    .replace(/^[A-Za-z]:\\Users\\[^\\]+\\AppData\\Roaming\\/i, "%APPDATA%\\")
    .replace(/^[A-Za-z]:\\Users\\[^\\]+/i, "~")
    .replace(/^\/(?:Users|home)\/[^/]+/, "~");
}

export interface AiCardProps {
  tool: ToolInfo;
  /** Node.js détecté : précisé quand la recette passe par npm. */
  node?: ToolInfo;
  platform: string;
  isDefault: boolean;
  onSetDefault: () => void;
}

export function AiCard({ tool, node, platform, isDefault, onSetDefault }: AiCardProps) {
  const ai = aiOfTool(tool.id)!;
  const test = useTestAi();
  const setPath = useSetToolPath();
  const [dialog, setDialog] = useState<"install" | "login" | null>(null);
  const status = toolStatus(tool);
  const missing = tool.status === "missing";
  const recipe = preferredRecipe(tool);
  const lastTest = test.data ?? tool.test;

  const locate = async () => {
    const path = await pickFile(`Emplacement de ${tool.name}`);
    if (path) setPath.mutate({ tool: tool.id, path });
  };

  return (
    <div className={cn("h-full rounded-card border border-line bg-panel", missing && "opacity-80")}>
      <div className="flex h-full flex-col p-5">
        <div className="flex items-center gap-[11px]">
          <span
            aria-hidden
            className={cn(
              "inline-flex size-[34px] shrink-0 items-center justify-center rounded-field",
              missing ? "bg-p4 text-tx3" : aiTileClass[ai],
            )}
          >
            <Icon name={toolIcons[tool.id]} size={18} />
          </span>
          <div className="min-w-0 grow">
            <div className="truncate text-title-sm font-semibold">{aiLabels[ai].name}</div>
            <div className="mt-px truncate text-caption text-tx3">{vendors[ai]}</div>
          </div>
          <Badge tone={status.tone}>{status.label}</Badge>
        </div>

        <div className="mt-4 grow">
          {missing ? (
            <>
              <div className="text-body-sm leading-[1.65] text-tx3">
                Aucun binaire <span className="mono text-tx2">{tool.id}</span> trouvé dans le PATH.{" "}
                {recipe?.needs === "node"
                  ? node?.version
                    ? `Cadence peut l'installer avec npm (Node.js ${node.version} détecté), ou vous indiquez son emplacement.`
                    : "Son installation passe par npm : installez d'abord Node.js, ou indiquez son emplacement."
                  : "Cadence peut l'installer avec l'installeur officiel, ou vous indiquez son emplacement."}
              </div>
              {recipe && (
                <>
                  <SectionLabel className="mt-3">Commande lancée par Cadence</SectionLabel>
                  <CommandBox command={recipe.command} className="mt-[7px]" />
                </>
              )}
            </>
          ) : (
            <>
              <Row
                label="Binaire"
                value={tool.path ? shortPath(tool.path) : "—"}
                title={tool.path}
              />
              <Row label="Version" value={tool.version ?? "—"} />
              <Row
                label="Compte"
                value={
                  tool.status === "not-connected"
                    ? "non connecté"
                    : (tool.account?.split(" · ")[0] ??
                      (tool.status === "ready" ? "connecté" : "—"))
                }
                title={tool.account}
              />
              <Row
                label="Session de test"
                last
                value={
                  test.isPending
                    ? "en cours…"
                    : lastTest
                      ? lastTest.ok
                        ? `OK · ${(lastTest.latencyMs / 1000).toFixed(1).replace(".", ",")} s`
                        : "échec"
                      : "—"
                }
                title={lastTest?.message}
                tone={lastTest && !lastTest.ok ? "text-red" : undefined}
              />
            </>
          )}
        </div>

        {!missing && (
          <label
            className={cn(
              "mt-3.5 flex cursor-pointer items-center gap-[9px] rounded-field border px-3 py-2.5",
              isDefault ? "border-clay-line bg-clay-d" : "border-line bg-p2",
            )}
          >
            <input
              type="radio"
              name="ia-defaut"
              checked={isDefault}
              onChange={onSetDefault}
              className="m-0 size-[15px] accent-clay"
            />
            <span className={cn("text-body-sm font-medium", isDefault ? "text-clay" : "text-tx2")}>
              IA par défaut
            </span>
          </label>
        )}

        <div className="mt-3.5 flex flex-wrap gap-2">
          {missing ? (
            <>
              <Button
                size="compact"
                icon="download"
                disabled={!recipe || (recipe.needs === "node" && !node?.path)}
                onClick={() => setDialog("install")}
              >
                Installer
              </Button>
              <Button variant="ghost" size="compact" onClick={locate}>
                Localiser le binaire…
              </Button>
            </>
          ) : tool.status === "not-connected" ? (
            <Button icon="user" onClick={() => setDialog("login")}>
              Se connecter
            </Button>
          ) : (
            <Button
              icon={test.isPending ? undefined : "refresh"}
              disabled={test.isPending}
              onClick={() => test.mutate(ai)}
            >
              {test.isPending && <Spinner />}
              {lastTest ? "Relancer le test" : "Lancer le test"}
            </Button>
          )}
        </div>
        {(setPath.error || (test.error && !lastTest)) && (
          <p className="mt-2 text-caption text-red">{(setPath.error ?? test.error)?.message}</p>
        )}
      </div>
      {dialog && (
        <ToolInstallDialog
          tool={tool}
          start={dialog}
          platform={platform}
          open
          onOpenChange={(open) => !open && setDialog(null)}
        />
      )}
    </div>
  );
}

function Row({
  label,
  value,
  title,
  last,
  tone,
}: {
  label: string;
  value: string;
  title?: string | null;
  last?: boolean;
  tone?: string;
}) {
  return (
    <div
      className={cn(
        "flex justify-between gap-3 py-[7px] text-body-sm",
        !last && "border-b border-line",
      )}
    >
      <span className="shrink-0 text-tx3">{label}</span>
      <span
        className={cn("truncate text-right mono", tone ?? "text-tx2")}
        title={title ?? undefined}
      >
        {value}
      </span>
    </div>
  );
}
