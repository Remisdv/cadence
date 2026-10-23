import { describe, expect, it } from "vitest";
import { compareVersions, nextSprintRange, nextVersions, versionStage } from "./versions";
import { formatAgo, formatDaysLeft, formatDuration, formatRange } from "./format";
import { groupBy, statusOrder } from "./issues";
import { versions } from "@/lib/ipc/mock/fixtures";

describe("versions", () => {
  it("trie en semver, pas en ordre lexical", () => {
    const sorted = ["0.11.0", "0.2.0", "0.1.10", "0.1.2", "roadmap", "1.0.0"].sort(compareVersions);
    expect(sorted).toEqual(["0.1.2", "0.1.10", "0.2.0", "0.11.0", "1.0.0", "roadmap"]);
  });

  it("propose sprint suivant, palier et majeure", () => {
    expect(nextVersions("0.1.2")).toEqual(["0.1.3", "0.2.0", "1.0.0"]);
    expect(nextVersions("v0.1.2")).toEqual(["0.1.3", "0.2.0", "1.0.0"]);
    expect(nextVersions("sprint 12")).toBeNull();
  });

  it("classe les versions de la roadmap", () => {
    const stages = versions.map((v) => versionStage(v, "0.1.2"));
    expect(stages).toEqual(["shipped", "shipped", "current", "planned", "milestone", "vision"]);
  });
});

describe("format", () => {
  it("formate les dates à la française", () => {
    expect(formatRange("2026-09-09", "2026-09-23")).toBe("9 sept. → 23 sept.");
  });

  it("formate les durées d'exécution", () => {
    expect(formatDuration(252)).toBe("04:12");
    expect(formatDuration(3852)).toBe("1:04:12");
  });

  it("formate l'ancienneté", () => {
    // ICU insère des espaces insécables : on les normalise pour comparer.
    const ago = (iso: string) =>
      formatAgo(iso, new Date("2026-09-17T10:00:00")).replace(/\s/g, " ");
    expect(ago("2026-09-17T09:56:00")).toBe("il y a 4 min");
    expect(ago("2026-09-16T18:24:00")).toBe("hier, 18:24");
  });

  it("formate les jours restants", () => {
    expect(formatDaysLeft(6)).toBe("6 j restants");
    expect(formatDaysLeft(1)).toBe("1 j restant");
  });
});

describe("groupBy", () => {
  it("garde l'ordre des colonnes même vides", () => {
    const groups = groupBy([{ s: "done" as const }], (x) => x.s, statusOrder);
    expect([...groups.keys()]).toEqual(statusOrder);
    expect(groups.get("todo")).toEqual([]);
  });
});

describe("nextSprintRange", () => {
  it("part de l'échéance close, ou d'aujourd'hui si elle est passée", () => {
    expect(nextSprintRange("2026-09-23T07:00:00Z", new Date("2026-09-20T10:00:00Z"))).toEqual({
      start: "2026-09-23",
      end: "2026-10-07",
    });
    expect(nextSprintRange("2026-09-23T07:00:00Z", new Date("2026-10-15T10:00:00Z"))).toEqual({
      start: "2026-10-15",
      end: "2026-10-29",
    });
  });
});

describe("lineDiff", () => {
  it("garde les lignes communes et marque ajouts et suppressions", async () => {
    const { lineDiff } = await import("./textDiff");
    expect(lineDiff("a\nb\nc", "a\nB\nc\nd")).toEqual([
      { type: "ctx", text: "a" },
      { type: "del", text: "b" },
      { type: "add", text: "B" },
      { type: "ctx", text: "c" },
      { type: "add", text: "d" },
    ]);
    expect(lineDiff("", "x")).toEqual([{ type: "add", text: "x" }]);
  });
});

describe("parseMarkdown", () => {
  it("lit titres, listes, cases et paragraphes", async () => {
    const { parseMarkdown } = await import("@/components/domain/Markdown");
    const blocks = parseMarkdown(
      "## Contexte\nLe client\nremonte l'erreur.\n\n## Attendu\n- un\n- deux\n\n- [ ] à faire\n- [x] fait\n\n> cité",
    );
    expect(blocks.map((b) => b.kind)).toEqual([
      "heading",
      "paragraph",
      "heading",
      "list",
      "tasks",
      "quote",
    ]);
    expect(blocks[1]).toEqual({ kind: "paragraph", text: "Le client remonte l'erreur." });
    expect(blocks[4]).toEqual({
      kind: "tasks",
      items: [
        { done: false, text: "à faire" },
        { done: true, text: "fait" },
      ],
    });
  });

  it("lit les tableaux des réponses de l'IA", async () => {
    const { parseMarkdown } = await import("@/components/domain/Markdown");
    const blocks = parseMarkdown(
      "**Contenu** (2 entrées)\n\n| Entrée | Type |\n|---|:--:|\n| `.git` | dossier |\n| `README.md` | fichier |\nfin",
      { breaks: true },
    );
    expect(blocks).toEqual([
      { kind: "paragraph", text: "**Contenu** (2 entrées)" },
      {
        kind: "table",
        head: ["Entrée", "Type"],
        rows: [
          ["`.git`", "dossier"],
          ["`README.md`", "fichier"],
        ],
      },
      { kind: "paragraph", text: "fin" },
    ]);
    expect(parseMarkdown("a | b\nsuite")).toEqual([{ kind: "paragraph", text: "a | b suite" }]);
  });
});

describe("branchName", () => {
  it("suit la convention de vcs::branch_name", async () => {
    const { branchName } = await import("./issues");
    expect(branchName("feat", 142, "Device flow OAuth GitHub + stockage trousseau")).toBe(
      "feat/142-device-flow-oauth-github-stockage",
    );
    expect(branchName("fix", 156, "Message d'erreur lisible quand le quota API est atteint")).toBe(
      "fix/156-message-d-erreur-lisible-quand-le-quota",
    );
  });
});
