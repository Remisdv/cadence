import type { ReactNode } from "react";
import type { AgentActivity, AgentOverview } from "@/lib/types";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { StatusDot, type DotTone } from "@/components/ui/StatusDot";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { cn } from "@/lib/cn";
import { agentOrder, agents } from "@/lib/domain/agents";
import { formatDuration } from "@/lib/domain/format";
import { useElapsedSeconds } from "@/lib/hooks/useElapsedSeconds";
import {
  useAgentsOverview,
  useAuthStatus,
  useCurrentVersion,
  useIssues,
  usePendingReviews,
  usePullRequests,
} from "@/lib/query/queries";
import { useNavigation, type View } from "@/stores/navigation";
import { Logo } from "./Logo";
import { RepoSwitcher } from "./RepoSwitcher";

/** Entrée de navigation active pour une vue donnée. */
function navKey(view: View): string {
  switch (view.id) {
    case "issue":
      return "issues";
    case "agent":
      return `agent-${view.kind}`;
    default:
      return view.id;
  }
}

export function Sidebar() {
  const view = useNavigation((s) => s.view);
  const navigate = useNavigation((s) => s.navigate);
  const active = navKey(view);

  const current = useCurrentVersion().data;
  const version = current?.version.title;
  const issues = useIssues(version ? { version } : {}).data;
  const prs = usePullRequests({ state: ["draft", "open"] }).data;
  const reviews = usePendingReviews().data;
  const overview = useAgentsOverview().data ?? [];
  const auth = useAuthStatus().data;

  const openIssues = issues?.filter((i) => i.status !== "done").length;
  const reviewsForYou = reviews?.filter((r) => r.audience === "you").length;
  const running = overview.find((a) => a.activity === "running" && a.currentRun);

  const item = (key: string, target: View, icon: IconName, label: string, count?: number) => (
    <NavItem
      icon={icon}
      label={label}
      count={count}
      active={active === key}
      onClick={() => navigate(target)}
    />
  );

  return (
    <nav
      aria-label="Navigation principale"
      className="flex w-[234px] shrink-0 flex-col gap-3.5 border-r border-line bg-panel px-3 pt-3.5 pb-3"
    >
      <Logo className="px-1" />

      <RepoSwitcher />

      <div className="flex min-h-0 grow flex-col gap-[13px] overflow-y-auto">
        <NavGroup label="Pilotage">
          {item("sprint", { id: "sprint" }, "sprint", "Sprint en cours")}
          {item("roadmap", { id: "roadmap" }, "roadmap", "Roadmap")}
          {item("version", { id: "version" }, "tag", "Versions")}
        </NavGroup>

        <NavGroup label="Travail">
          {item("issues", { id: "issues" }, "issue", "Issues", openIssues)}
          {item(
            "pull-requests",
            { id: "pull-requests" },
            "pullRequest",
            "Pull requests",
            prs?.length,
          )}
          {item("reviews", { id: "reviews" }, "review", "Revues à faire", reviewsForYou)}
        </NavGroup>

        <NavGroup label="Agents">
          {item("agents", { id: "agents" }, "bot", "Console agents")}
          <div className="mt-0.5 ml-[9px] flex flex-col gap-0.5 border-l border-line pl-1.5">
            {agentOrder.map((kind) => (
              <AgentNavItem
                key={kind}
                agent={overview.find((a) => a.kind === kind)}
                kind={kind}
                active={active === `agent-${kind}`}
                onClick={() => navigate({ id: "agent", kind })}
              />
            ))}
          </div>
        </NavGroup>
      </div>

      <div className="flex flex-col gap-2">
        {running?.currentRun && <RunningAgentCard agent={running} />}
        <NavItem
          icon="settings"
          label="Réglages"
          active={active === "settings"}
          onClick={() => navigate({ id: "settings" })}
        />
        <div className="flex items-center gap-[9px] border-t border-line px-[9px] py-1.5">
          <Avatar
            size={24}
            initials={initialsOf(auth?.github.login ?? "?")}
            src={auth?.github.avatarUrl}
          />
          <span className="grow text-body-sm">Compte GitHub</span>
          <StatusDot tone={auth?.github.status === "connected" ? "green" : "red"} />
        </div>
      </div>
    </nav>
  );
}

function NavGroup({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <SectionLabel className="px-[9px] pb-[5px]">{label}</SectionLabel>
      {children}
    </div>
  );
}

interface NavItemProps {
  icon: IconName;
  label: string;
  count?: number;
  active: boolean;
  onClick: () => void;
}

export function NavItem({ icon, label, count, active, onClick }: NavItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-[31px] w-full items-center gap-[9px] rounded-control px-[9px] text-left text-body transition-colors",
        active ? "bg-p3 font-medium text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      <span className={cn("flex", active ? "text-clay" : "text-tx3")}>
        <Icon name={icon} size={16} />
      </span>
      <span className="grow">{label}</span>
      {count !== undefined && <span className="mono text-tiny text-tx3">{count}</span>}
    </button>
  );
}

const activityDot: Record<AgentActivity, DotTone> = {
  idle: "neutral",
  waiting: "amber",
  running: "green",
};

function AgentNavItem({
  kind,
  agent,
  active,
  onClick,
}: {
  kind: AgentOverview["kind"];
  agent?: AgentOverview;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-[29px] w-full items-center gap-[9px] rounded-control pr-[9px] pl-2.5 text-left text-body-sm transition-colors",
        active ? "bg-p3 text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
      )}
    >
      <AgentBadge agent={kind} size={17} />
      <span className="grow">{agents[kind].short}</span>
      <StatusDot tone={activityDot[agent?.activity ?? "idle"]} />
    </button>
  );
}

function RunningAgentCard({ agent }: { agent: AgentOverview }) {
  const run = agent.currentRun!;
  const elapsed = useElapsedSeconds(run.startedAt);
  const progress = run.step && run.steps ? ((run.step - 0.5) / run.steps) * 100 : 0;
  const navigate = useNavigation((s) => s.navigate);

  return (
    <button
      type="button"
      onClick={() => navigate({ id: "agent", kind: agent.kind })}
      className="rounded-field border border-green-line bg-green-d px-2.5 py-[9px] text-left"
    >
      <div className="flex items-center gap-[7px] text-caption font-medium text-green">
        <StatusDot tone="green" pulse />
        Agent {agents[agent.kind].short.toLowerCase()} en cours
      </div>
      <div className="mt-1 mono text-micro text-tx2">
        {run.issue && `#${run.issue} · `}
        {run.step && run.steps && `étape ${run.step}/${run.steps} · `}
        {formatDuration(elapsed)}
      </div>
      <ProgressBar className="mt-[7px]" value={progress} tone="green" track="green" height={3} />
    </button>
  );
}
