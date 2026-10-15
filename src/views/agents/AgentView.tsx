/** Écran d'un agent : CTO (0.1.3) et développeur (0.2.0) branchés, les autres arrivent avec leur version. */
import type { ViewProps } from "@/views/registry";
import { placeholder } from "@/views/Placeholder";
import AgentCtoView from "./AgentCtoView";
import AgentDevView from "./AgentDevView";

const others = {
  ceo: placeholder("Agent CEO", "Agent-CEO.dc.html"),
  deploy: placeholder("Agent déploiement", "Agent-Deploy.dc.html"),
};

export default function AgentView({ view }: ViewProps<"agent">) {
  if (view.kind === "cto") return <AgentCtoView />;
  if (view.kind === "dev") return <AgentDevView />;
  const Other = others[view.kind];
  return <Other />;
}
