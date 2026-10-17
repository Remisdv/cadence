import { describe, expect, it } from "vitest";
import type { ToolInfo } from "@/lib/types";
import {
  applyRunEvent,
  blocksContinue,
  installable,
  loginUrl,
  pairingCode,
  startLog,
  toolStatus,
  toolsSummary,
} from "./tools";

function tool(patch: Partial<ToolInfo>): ToolInfo {
  return {
    id: "gh",
    name: "GitHub CLI",
    description: "",
    status: "ready",
    path: null,
    version: null,
    minVersion: "2.40.0",
    account: null,
    test: null,
    login: "gh auth login --web",
    loginInteractive: false,
    homepage: "https://cli.github.com",
    recipes: [
      {
        id: "winget",
        label: "recette winget",
        command: "winget install",
        available: true,
        needs: null,
        elevated: true,
      },
    ],
    requestedBy: [],
    required: false,
    ignored: false,
    message: null,
    ...patch,
  };
}

describe("registre d'outils", () => {
  it("résume l'en-tête comme la maquette", () => {
    const summary = toolsSummary([
      tool({ id: "git", status: "ready", login: null }),
      tool({ status: "missing" }),
      tool({ id: "codex", status: "not-connected", required: true }),
      tool({ id: "node", status: "missing", required: true }),
    ]);
    expect(summary).toEqual({ ready: 1, toConnect: 1, missingRequired: 1, missingOptional: 1 });
  });

  it("ne bloque que sur un outil requis non prêt et non ignoré", () => {
    expect(blocksContinue(tool({ status: "missing", required: true }))).toBe(true);
    expect(blocksContinue(tool({ status: "missing", required: false }))).toBe(false);
    expect(blocksContinue(tool({ status: "missing", required: true, ignored: true }))).toBe(false);
    expect(blocksContinue(tool({ status: "ready", required: true }))).toBe(false);
  });

  it("libelle les états", () => {
    expect(toolStatus(tool({})).label).toBe("Connecté");
    expect(toolStatus(tool({ id: "git", login: null })).label).toBe("Installé");
    expect(toolStatus(tool({ id: "opencode", status: "missing" })).label).toBe("Introuvable");
    expect(toolStatus(tool({ status: "missing" })).label).toBe("Manquant");
  });

  it("propose à « Tout installer » les outils manquants installables", () => {
    const list = installable([
      tool({ status: "missing" }),
      tool({ id: "node", status: "missing", ignored: true }),
      tool({ id: "git", status: "missing", recipes: [] }),
      tool({ id: "codex", status: "ready" }),
    ]);
    expect(list.map((t) => t.id)).toEqual(["gh"]);
  });

  it("reconstruit le journal, barres de progression comprises", () => {
    let log = startLog("install", "winget install --id GitHub.cli -e");
    const ev = (line: string | null, replace = false) =>
      ({ tool: "gh", kind: "install", line, replace, done: null }) as const;
    log = applyRunEvent(log, ev("Trouvé GitHub CLI"));
    log = applyRunEvent(log, ev("██░░ 10 %"));
    log = applyRunEvent(log, ev("████ 100 %", true));
    expect(log.lines).toEqual(["Trouvé GitHub CLI", "████ 100 %"]);
    log = applyRunEvent(log, {
      tool: "gh",
      kind: "install",
      line: null,
      replace: false,
      done: { ok: true, exitCode: 0, message: null, durationMs: 38_000 },
    });
    expect(log.running).toBe(false);
    expect(log.result?.ok).toBe(true);
  });

  it("repère le code d'appairage et l'adresse de gh", () => {
    const lines = [
      "! First copy your one-time code: a1b2-c3d4",
      "Press Enter to open https://github.com/login/device in your browser...",
    ];
    expect(pairingCode(lines)).toBe("A1B2-C3D4");
    expect(loginUrl(lines)).toBe("https://github.com/login/device");
  });
});
