/**
 * Plugins (V2-Onboarding-Plugins.dc.html) : rien n'est livré d'office, tout vient du
 * catalogue. Les plugins cochés s'installent l'un après l'autre ; décocher un plugin déjà
 * installé le désactive (il reste installé, avec ses réglages).
 */
import { useMemo, useState, type ReactNode } from "react";
import type { PluginCategory } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon } from "@/components/icons/Icon";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Spinner } from "@/components/ui/Spinner";
import { PluginTile } from "@/components/domain/PluginTile";
import { OnboardingActions, OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { LoadingState } from "@/components/layout/EmptyState";
import {
  catalogCards,
  categoryOrder,
  pluginCategories,
  type CatalogCard,
} from "@/lib/domain/plugins";
import { formatSize, stepIndex } from "@/lib/domain/onboarding";
import {
  useCatalog,
  useInstallFromCatalog,
  usePlugins,
  useRefreshCatalog,
  useSetPluginEnabled,
} from "@/lib/query/queries";
import type { StepProps } from "../OnboardingView";

export function PluginsStep({ onNext, onBack }: StepProps) {
  const catalog = useCatalog();
  const installed = usePlugins();
  const refresh = useRefreshCatalog();
  const install = useInstallFromCatalog();
  const enable = useSetPluginEnabled();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<PluginCategory | null>(null);
  const [picked, setPicked] = useState<Set<string> | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number; name: string } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  const cards = useMemo(
    () =>
      catalogCards(catalog.data?.plugins ?? [], installed.data ?? []).filter(
        (c) => c.entry?.compatible !== false || c.installed,
      ),
    [catalog.data, installed.data],
  );

  // Sélection de départ : ce qui est déjà activé, sinon les plugins officiels.
  const selected =
    picked ??
    new Set(
      cards.some((c) => c.installed?.enabled)
        ? cards.filter((c) => c.installed?.enabled).map((c) => c.id)
        : cards.filter((c) => c.official).map((c) => c.id),
    );

  const frame = (children: ReactNode) => (
    <OnboardingFrame
      step={stepIndex("plugins")}
      title="Choisir vos plugins"
      intro="Rien n'est livré d'office : même les deux plugins officiels s'installent depuis le catalogue. Chaque plugin déclare sa catégorie, qui fixe sa place dans le rail de l'app. Tout reste activable ou retirable ensuite."
    >
      {children}
    </OnboardingFrame>
  );

  if (catalog.isPending || installed.isPending) return frame(<LoadingState />);

  const visible = cards.filter(
    (c) =>
      (!category || c.category === category) &&
      (!query.trim() ||
        `${c.name} ${c.description}`.toLowerCase().includes(query.trim().toLowerCase())),
  );
  const toInstall = cards.filter((c) => selected.has(c.id) && !c.installed && c.entry);
  const toEnable = cards.filter((c) => selected.has(c.id) && c.installed && !c.installed.enabled);
  const toDisable = cards.filter((c) => !selected.has(c.id) && c.installed?.enabled);
  const size = toInstall.reduce((sum, c) => sum + (c.entry?.size ?? 0), 0);
  const busy = progress !== null;

  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setPicked(next);
  };

  const apply = async () => {
    setError(null);
    try {
      for (const card of toDisable) await enable.mutateAsync({ id: card.id, enabled: false });
      for (const card of toEnable) await enable.mutateAsync({ id: card.id, enabled: true });
      for (const [i, card] of toInstall.entries()) {
        setProgress({ done: i, total: toInstall.length, name: card.name });
        await install.mutateAsync({ id: card.id, update: false });
      }
      setProgress(null);
      onNext();
    } catch (e) {
      setProgress(null);
      setError((e as Error).message);
    }
  };

  const counts = (c: PluginCategory | null) =>
    cards.filter((card) => !c || card.category === c).length;

  return frame(
    <div className="flex flex-col gap-3.5">
      <div className="flex flex-wrap items-center gap-[7px]">
        <div className="flex h-8 w-[200px] items-center gap-2 rounded-control border border-line bg-p2 px-2.5 text-tx3">
          <Icon name="search" size={14} />
          <input
            aria-label="Rechercher un plugin"
            placeholder="Rechercher…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
          />
        </div>
        <Chip active={category === null} count={counts(null)} onClick={() => setCategory(null)}>
          Toutes
        </Chip>
        {categoryOrder
          .filter((c) => counts(c) > 0)
          .map((c) => (
            <Chip key={c} active={category === c} count={counts(c)} onClick={() => setCategory(c)}>
              {pluginCategories[c].label}
            </Chip>
          ))}
        <div className="grow" />
        <span className="text-small whitespace-nowrap text-tx3">
          <span className="text-tx">{selected.size}</span> sélectionné{selected.size > 1 ? "s" : ""}
        </span>
      </div>

      {catalog.data?.error && (
        <Callout tone="warning">
          {catalog.data.offline
            ? "Catalogue hors ligne : voici la dernière liste connue. "
            : `Catalogue indisponible : ${catalog.data.error} `}
          <button
            type="button"
            className="text-clay hover:text-clay-hover"
            disabled={refresh.isPending}
            onClick={() => refresh.mutate()}
          >
            Réessayer
          </button>
        </Callout>
      )}

      <div className="grid grid-cols-2 gap-3">
        {visible.map((card) => (
          <PluginChoice
            key={card.id}
            card={card}
            checked={selected.has(card.id)}
            disabled={busy}
            onToggle={() => toggle(card.id)}
          />
        ))}
        {visible.length === 0 && (
          <div className="col-span-2 rounded-card border border-dashed border-line2 px-4 py-6 text-center text-body-sm text-tx3">
            {cards.length === 0
              ? "Le catalogue est vide pour l'instant : vous pourrez ajouter des plugins plus tard."
              : "Aucun plugin ne correspond à cette recherche."}
          </div>
        )}
      </div>

      <Callout tone="info" icon="info">
        Un plugin s'exécute avec vos droits, sans système de permissions. Le filtre, c'est la revue
        des pull requests par <span className="mono text-tx">@RemiSDV</span> et l'empreinte SHA-256
        de chaque archive.
      </Callout>

      <OnboardingActions onBack={busy ? undefined : onBack}>
        <Button
          variant="primary"
          size="lg"
          icon={busy ? undefined : toInstall.length > 0 ? "download" : undefined}
          iconRight={!busy && toInstall.length === 0 ? "arrowRight" : undefined}
          disabled={busy}
          onClick={apply}
        >
          {busy && <Spinner className="text-clay-ink" />}
          {busy
            ? `Installation de ${progress.name} · ${progress.done + 1}/${progress.total}`
            : toInstall.length > 0
              ? `Installer ${toInstall.length} plugin${toInstall.length > 1 ? "s" : ""}`
              : "Continuer"}
        </Button>
        <span className="text-body-sm text-tx3">
          {error ? (
            <span className="text-red">{error}</span>
          ) : toInstall.length > 0 ? (
            `${size > 0 ? `${formatSize(size)} à télécharger · ` : ""}aucun plugin n'est requis`
          ) : (
            "Aucun plugin n'est requis : le chat fonctionne sans."
          )}
        </span>
      </OnboardingActions>
    </div>,
  );
}

function Chip({
  active,
  count,
  onClick,
  children,
}: {
  active: boolean;
  count: number;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "inline-flex h-[30px] items-center gap-1.5 rounded-control border px-[11px] text-body-sm",
        active
          ? "border-clay-line bg-clay-d text-clay"
          : "border-line bg-p2 text-tx2 hover:text-tx",
      )}
    >
      {children}
      <span className="mono text-tiny opacity-75">{count}</span>
    </button>
  );
}

function PluginChoice({
  card,
  checked,
  disabled,
  onToggle,
}: {
  card: CatalogCard;
  checked: boolean;
  disabled: boolean;
  onToggle: () => void;
}) {
  const entry = card.entry;
  const views = entry?.views.length ?? card.installed?.views.filter((v) => !v.hidden).length ?? 0;
  const actions = entry?.actions.length ?? 0;
  const tools = entry?.tools.length ?? 0;
  const plural = (n: number, word: string) => `${n} ${word}${n > 1 ? "s" : ""}`;
  return (
    <label
      className={cn(
        "flex cursor-pointer flex-col gap-2.5 rounded-card border px-4 py-[15px]",
        checked ? "border-clay-line bg-p2" : "border-line bg-panel hover:border-line2",
      )}
    >
      <div className="flex items-start gap-3">
        <PluginTile color={card.color} icon={card.railIcon} size={36} />
        <div className="min-w-0 grow">
          <div className="flex items-center gap-[7px]">
            <span className="truncate text-title-sm font-semibold">{card.name}</span>
            {card.official ? (
              <Badge tone="clay">Officiel</Badge>
            ) : (
              <Badge tone="muted">Communauté</Badge>
            )}
          </div>
          <div className="mt-1 flex items-center gap-[7px] text-caption text-tx3">
            <span className="inline-flex h-[19px] items-center rounded-badge border border-line2 px-[7px] text-tiny font-medium whitespace-nowrap text-tx2">
              {pluginCategories[card.category].label}
            </span>
            <span className="truncate">
              {card.authors.length > 0 && `${card.authors.join(", ")} · `}
              <span className="mono">v{card.version}</span>
              {card.installed && " · installé"}
            </span>
          </div>
        </div>
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={onToggle}
          className="mt-0.5 size-[17px] accent-clay"
        />
      </div>
      <p className="text-body-sm leading-[1.6] text-tx2">{card.description}</p>
      <div className="flex flex-wrap items-center gap-1.5">
        {views > 0 && <Badge>{plural(views, "vue")}</Badge>}
        {actions > 0 && <Badge>{plural(actions, "action")}</Badge>}
        {tools > 0 && <Badge>{plural(tools, "outil")}</Badge>}
        <div className="grow" />
        {card.services.includes("repos") && (
          <span className="inline-flex items-center gap-[5px] text-caption text-tx3">
            <Icon name="github" size={13} />
            Service Dépôts GitHub
          </span>
        )}
      </div>
    </label>
  );
}
