import { describe, expect, it } from "vitest";
import { pullRequests } from "@/lib/ipc/mock/fixtures";
import type { PrRow } from "@/lib/types";
import { defaultPrFilter, matchesPr, parsePatch, reviewSummary, waitingFor } from "./prs";

const now = new Date("2026-10-15T12:00:00Z");
const base = pullRequests[0]!;
const pr = (p: Partial<PrRow>): PrRow => ({ ...base, ...p });

describe("matchesPr", () => {
  it("garde ouvertes et brouillons par défaut, filtre auteur et version", () => {
    const open = pullRequests.filter((p) => matchesPr(p, defaultPrFilter));
    expect(open.every((p) => p.state === "open" || p.state === "draft")).toBe(true);
    const mine = pullRequests.filter((p) => matchesPr(p, { ...defaultPrFilter, author: "me" }));
    expect(mine.every((p) => p.author.isSelf)).toBe(true);
    const v011 = pullRequests.filter((p) =>
      matchesPr(p, { ...defaultPrFilter, state: "all", versions: ["0.1.1"] }),
    );
    expect(v011.map((p) => p.number)).toEqual([76]);
  });
});

describe("reviewSummary", () => {
  it("préfère les changements demandés, puis les approbations, puis l'attente", () => {
    expect(reviewSummary(pr({ changesRequested: 1, approvals: 2 }), now)).toEqual({
      text: "1 changement demandé",
      tone: "amber",
    });
    expect(reviewSummary(pr({ changesRequested: 0, approvals: 2 }), now)?.text).toBe("2 approbations");
    expect(
      reviewSummary(
        pr({ changesRequested: 0, approvals: 0, state: "open", reviewRequestedAt: "2026-10-13T10:00:00Z" }),
        now,
      ),
    ).toEqual({ text: "en attente · 2 j", tone: "red" });
    expect(reviewSummary(pr({ changesRequested: 0, approvals: 0, reviewRequestedAt: null }), now)).toBeNull();
  });

  it("compte l'attente en heures puis en jours", () => {
    expect(waitingFor("2026-10-15T06:00:00Z", now)).toEqual({ label: "6 h", tone: "amber" });
    expect(waitingFor("2026-10-15T11:30:00Z", now).label).toBe("< 1 h");
  });
});

describe("parsePatch", () => {
  it("lit ajouts, suppressions et contexte, et sépare les blocs", () => {
    const lines = parsePatch(
      "@@ -1,3 +1,3 @@\n .rail {\n-  height: 100vh;\n+  height: 100%;\n@@ -20,2 +20,2 @@\n }\n\\ No newline at end of file",
    );
    expect(lines).toEqual([
      { type: "ctx", text: ".rail {" },
      { type: "del", text: "  height: 100vh;" },
      { type: "add", text: "  height: 100%;" },
      { type: "ctx", text: "⋯" },
      { type: "ctx", text: "}" },
    ]);
  });
});
