/** Écran d'un agent : CTO (0.1.3), développeur (0.2.0) et déploiement (0.2.1) branchés, le CEO arrive avec sa version. */
import type { ViewProps } from "@/views/registry";
import { placeholder } from "@/views/Placeholder";
import AgentCtoView from "./AgentCtoView";
import AgentDeployView from "./AgentDeployView";
import AgentDevView from "./AgentDevView";

const others = {
  ceo: placeholder("Agent CEO", "Agent-CEO.dc.html"),
};

export default function AgentView({ view }: ViewProps<"agent">) {
  if (view.kind === "cto") return <AgentCtoView />;
  if (view.kind === "dev") return <AgentDevView />;
  if (view.kind === "deploy") return <AgentDeployView />;
  const Other = others[view.kind];
  return <Other />;
}
