/** Écran d'un agent : CTO (0.1.3) et développeur (0.2.0) branchés, les autres arrivent avec leur version. */
import type { ViewProps } from "@/views/registry";
import { placeholder } from "@/views/Placeholder";
import AgentCeoView from "./AgentCeoView";
import AgentCtoView from "./AgentCtoView";
import AgentDevView from "./AgentDevView";

const others = {
  deploy: placeholder("Agent déploiement", "Agent-Deploy.dc.html"),
};

export default function AgentView({ view }: ViewProps<"agent">) {
  if (view.kind === "cto") return <AgentCtoView />;
  if (view.kind === "dev") return <AgentDevView />;
  if (view.kind === "ceo") return <AgentCeoView />;
  const Other = others[view.kind];
  return <Other />;
}
