/**
 * Pool d'iframes des plugins (plan § 2.6) : une iframe par plugin, créée à la première vue
 * ouverte puis gardée (cachée, pas démontée) pour conserver son état. Changer de vue envoie
 * un événement `route` au lieu de recharger la page.
 *
 * Déplacer une iframe dans le DOM la recharge : le pool vit donc à la racine de l'app et
 * pose chaque iframe en position fixe sur l'emplacement que la vue du plugin réserve
 * (`PluginFrameSlot`).
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { create } from "zustand";
import { subscribe } from "@/lib/ipc/events";
import { usePlugins } from "@/lib/query/queries";
import type { InstalledPluginView } from "@/lib/types";
import { createBridge, type Bridge } from "./bridge";
import { isMockIpc } from "@/lib/ipc/transport";
import { hostApi, mockFrameDocument, pluginRuns, pluginUrl, shellTheme } from "./host";

type Route = { view: string; params: Record<string, string> };
type Rect = { left: number; top: number; width: number; height: number };

interface FramesState {
  /** Plugins dont l'iframe existe, dans l'ordre d'ouverture. */
  opened: string[];
  /** Dernière route de chaque plugin (réponse à `hello`). */
  routes: Record<string, Route>;
  /** Plugin affiché et emplacement réservé par sa vue. */
  active: string | null;
  rect: Rect | null;
  show: (plugin: string, route: Route) => void;
  place: (rect: Rect) => void;
  hide: (plugin: string) => void;
}

export const useFrames = create<FramesState>((set) => ({
  opened: [],
  routes: {},
  active: null,
  rect: null,
  show: (plugin, route) =>
    set((s) => ({
      opened: s.opened.includes(plugin) ? s.opened : [...s.opened, plugin],
      routes: { ...s.routes, [plugin]: route },
      active: plugin,
    })),
  place: (rect) => set({ rect }),
  hide: (plugin) => set((s) => (s.active === plugin ? { active: null, rect: null } : s)),
}));

/** Emplacement de l'iframe dans la vue d'un plugin : occupe toute la zone de contenu. */
export function PluginFrameSlot({ plugin, view, params }: { plugin: string } & Route) {
  const ref = useRef<HTMLDivElement>(null);
  const key = JSON.stringify(params);
  useEffect(() => {
    useFrames.getState().show(plugin, { view, params });
    return () => useFrames.getState().hide(plugin);
    // `params` est comparé par sa forme sérialisée.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plugin, view, key]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      useFrames.getState().place({ left: r.left, top: r.top, width: r.width, height: r.height });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);

  return <div ref={ref} className="min-h-0 min-w-0 grow" aria-hidden="true" />;
}

/** Racine du pool, montée une fois (App). */
export function FramePool() {
  const plugins = usePlugins().data ?? [];
  const opened = useFrames((s) => s.opened);
  const usable = useMemo(
    () =>
      opened
        .map((id) => plugins.find((p) => p.id === id))
        .filter((p): p is InstalledPluginView => !!p && p.enabled && !!p.uiEntry),
    [opened, plugins],
  );
  return (
    <>
      {usable.map((plugin) => (
        // La version dans la clé : une mise à jour recharge l'iframe sur la nouvelle version.
        <ManagedFrame key={`${plugin.id}@${plugin.version}`} plugin={plugin} />
      ))}
    </>
  );
}

function ManagedFrame({ plugin }: { plugin: InstalledPluginView }) {
  const iframe = useRef<HTMLIFrameElement>(null);
  const bridge = useRef<Bridge | null>(null);
  const visible = useFrames((s) => s.active === plugin.id && s.rect !== null);
  const rect = useFrames((s) => s.rect);
  const route = useFrames((s) => s.routes[plugin.id]);
  const [srcDoc, setSrcDoc] = useState<string | null>(null);
  useEffect(() => {
    if (isMockIpc) void mockFrameDocument(plugin).then(setSrcDoc);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plugin.id, plugin.version]);

  // Pont lié à la fenêtre de cette iframe (son WindowProxy ne change pas au chargement).
  useEffect(() => {
    const frame = iframe.current?.contentWindow;
    if (!frame) return;
    const b = createBridge({
      frame,
      api: hostApi(plugin),
      hello: () => ({
        api: 1,
        plugin: { id: plugin.id, version: plugin.version },
        route: useFrames.getState().routes[plugin.id] ?? { view: "", params: {} },
        theme: shellTheme(),
        locale: "fr-FR",
        host: { version: "0.3" },
      }),
    });
    bridge.current = b;
    const onMessage = (event: MessageEvent) =>
      void b.handle({ source: event.source, data: event.data });
    window.addEventListener("message", onMessage);
    const offRuns = subscribe("run://report", (report) => {
      if (pluginRuns.get(report.run) === plugin.id)
        b.emit("run", {
          runId: report.run,
          text: report.text,
          progress: report.progress,
          final: report.final,
          data: report.data,
        });
    });
    const offRunState = subscribe("chat://state", ({ session, state }) => {
      if (pluginRuns.get(session) === plugin.id) b.emit("run", { runId: session, state });
    });
    const offEvents = subscribe("plugin://event", (payload) => {
      if (payload.plugin === plugin.id) b.emit(payload.event, payload.data);
    });
    return () => {
      window.removeEventListener("message", onMessage);
      offEvents();
      offRuns();
      offRunState();
      bridge.current = null;
    };
    // Recréé seulement si l'identité du plugin change (pas à chaque relecture de la liste).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plugin.id, plugin.version, plugin.name]);

  // Nouvelle vue du même plugin : événement `route`, sans rechargement.
  const routeKey = JSON.stringify(route);
  useEffect(() => {
    if (route) bridge.current?.emit("route", route);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routeKey]);

  const box = visible && rect ? rect : { left: 0, top: 0, width: 0, height: 0 };
  return (
    <iframe
      ref={iframe}
      title={`Interface de ${plugin.name}`}
      src={isMockIpc ? undefined : pluginUrl(plugin)}
      srcDoc={isMockIpc ? (srcDoc ?? "") : undefined}
      sandbox="allow-scripts allow-forms allow-modals allow-downloads"
      allow="clipboard-read; clipboard-write"
      className="fixed border-0 bg-bg"
      style={{
        left: box.left,
        top: box.top,
        width: box.width,
        height: box.height,
        visibility: visible ? "visible" : "hidden",
        pointerEvents: visible ? "auto" : "none",
      }}
    />
  );
}
