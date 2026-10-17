/**
 * Réglages › IA de code (V2-Settings.dc.html) : IA par défaut, puis IA réaffectée action par
 * action (`actionAi`, clé `<plugin>/<action>`). Le mode (chat ou en fond) vient du manifeste.
 */
import type { AiId, InstalledPluginView } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Callout } from "@/components/ui/Callout";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Select } from "@/components/ui/Field";
import { aiTileClass } from "@/components/domain/AiCard";
import { PluginTile } from "@/components/domain/PluginTile";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { aiLabels, aiOrder } from "@/lib/domain/ai";
import { toolIcons, toolOfAi, toolStatus } from "@/lib/domain/tools";
import { useAppSettings, usePlugins, useSaveAppSettings, useTools } from "@/lib/query/queries";
import { SectionTitle } from "./SectionTitle";

export function AiSettings() {
  const settings = useAppSettings();
  const tools = useTools();
  const plugins = usePlugins();
  const save = useSaveAppSettings();

  if (settings.isPending || tools.isPending) return <LoadingState />;
  if (settings.isError) return <ErrorState error={settings.error} />;
  if (tools.isError) return <ErrorState error={tools.error} />;

  const s = settings.data;
  const byAi = (ai: AiId) => tools.data.tools.find((t) => t.id === toolOfAi[ai])!;
  const installed = aiOrder.filter((ai) => byAi(ai).status !== "missing" && !byAi(ai).ignored);
  const defaultName = s.defaultAi ? aiLabels[s.defaultAi].name : "à choisir";
  const enabled = (plugins.data ?? []).filter((p) => p.enabled);
  const actions = enabled.flatMap((plugin) => plugin.actions.map((action) => ({ plugin, action })));

  return (
    <div className="max-w-[900px]">
      <SectionTitle title="IA de code">
        Cadence pilote les CLI installées avec vos abonnements. L'IA par défaut sert au chat et à
        toutes les actions, sauf celles que vous réaffectez ci-dessous.
      </SectionTitle>

      <SectionLabel className="mt-4">IA par défaut</SectionLabel>
      <div className="mt-[9px] grid grid-cols-3 gap-2.5">
        {aiOrder.map((ai) => {
          const tool = byAi(ai);
          const missing = tool.status === "missing";
          const status = tool.ignored
            ? { label: "Ignorée", tone: "neutral" as const }
            : toolStatus(tool);
          const checked = s.defaultAi === ai;
          return (
            <label
              key={ai}
              className={cn(
                "flex items-center gap-[11px] rounded-[11px] border px-3.5 py-[13px]",
                checked ? "border-clay-line bg-clay-d" : "border-line bg-panel",
                missing ? "cursor-not-allowed opacity-70" : "cursor-pointer",
              )}
            >
              <input
                type="radio"
                name="set-ai"
                checked={checked}
                disabled={missing || save.isPending}
                onChange={() => save.mutate({ defaultAi: ai })}
                className="m-0 size-[15px] accent-clay"
              />
              <span
                aria-hidden
                className={cn(
                  "inline-flex size-7 shrink-0 items-center justify-center rounded-control",
                  missing ? "bg-p4 text-tx3" : aiTileClass[ai],
                )}
              >
                <Icon name={toolIcons[tool.id]} size={15} />
              </span>
              <span className="min-w-0 grow">
                <span className="block truncate text-body font-semibold">{aiLabels[ai].name}</span>
                <span className="mt-0.5 block truncate mono text-tiny text-tx3">
                  {missing
                    ? "absent du PATH"
                    : [tool.version, tool.account?.split(" · ")[0]].filter(Boolean).join(" · ")}
                </span>
              </span>
              <Badge tone={status.tone}>{status.label}</Badge>
            </label>
          );
        })}
      </div>

      <div className="mt-[22px] flex items-center gap-2.5">
        <SectionLabel>Actions des plugins</SectionLabel>
        <div className="grow" />
        <span className="text-caption text-tx3">
          {actions.length} action{actions.length > 1 ? "s" : ""} · {enabled.length} plugin
          {enabled.length > 1 ? "s" : ""} activé{enabled.length > 1 ? "s" : ""}
        </span>
      </div>
      {actions.length === 0 ? (
        <div className="mt-[9px] rounded-card border border-dashed border-line2 px-4 py-5 text-center text-body-sm text-tx3">
          Aucun plugin activé ne déclare d'action pour l'instant.
        </div>
      ) : (
        <div className="mt-[9px] overflow-hidden rounded-card border border-line bg-panel">
          <div className="flex h-8 items-center gap-3 border-b border-line px-3.5 text-tiny font-semibold tracking-[0.06em] text-tx3 uppercase">
            <span className="w-3.5" />
            <span className="grow">Action</span>
            <span className="w-40 max-[1320px]:w-32">Plugin</span>
            <span className="w-[84px]">Mode</span>
            <span className="w-[220px] max-[1320px]:w-[180px]">IA</span>
          </div>
          {actions.map(({ plugin, action }) => (
            <ActionRow
              key={`${plugin.id}/${action.id}`}
              plugin={plugin}
              id={action.id}
              label={action.label}
              background={action.mode === "background"}
              value={s.actionAi[`${plugin.id}/${action.id}`] ?? null}
              defaultName={defaultName}
              choices={installed}
              disabled={save.isPending}
              onChange={(ai) => save.mutate({ actionAi: { [`${plugin.id}/${action.id}`]: ai } })}
            />
          ))}
        </div>
      )}
      <div className="mt-3 text-caption leading-[1.6] text-tx3">
        Le mode (chat ou en fond) est choisi par le plugin dans son manifeste.
      </div>
      {save.isError && (
        <Callout tone="danger" className="mt-3">
          {save.error.message}
        </Callout>
      )}
    </div>
  );
}

function ActionRow({
  plugin,
  id,
  label,
  background,
  value,
  defaultName,
  choices,
  disabled,
  onChange,
}: {
  plugin: InstalledPluginView;
  id: string;
  label: string;
  background: boolean;
  value: AiId | null;
  defaultName: string;
  choices: AiId[];
  disabled: boolean;
  onChange: (ai: AiId | null) => void;
}) {
  return (
    <div className="flex h-10 items-center gap-3 border-b border-line px-3.5 last:border-b-0">
      <span className="flex text-clay">
        <Icon name="sparkle" size={14} />
      </span>
      <span className="min-w-0 grow truncate text-body-sm">{label}</span>
      {value && <Badge tone="amber">modifié</Badge>}
      <span className="flex w-40 items-center gap-[7px] truncate text-small text-tx3 max-[1320px]:w-32">
        <PluginTile color={plugin.color} icon={plugin.railIcon} size={14} />
        <span className="truncate">{plugin.name}</span>
      </span>
      <span className="flex w-[84px]">
        <Badge tone={background ? "purple" : "blue"}>{background ? "en fond" : "chat"}</Badge>
      </span>
      <Select
        size="sm"
        aria-label={`IA pour ${label}`}
        value={value ?? ""}
        disabled={disabled}
        onChange={(e) => onChange((e.target.value || null) as AiId | null)}
        className="w-[220px] max-[1320px]:w-[180px]"
        data-action={id}
      >
        <option value="">IA par défaut ({defaultName})</option>
        {[...new Set([...choices, ...(value ? [value] : [])])].map((ai) => (
          <option key={ai} value={ai}>
            {aiLabels[ai].name}
          </option>
        ))}
      </Select>
    </div>
  );
}
