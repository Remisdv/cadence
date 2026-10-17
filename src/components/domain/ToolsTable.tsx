/**
 * Tableau des outils de la machine (maquette V2-Onboarding-Tools.dc.html, reprise dans
 * Réglages › Outils) : demandeur, état, version, et pour un outil manquant la commande exacte
 * que Cadence lancera, avec « Installer moi-même » et le lien officiel.
 */
import { useState } from "react";
import type { InstalledPluginView, ToolInfo, ToolRequester } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { CommandBox } from "./CommandBox";
import { ToolInstallDialog } from "./ToolInstallDialog";
import { aiTileClass } from "./AiCard";
import { kitIcon, pluginTileClass } from "@/lib/domain/plugins";
import { aiOfTool, preferredRecipe, requestLabel, toolIcons, toolStatus } from "@/lib/domain/tools";
import { copyText, openExternal } from "@/lib/platform";
import { useAppSettings, useSaveAppSettings } from "@/lib/query/queries";

/** Colonnes de la maquette (1440px), resserrées à la largeur minimale de la fenêtre. */
const cols = {
  requested: "w-[210px] shrink-0 max-[1320px]:w-[132px]",
  status: "w-[130px] shrink-0 max-[1320px]:w-[112px]",
  version: "w-[70px] shrink-0 max-[1320px]:w-[58px]",
  actions: "w-[186px] shrink-0 max-[1320px]:w-[164px]",
};

export interface ToolsTableProps {
  tools: ToolInfo[];
  plugins: InstalledPluginView[];
  platform: string;
  /** Réglages : affiche aussi la colonne de chemin des outils non demandés. */
  showUnrequested?: boolean;
}

export function ToolsTable({ tools, plugins, platform }: ToolsTableProps) {
  const settings = useAppSettings().data;
  const save = useSaveAppSettings();
  const [dialog, setDialog] = useState<{ tool: ToolInfo; start: "install" | "login" } | null>(null);

  const setIgnored = (tool: ToolInfo, ignored: boolean) => {
    if (!settings) return;
    const ai = aiOfTool(tool.id);
    if (ai) save.mutate({ ai: [{ ai, enabled: !ignored }] });
    else
      save.mutate({
        ignoredTools: ignored
          ? [...settings.ignoredTools, tool.id]
          : settings.ignoredTools.filter((t) => t !== tool.id),
      });
  };

  return (
    <div className="overflow-hidden rounded-card border border-line bg-panel">
      <div className="flex h-8 items-center gap-3 border-b border-line px-3.5 text-tiny font-semibold tracking-[0.06em] text-tx3 uppercase">
        <span className="w-7 shrink-0" />
        <span className="grow">Outil</span>
        <span className={cols.requested}>Demandé par</span>
        <span className={cols.status}>État</span>
        <span className={cols.version}>Version</span>
        <span className={cols.actions} />
      </div>
      {tools.map((tool) => (
        <ToolRow
          key={tool.id}
          tool={tool}
          plugins={plugins}
          platform={platform}
          onInstall={() => setDialog({ tool, start: "install" })}
          onLogin={() => setDialog({ tool, start: "login" })}
          onIgnore={(ignored) => setIgnored(tool, ignored)}
        />
      ))}
      {dialog && (
        <ToolInstallDialog
          tool={tools.find((t) => t.id === dialog.tool.id) ?? dialog.tool}
          start={dialog.start}
          platform={platform}
          open
          onOpenChange={(open) => !open && setDialog(null)}
        />
      )}
    </div>
  );
}

function ToolRow({
  tool,
  plugins,
  platform,
  onInstall,
  onLogin,
  onIgnore,
}: {
  tool: ToolInfo;
  plugins: InstalledPluginView[];
  platform: string;
  onInstall: () => void;
  onLogin: () => void;
  onIgnore: (ignored: boolean) => void;
}) {
  const status = toolStatus(tool);
  const ai = aiOfTool(tool.id);
  const missing = tool.status === "missing" || tool.status === "outdated";
  const recipe = preferredRecipe(tool);
  const blockedByNode = recipe?.needs === "node" && !recipe.available;
  const [copied, setCopied] = useState(false);
  const request = requestLabel(tool);
  const subtitle =
    ai && tool.account
      ? tool.account.split(" · ")[0]
      : tool.status === "not-connected" && tool.login
        ? `À connecter avec ${tool.login.split(" ").slice(0, 2).join(" ")}`
        : tool.description;

  return (
    <div className={cn("border-b border-line last:border-b-0", tool.ignored && "opacity-60")}>
      <div className="flex h-[58px] items-center gap-3 px-3.5">
        <span
          aria-hidden
          className={cn(
            "inline-flex size-7 shrink-0 items-center justify-center rounded-control",
            ai && !missing ? aiTileClass[ai] : "bg-p4 text-tx",
          )}
        >
          <Icon name={toolIcons[tool.id]} size={15} />
        </span>
        <div className="min-w-0 grow">
          <div className="text-body font-semibold">{tool.name}</div>
          <div className="mt-0.5 truncate text-caption text-tx3" title={tool.path ?? undefined}>
            {subtitle}
          </div>
        </div>
        <div className={cn("flex items-center gap-1.5", cols.requested)}>
          {tool.requestedBy[0] ? (
            <>
              <RequesterChip requester={tool.requestedBy[0]} plugins={plugins} />
              <Badge tone={request.tone}>{request.label}</Badge>
            </>
          ) : (
            <span className="text-caption text-tx3">—</span>
          )}
        </div>
        <div className={cn("flex", cols.status)}>
          {tool.ignored ? (
            <Badge>Ignoré</Badge>
          ) : (
            <Badge
              tone={status.tone}
              icon={tool.status === "ready" ? "check" : missing ? "warning" : undefined}
            >
              {status.label}
            </Badge>
          )}
        </div>
        <div className={cols.version}>
          <span className="block mono text-small text-tx2">{tool.version ?? "—"}</span>
          {tool.version && tool.requestedBy.length > 0 && tool.id !== "node" && !ai && (
            <span className="mt-0.5 block mono text-micro text-tx3">
              min. {tool.minVersion.replace(/\.0$/, "")}
            </span>
          )}
        </div>
        <div className={cn("flex justify-end gap-1.5", cols.actions)}>
          {tool.ignored ? (
            <Button variant="quiet" size="compact" onClick={() => onIgnore(false)}>
              Rétablir
            </Button>
          ) : missing ? (
            <>
              <Button
                variant="primary"
                size="compact"
                icon="download"
                disabled={!recipe || blockedByNode}
                title={blockedByNode ? "Installez d'abord Node.js" : undefined}
                onClick={onInstall}
              >
                {tool.status === "outdated" ? "Mettre à jour" : "Installer"}
              </Button>
              {!tool.required && (
                <Button variant="quiet" size="compact" onClick={() => onIgnore(true)}>
                  Ignorer
                </Button>
              )}
            </>
          ) : tool.status === "not-connected" ? (
            <>
              <Button size="compact" icon="user" onClick={onLogin}>
                Se connecter
              </Button>
              {!tool.required && (
                <Button variant="quiet" size="compact" onClick={() => onIgnore(true)}>
                  Ignorer
                </Button>
              )}
            </>
          ) : null}
        </div>
      </div>

      {missing && !tool.ignored && recipe && (
        <div className="mr-3.5 mb-3.5 ml-[54px] rounded-tile border border-line bg-p2 px-3.5 py-3">
          <div className="flex items-center gap-2">
            <SectionLabel className="grow">Commande que Cadence lancera</SectionLabel>
            <span className="text-tiny text-tx3">
              {recipe.label} · {platform}
            </span>
          </div>
          <div className="mt-2 flex items-center gap-2">
            <div className="min-w-0 grow">
              <CommandBox command={recipe.command} />
            </div>
            <Button
              variant="ghost"
              size="compact"
              icon={copied ? "check" : "copy"}
              onClick={async () => setCopied(await copyText(recipe.command))}
            >
              Installer moi-même
            </Button>
            <button
              type="button"
              onClick={() => void openExternal(tool.homepage)}
              className="inline-flex items-center gap-1.5 text-small whitespace-nowrap text-clay hover:text-clay-hover"
            >
              {tool.homepage.replace(/^https:\/\//, "").replace(/\/.*$/, "")}
              <Icon name="external" size={13} />
            </button>
          </div>
          <div className="mt-2 text-caption leading-[1.55] text-tx3">
            {blockedByNode
              ? "Cette recette passe par npm : installez d'abord Node.js (ligne plus bas), puis revenez ici."
              : `Lancée dans un terminal intégré en lecture seule, avec le journal en direct${
                  recipe.elevated
                    ? platform.startsWith("Windows")
                      ? " ; Windows peut demander une élévation (UAC)"
                      : " ; le système demandera votre mot de passe"
                    : ""
                }.`}
            {tool.login && !blockedByNode && (
              <>
                {" "}
                Ensuite, connexion avec{" "}
                <span className="mono text-tx2">
                  {tool.login.split(" ").slice(0, 3).join(" ")}
                </span>{" "}
                et nouvelle détection.
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function RequesterChip({
  requester,
  plugins,
}: {
  requester: ToolRequester;
  plugins: InstalledPluginView[];
}) {
  const plugin = requester.kind === "plugin" ? plugins.find((p) => p.id === requester.id) : null;
  const tile =
    requester.kind === "service"
      ? { cls: "bg-clay text-clay-ink", icon: "github" as const }
      : requester.kind === "ai"
        ? { cls: "bg-clay text-ink", icon: "sparkle" as const }
        : requester.kind === "tool"
          ? {
              cls: aiOfTool(requester.id as ToolInfo["id"])
                ? aiTileClass[aiOfTool(requester.id as ToolInfo["id"])!]
                : "bg-p4 text-tx",
              icon: toolIcons[requester.id as ToolInfo["id"]] ?? "package",
            }
          : {
              cls: plugin ? `${pluginTileClass[plugin.color]} text-ink` : "bg-p4 text-tx",
              icon: kitIcon(plugin?.railIcon ?? "layers"),
            };
  return (
    <span className="inline-flex h-[21px] min-w-0 items-center gap-1.5 rounded-badge bg-p3 pr-2 pl-1 text-caption font-medium whitespace-nowrap text-tx2">
      <span
        aria-hidden
        className={cn(
          "inline-flex size-3.5 shrink-0 items-center justify-center rounded-badge",
          tile.cls,
        )}
      >
        <Icon name={tile.icon} size={9} />
      </span>
      <span className="truncate">{requester.name}</span>
    </span>
  );
}
