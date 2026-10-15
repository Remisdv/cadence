/** Écran d'un agent : l'agent CTO est branché (0.1.3), les autres arrivent avec leur version. */
import type { ViewProps } from "@/views/registry";
import { placeholder } from "@/views/Placeholder";
import AgentCtoView from "./AgentCtoView";

const others = {
  ceo: placeholder("Agent CEO", "Agent-CEO.dc.html"),
  dev: placeholder("Agent développeur", "Agent-Dev.dc.html"),
  deploy: placeholder("Agent déploiement", "Agent-Deploy.dc.html"),
};

export default function AgentView({ view }: ViewProps<"agent">) {
  if (view.kind === "cto") return <AgentCtoView />;
  const Other = others[view.kind];
  return <Other />;
}
