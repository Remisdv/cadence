import { describe, expect, it } from "vitest";
import { versions } from "@/lib/ipc/mock/fixtures";
import type { AgentRunDetail, CeoProposal, Version } from "@/lib/types";
import {
  ceoTarget,
  isDefaultPick,
  milestonesLabel,
  pendingProposals,
  themeCoverage,
  themesLabel,
} from "./ceo";

const proposal = (over: Partial<CeoProposal> = {}): CeoProposal => ({
  index: 1,
  title: "Inviter des collaborateurs",
  body: "## Contexte",
  milestone: "0.2.0",
  issueType: "feat",
  labels: [],
  theme: "Rôles et permissions",
  rationale: "Rien ne couvre l'invitation.",
  overlap: null,
  overlapIssues: [],
  state: "ready",
  issue: null,
  issueUrl: null,
  error: null,
  ...over,
});

describe("ceoTarget", () => {
  it("vise le premier palier ouvert après la version en cours", () => {
    expect(ceoTarget(versions, "0.1.2")).toBe("0.2.0");
    expect(ceoTarget(versions, "0.2.0")).toBe("1.0.0");
    expect(ceoTarget(versions, null)).toBe("0.2.0");
  });

  it("retombe sur la prochaine version ouverte, ou rien", () => {
    const sprints: Version[] = versions.filter((v) => v.kind === "sprint");
    expect(ceoTarget(sprints, "0.1.2")).toBe("0.1.3");
    expect(ceoTarget(sprints, "0.1.3")).toBeNull();
  });
});

describe("propositions", () => {
  it("coche d'office celles du palier visé sans recouvrement", () => {
    expect(isDefaultPick(proposal(), "0.2.0")).toBe(true);
    expect(isDefaultPick(proposal({ milestone: "1.0.0" }), "0.2.0")).toBe(false);
    expect(isDefaultPick(proposal({ overlap: "Recouvre #145" }), "0.2.0")).toBe(false);
    expect(isDefaultPick(proposal({ state: "failed" }), "0.2.0")).toBe(false);
  });

  it("marque les thèmes couverts et ajoute ceux cités hors description", () => {
    const palier = versions.find((v) => v.title === "0.2.0");
    const coverage = themeCoverage(palier, [
      proposal(),
      proposal({ index: 2, theme: "revue assistée", state: "skipped" }),
      proposal({ index: 3, theme: "Notifications" }),
    ]);
    expect(coverage).toEqual([
      { theme: "Rôles et permissions", covered: true },
      { theme: "Revue assistée", covered: false },
      { theme: "Multi-dépôts", covered: false },
      { theme: "Notifications", covered: false },
    ]);
  });

  it("résume thèmes et milestones, et ne garde que ce qui attend", () => {
    const list = [proposal(), proposal({ index: 2, milestone: "1.0.0", theme: "Notifications" })];
    expect(themesLabel(list)).toBe("« Rôles et permissions », « Notifications »");
    expect(milestonesLabel(list.slice(0, 1))).toBe("le milestone 0.2.0");
    expect(milestonesLabel(list)).toBe("les milestones 0.2.0 et 1.0.0");
    const run = {
      kind: "ceo",
      proposals: [...list, proposal({ index: 3, state: "applied" })],
    } as AgentRunDetail;
    expect(pendingProposals(run).map((p) => p.index)).toEqual([1, 2]);
    expect(pendingProposals(null)).toEqual([]);
  });
});
