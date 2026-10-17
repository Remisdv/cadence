/**
 * Vue d'un plugin (maquette V2-Project-Issue.dc.html) : barre du plugin à gauche, sa vue à
 * droite. L'interface de la vue est fournie par le plugin, rendue dans une iframe isolée
 * (plan § 2.6–2.7) : le pool d'iframes la pose sur l'emplacement réservé ici.
 */
import type { ViewProps } from "@/views/registry";
import { PluginBar } from "@/components/layout/PluginBar";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";

import { kitIcon, pluginCategories } from "@/lib/domain/plugins";
import { usePlugins, useSetPluginEnabled } from "@/lib/query/queries";
import { PluginFrameSlot } from "@/plugin-host/FramePool";
import { useNavigation } from "@/stores/navigation";

export default function PluginView({ view }: ViewProps<"plugin">) {
  const plugins = usePlugins();
  const navigate = useNavigation((s) => s.navigate);
  const enable = useSetPluginEnabled();

  if (plugins.isPending) return <LoadingState />;
  if (plugins.isError) return <ErrorState error={plugins.error} />;

  const plugin = plugins.data.find((p) => p.id === view.plugin);
  if (!plugin || !plugin.enabled) {
    return (
      <Page title="Plugin" search={false}>
        <Card className="h-full">
          <EmptyState
            icon="grid"
            title={plugin ? `${plugin.name} est désactivé` : "Plugin introuvable"}
            action={
              plugin ? (
                <Button
                  variant="primary"
                  disabled={enable.isPending}
                  onClick={() => enable.mutate({ id: plugin.id, enabled: true })}
                >
                  Réactiver
                </Button>
              ) : (
                <Button variant="primary" icon="grid" onClick={() => navigate({ id: "catalog" })}>
                  Ouvrir le catalogue
                </Button>
              )
            }
          >
            {plugin
              ? "Un plugin désactivé reste installé, avec ses réglages et ses données."
              : `« ${view.plugin} » n'est pas installé sur cette machine.`}
          </EmptyState>
        </Card>
      </Page>
    );
  }

  const current = plugin.views.find((v) => v.id === view.view);

  return (
    <>
      <PluginBar plugin={plugin} view={view.view} />
      <Page
        title={current?.title ?? view.view}
        subtitle={`${plugin.name} · ${pluginCategories[plugin.category].label} · v${plugin.version}`}
        padding={plugin.uiEntry && current ? "none" : "default"}
      >
        {plugin.uiEntry && current ? (
          <PluginFrameSlot plugin={plugin.id} view={view.view} params={view.params ?? {}} />
        ) : (
          <Card className="flex h-full flex-col">
            <EmptyState
              icon={current ? kitIcon(current.icon) : "layers"}
              title={
                current ? `${plugin.name} n'a pas d'interface` : `Vue « ${view.view} » inconnue`
              }
            >
              {current
                ? `Ce plugin n'apporte que des outils et des skills pour l'IA : utilisez-le depuis le chat.`
                : `${plugin.name} ne déclare pas cette vue dans son manifeste.`}
            </EmptyState>
          </Card>
        )}
      </Page>
    </>
  );
}
