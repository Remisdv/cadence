import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import {
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
  const auth = useAuthStatus().data;

  const openIssues = issues?.filter((i) => i.status !== "done").length;
  const reviewsForYou = reviews?.filter((r) => r.audience === "you").length;

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
      </div>

      <div className="flex flex-col gap-2">
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
