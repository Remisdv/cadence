/**
 * Catalogue de plugins (maquette V2-Catalog.dc.html) : vues et catégories dans la barre,
 * cartes des plugins à droite. Le catalogue en ligne (`catalog.json` du dépôt
 * RemiSDV/cadence-plugins) arrive avec l'installation ; l'écran montre pour l'instant les
 * plugins installés sur cette machine.
 */
import { useState } from "react";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import {
  ContextBar,
  ContextBarHeader,
  ContextBarHint,
  ContextBarItem,
  ContextBarSection,
} from "@/components/layout/ContextBar";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { cn } from "@/lib/cn";
import { categoryOrder, pluginCategories } from "@/lib/domain/plugins";
import { usePlugins, useSetPluginEnabled } from "@/lib/query/queries";
import type { InstalledPluginView, PluginCategory } from "@/lib/types";
import { useNavigation } from "@/stores/navigation";

export default function CatalogView({ view }: ViewProps<"catalog">) {
  const plugins = usePlugins();
  const [category, setCategory] = useState<PluginCategory | null>(null);
  const all = plugins.data ?? [];
  const shown = all.filter((p) => (category ? p.category === category : true));
  const focused = view.plugin ?? null;

  return (
    <>
      <ContextBar
        label="Catalogue"
        header={<ContextBarHeader tile={<NeutralTile icon="grid" />} title="Plugins" />}
        footer={
          <ContextBarHint>
            La catégorie est déclarée dans le manifeste, parmi une liste fixe. Elle range le plugin
            dans le rail.
          </ContextBarHint>
        }
      >
        <ContextBarSection title="Vues">
          <ContextBarItem
            icon="grid"
            label="Catalogue"
            active={category === null}
            onClick={() => setCategory(null)}
          />
          <ContextBarItem icon="check" label="Installés" count={all.length} />
          <ContextBarItem icon="download" label="Mises à jour" count={0} />
        </ContextBarSection>
        <ContextBarSection title="Catégories">
          <CategoryFilter
            label="Toutes"
            count={all.length}
            active={category === null}
            onClick={() => setCategory(null)}
          />
          {categoryOrder.map((c) => (
            <CategoryFilter
              key={c}
              label={pluginCategories[c].label}
              count={all.filter((p) => p.category === c).length}
              active={category === c}
              onClick={() => setCategory(c)}
            />
          ))}
        </ContextBarSection>
      </ContextBar>

      <Page
        title="Catalogue de plugins"
        subtitle="dépôt public RemiSDV/cadence-plugins · plugins installés sur cette machine"
      >
        {plugins.isPending ? (
          <LoadingState />
        ) : plugins.isError ? (
          <ErrorState error={plugins.error} />
        ) : shown.length === 0 ? (
          <Card className="h-full">
            <EmptyState icon="grid" title="Aucun plugin installé">
              L'installation depuis le catalogue public arrive avec la prochaine étape de Cadence 2.
            </EmptyState>
          </Card>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-3">
            {shown.map((p) => (
              <PluginCard key={p.id} plugin={p} highlight={p.id === focused} />
            ))}
          </div>
        )}
      </Page>
    </>
  );
}

function CategoryFilter({
  label,
  count,
  active,
  onClick,
}: {
  label: string;
  count: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex h-[30px] w-full items-center gap-[9px] rounded-control px-[9px] text-left text-body",
        active ? "bg-p3 font-medium text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      <span
        className={cn(
          "inline-block size-1.5 shrink-0 rounded-full",
          active ? "bg-clay" : "bg-line2",
        )}
      />
      <span className="grow">{label}</span>
      <span className={cn("mono text-tiny", active ? "text-clay" : "text-tx3")}>{count}</span>
    </button>
  );
}

function PluginCard({ plugin, highlight }: { plugin: InstalledPluginView; highlight: boolean }) {
  const navigate = useNavigation((s) => s.navigate);
  const setEnabled = useSetPluginEnabled();
  return (
    <Card className={cn("flex flex-col gap-3 p-4", highlight && "border-clay-line")}>
      <div className="flex items-start gap-3">
        <PluginTile color={plugin.color} icon={plugin.railIcon} />
        <div className="min-w-0 grow">
          <div className="flex items-center gap-2">
            <span className="truncate text-body font-semibold">{plugin.name}</span>
            {plugin.source === "dev" && <Badge tone="purple">Développement</Badge>}
            <Badge tone="muted">{pluginCategories[plugin.category].label}</Badge>
          </div>
          <div className="mt-0.5 mono text-caption text-tx3">
            {plugin.id} · v{plugin.version}
          </div>
        </div>
      </div>
      {plugin.description && (
        <p className="text-body-sm leading-[1.55] text-tx2">{plugin.description}</p>
      )}
      {plugin.problem && <p className="text-caption text-amber">{plugin.problem}</p>}
      <div className="mt-auto flex flex-wrap items-center gap-2">
        {plugin.status === "invalid" ? (
          <Badge tone="red">Invalide</Badge>
        ) : plugin.enabled ? (
          <Badge tone="green" icon="check">
            Activé
          </Badge>
        ) : (
          <Badge tone="muted">Désactivé</Badge>
        )}
        {plugin.status === "needs-setup" && <Badge tone="amber">À configurer</Badge>}
        {plugin.services.includes("repos") && <Badge tone="muted">Dépôts GitHub</Badge>}
        <div className="grow" />
        <Button
          size="sm"
          variant="ghost"
          disabled={setEnabled.isPending || plugin.status === "invalid"}
          onClick={() => setEnabled.mutate({ id: plugin.id, enabled: !plugin.enabled })}
        >
          {plugin.enabled ? "Désactiver" : "Activer"}
        </Button>
        <Button
          size="sm"
          icon="settings"
          onClick={() => navigate({ id: "settings", section: "plugin", plugin: plugin.id })}
        >
          Réglages
        </Button>
      </div>
    </Card>
  );
}
