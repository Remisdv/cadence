/**
 * Vue d'un plugin (maquette V2-Project-Issue.dc.html) : barre du plugin à gauche, sa vue à
 * droite. L'interface de la vue est fournie par le plugin et sera rendue dans une iframe
 * isolée (plan § 2.6–2.7) ; en attendant l'hôte d'iframe, la zone montre ce qu'elle recevra.
 */
import type { ViewProps } from "@/views/registry";
import { PluginBar } from "@/components/layout/PluginBar";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { KeyValue } from "@/components/ui/KeyValue";
import { kitIcon, pluginCategories } from "@/lib/domain/plugins";
import { usePlugins, useSetPluginEnabled } from "@/lib/query/queries";
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
  const params = Object.entries(view.params ?? {});

  return (
    <>
      <PluginBar plugin={plugin} view={view.view} />
      <Page
        title={current?.title ?? view.view}
        subtitle={`${plugin.name} · ${pluginCategories[plugin.category].label} · v${plugin.version}`}
      >
        <Card className="flex h-full flex-col">
          <EmptyState
            icon={current ? kitIcon(current.icon) : "layers"}
            title={current ? `Vue « ${current.title} »` : `Vue « ${view.view} » inconnue`}
          >
            {current
              ? `Cette vue est fournie par ${plugin.name} : son interface s'affichera ici, dans un cadre isolé, dès que l'hôte d'iframe sera branché.`
              : `${plugin.name} ne déclare pas cette vue dans son manifeste.`}
          </EmptyState>
          {params.length > 0 && (
            <div className="mx-auto mb-6 w-[320px]">
              {params.map(([k, v]) => (
                <KeyValue key={k} label={k} value={v} />
              ))}
            </div>
          )}
        </Card>
      </Page>
    </>
  );
}
