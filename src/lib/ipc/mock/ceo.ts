/**
 * Simulation de l'agent CEO en mode mock : une exécution déjà relue au démarrage (les cinq
 * propositions des maquettes), et « Relancer » rejoue une session où les propositions
 * arrivent une à une pendant que la console défile. Même contrat que Rust.
 */
import type {
  AgentEventKind,
  AgentRunDetail,
  ApplyReport,
  CeoProposal,
  IssueRow,
} from "@/lib/types";
import { mockBus } from "./bus";
import * as data from "./fixtures";

const STEP_MS = 500;
let timer: ReturnType<typeof setTimeout> | null = null;

type Draft = Pick<CeoProposal, "title" | "milestone" | "theme" | "rationale" | "overlap"> &
  Partial<Pick<CeoProposal, "overlapIssues" | "labels">>;

const drafts: Draft[] = [
  {
    title: "Inviter des collaborateurs avec un rôle",
    milestone: "0.2.0",
    theme: "Rôles et permissions",
    rationale:
      "Aucune issue ouverte ne couvre l'invitation ; le stockage du jeton (#142) suppose déjà un utilisateur unique.",
    overlap: null,
    labels: ["area/auth"],
  },
  {
    title: "Journal d'activité partagé entre membres",
    milestone: "0.2.0",
    theme: "Rôles et permissions",
    rationale:
      "Le journal des agents existe en local (0.1.2) mais n'est pas partagé : prérequis du travail à plusieurs.",
    overlap: null,
    labels: ["area/agents"],
  },
  {
    title: "Sélecteur multi-dépôts dans le rail",
    milestone: "0.2.0",
    theme: "Multi-dépôts",
    rationale: "Le thème n'a aucune issue. Le sélecteur actuel est mono-dépôt depuis 0.1.0.",
    overlap: null,
    labels: ["area/ui"],
  },
  {
    title: "Pré-revue de PR assistée par agent",
    milestone: "0.2.0",
    theme: "Revue assistée",
    rationale: "Aucune issue ne prévoit de relecture par un agent avant la revue humaine.",
    overlap: "Recouvre partiellement le journal des agents (#145) — à fusionner ou à préciser.",
    overlapIssues: [145],
    labels: ["area/agents"],
  },
  {
    title: "Notifications système en fin de sprint",
    milestone: "1.0.0",
    theme: "Notifications",
    rationale:
      "Dérivé du thème « Notifications », marqué comme non prioritaire dans la roadmap. Proposé pour 1.0.0 plutôt que 0.2.0.",
    overlap: null,
  },
];

function toProposal(d: Draft, index: number): CeoProposal {
  return {
    index,
    title: d.title,
    body: [
      "## Contexte",
      d.rationale,
      "",
      "## Attendu",
      `- ${d.title}`,
      "- Libellés et messages en français",
      "",
      "## Critères d'acceptation",
      "- [ ] Le cas nominal est couvert par un test",
      "- [ ] La documentation est à jour",
    ].join("\n"),
    milestone: d.milestone,
    issueType: "feat",
    labels: d.labels ?? [],
    theme: d.theme,
    rationale: d.rationale,
    overlap: d.overlap,
    overlapIssues: d.overlapIssues ?? [],
    state: "ready",
    issue: null,
    issueUrl: null,
    error: null,
  };
}

const hourAgo = Date.now() - 3_600_000;

/** Exécution d'il y a une heure, en attente de relecture (maquettes Roadmap et Agent CEO). */
let run: AgentRunDetail | null = {
  run: "ceo-seed",
  kind: "ceo",
  state: "review",
  autonomy: "propose",
  version: "0.2.0",
  model: data.config.agents.ceo.model,
  startedAt: new Date(hourAgo - 180_000).toISOString(),
  endedAt: new Date(hourAgo).toISOString(),
  items: [],
  tasks: [],
  log: [
    { kind: "system", text: `› copie de ${data.project.repo.fullName} (main)…` },
    { kind: "system", text: "› 0.2.0 : 3 thèmes, 24 issues lues, README.md" },
    { kind: "read", text: "read  README.md" },
    { kind: "read", text: "glob  docs/**/*.md" },
    { kind: "success", text: "✓ 5 issues proposées, à relire" },
  ],
  error: null,
  paused: false,
  deploy: null,
  proposals: drafts.map((d, i) => toProposal(d, i + 1)),
};

function emitState() {
  if (!run) return;
  const decided = run.proposals.filter((p) => p.state !== "ready").length;
  mockBus.emit("agent://state", {
    run: run.run,
    state: run.state,
    step: decided,
    steps: run.proposals.length,
  });
}

function log(kind: AgentEventKind, text: string) {
  if (!run) return;
  run.log.push({ kind, text });
  mockBus.emit("agent://event", { run: run.run, kind, text, payload: null });
}

const script: [AgentEventKind, string][] = [
  ["read", "read  README.md"],
  ["read", "glob  docs/**/*.md"],
  ["read", 'grep  "repo_cache"'],
  ["message", "Le sélecteur de dépôt suppose un seul dépôt ouvert à la fois."],
  ["read", "read  src-tauri/src/agents/engine.rs"],
  ["message", "Le journal des agents vit dans le cache local, sans partage."],
];

function settle() {
  if (!run) return;
  const waiting = run.proposals.some((p) => p.state === "ready" || p.state === "failed");
  if (run.state === "review" && !waiting) {
    run.state = "done";
    run.endedAt = new Date().toISOString();
  }
  emitState();
}

export const ceoMock = {
  latest: () => run,

  start(version?: string | null): string {
    if (run && (run.state === "running" || run.state === "prepare")) return run.run;
    const target = version ?? "0.2.0";
    if (!data.versions.some((v) => v.title === target && v.state === "open"))
      throw new Error(`La version ${target} n'est pas un milestone ouvert du dépôt.`);
    for (const p of run?.proposals ?? [])
      if (p.state === "ready" || p.state === "failed") {
        p.state = "skipped";
        p.error = "Remplacée par l'exécution suivante.";
      }
    run = {
      run: `ceo-${Date.now()}`,
      kind: "ceo",
      state: "running",
      autonomy: "propose",
      version: target,
      model: data.config.agents.ceo.model,
      startedAt: new Date().toISOString(),
      endedAt: null,
      items: [],
      tasks: [],
      log: [],
      error: null,
      paused: false,
      deploy: null,
      proposals: [],
    };
    const current = run;
    log("system", `› copie de ${data.project.repo.fullName} (main)…`);
    log("system", `› ${target} : 3 thèmes, ${data.issues.length} issues lues, README.md`);
    emitState();
    // Comme Rust : une proposition identique à une issue existante (déjà créée) est écartée.
    const pending = drafts
      .filter((d) => !data.issues.some((i) => i.title.toLowerCase() === d.title.toLowerCase()))
      .map((d) => ({ ...d, milestone: d.milestone === "0.2.0" ? target : d.milestone }));
    let step = 0;
    const tick = () => {
      if (run !== current || current.state !== "running") return;
      if (step < script.length) {
        const [kind, text] = script[step++]!;
        log(kind, text);
      } else if (pending.length > 0) {
        const draft = pending.shift()!;
        current.proposals.push(toProposal(draft, current.proposals.length + 1));
        log("success", `✓ proposée : ${draft.title}`);
        emitState();
      } else {
        const n = current.proposals.length;
        current.state = n > 0 ? "review" : "done";
        current.endedAt = new Date().toISOString();
        log(
          "success",
          n > 0
            ? `✓ ${n} issue${n > 1 ? "s" : ""} proposée${n > 1 ? "s" : ""}, à relire`
            : "✓ rien ne manque : chaque thème est déjà couvert",
        );
        emitState();
        return;
      }
      timer = setTimeout(tick, STEP_MS);
    };
    timer = setTimeout(tick, STEP_MS);
    return current.run;
  },

  stop() {
    if (!run) return;
    if (timer) clearTimeout(timer);
    run.state = run.proposals.some((p) => p.state === "ready") ? "review" : "discarded";
    run.endedAt = new Date().toISOString();
    log("system", "› arrêtée à votre demande");
    emitState();
  },

  apply(ids: string[]): ApplyReport {
    const report: ApplyReport = { applied: [], failed: [] };
    for (const id of ids) {
      const p = run?.proposals.find((p) => `propose-${p.index}` === id);
      if (!p || (p.state !== "ready" && p.state !== "failed")) {
        report.failed.push({ id, message: "Pas de proposition à créer." });
        continue;
      }
      const number = Math.max(...data.issues.map((i) => i.number)) + 1;
      const row: IssueRow = {
        number,
        title: p.title,
        version: p.milestone,
        issueType: p.issueType,
        status: "todo",
        labels: ["agent/ceo", ...p.labels],
        assignee: null,
        estimate: null,
        activeAgent: null,
        linkedPr: null,
        flags: [],
        updatedAt: new Date().toISOString(),
      };
      data.issues.unshift(row);
      p.state = "applied";
      p.issue = number;
      p.issueUrl = `https://github.com/${data.project.repo.fullName}/issues/${number}`;
      log("write", `write #${number} : issue créée dans ${p.milestone} (« ${p.title} »)`);
      report.applied.push(id);
    }
    settle();
    return report;
  },

  reject(indexes: number[]) {
    for (const p of run?.proposals ?? [])
      if (indexes.includes(p.index) && (p.state === "ready" || p.state === "failed"))
        p.state = "skipped";
    settle();
  },

  edit(index: number, title: string, body: string) {
    const p = run?.proposals.find(
      (p) => p.index === index && (p.state === "ready" || p.state === "failed"),
    );
    if (!p) throw new Error(`Aucune proposition ${index} à modifier.`);
    if (!title.trim()) throw new Error("Le titre ne peut pas être vide.");
    if (!body.trim()) throw new Error("La description ne peut pas être vide.");
    p.title = title.trim();
    p.body = body.trim();
    emitState();
  },
};
