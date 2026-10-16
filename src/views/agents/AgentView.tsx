/** Écran d'un agent : CTO (0.1.3), développeur et CEO (0.2.0), déploiement (0.2.1). */
import type { ViewProps } from "@/views/registry";
import AgentCeoView from "./AgentCeoView";
import AgentCtoView from "./AgentCtoView";
import AgentDeployView from "./AgentDeployView";
import AgentDevView from "./AgentDevView";

export default function AgentView({ view }: ViewProps<"agent">) {
  switch (view.kind) {
    case "cto":
      return <AgentCtoView />;
    case "dev":
      return <AgentDevView />;
    case "ceo":
      return <AgentCeoView />;
    case "deploy":
      return <AgentDeployView />;
  }
}
