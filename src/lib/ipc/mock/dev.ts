/**
 * Simulation de l'agent développeur en mode mock : six étapes par tâche, console qui défile,
 * push en attente de validation (autonomie « applique après validation »).
 */
import type { AgentEventKind, AgentRunDetail, DevTask } from "@/lib/types";
import { branchName } from "@/lib/domain/issues";
import { mockBus } from "./bus";
import * as data from "./fixtures";

let run: AgentRunDetail | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let prSeq = 90;
const STEP_MS = 700;

function emitState() {
  if (!run) return;
  const task = run.tasks.find((t) => t.state === "running");
  mockBus.emit("agent://state", { run: run.run, state: run.state, step: task?.step ?? 0, steps: 6 });
}

function log(kind: AgentEventKind, text: string) {
  if (!run) return;
  run.log.push({ kind, text });
  mockBus.emit("agent://event", { run: run.run, kind, text, payload: null });
}

const script: Record<number, [AgentEventKind, string][]> = {
  1: [
    ["read", "read  src-tauri/src/auth/mod.rs"],
    ["read", "read  src-tauri/src/commands/auth.rs"],
    ["message", "Le flux existant est réutilisable ; j'ajoute un module séparé."],
  ],
  3: [
    ["write", "write src-tauri/src/auth/device_flow.rs"],
    ["write", "edit  src-tauri/src/commands/auth.rs"],
    ["write", "edit  src/views/connect.tsx"],
  ],
  4: [
    ["read", "bash  cargo test"],
    ["message", "   Finished test in 12.4s — 64 passed"],
  ],
};

/** Résumé de l'étape courante. */
function note(task: DevTask, text: string) {
  task.stepNotes[Math.max(task.step, 1) - 1] = text;
}

function advance(task: DevTask) {
  if (!run || task.state !== "running") return;
  if (run.paused) {
    timer = setTimeout(() => advance(task), STEP_MS);
    return;
  }
  const lines = script[task.step] ?? [];
  for (const [kind, text] of lines) log(kind, text);
  switch (task.step) {
    case 1:
      note(task, "plan établi : module séparé, 3 fichiers visés");
      break;
    case 2:
      task.pr = prSeq++;
      task.prUrl = `https://github.com/cadence-app/desktop/pull/${task.pr}`;
      log("success", `✓ PR #${task.pr} ouverte (brouillon)`);
      note(task, `${task.branch} · PR #${task.pr}`);
      break;
    case 3:
      task.files = [
        { path: "src-tauri/src/auth/device_flow.rs", additions: 142, deletions: 0, added: true },
        { path: "src-tauri/src/commands/auth.rs", additions: 68, deletions: 12, added: false },
        { path: "src/views/connect.tsx", additions: 45, deletions: 13, added: false },
      ];
      task.additions = 255;
      task.deletions = 25;
      note(task, "3 fichier(s) : device_flow.rs, auth.rs, connect.tsx");
      break;
    case 4:
      task.summary = "Ajout du module `device_flow` et de la commande associée ; vue de connexion mise à jour.";
      note(task, "tests : réussis");
      log("success", "✓ travail commité dans le worktree");
      break;
  }
  if (task.step === 4) {
    task.step = 5;
    task.state = "review";
    note(task, "en attente de votre validation pour pousser");
    log("question", `? #${task.number} : relisez le diff puis validez le push`);
    emitState();
    timer = setTimeout(next, STEP_MS);
    return;
  }
  task.step += 1;
  emitState();
  timer = setTimeout(() => advance(task), STEP_MS);
}

function next() {
  if (!run || run.state !== "running") return;
  const task = run.tasks.find((t) => t.state === "queued");
  if (!task) {
    run.state = run.tasks.some((t) => t.state === "review") ? "review" : "done";
    if (run.state === "done") run.endedAt = new Date().toISOString();
    log("system", "› file terminée");
    emitState();
    return;
  }
  task.state = "running";
  task.step = 1;
  task.startedAt = new Date().toISOString();
  task.worktree = `C:/Users/vous/.cadence/wt/cadence-app/desktop/${task.number}`;
  log("system", `› #${task.number} ${task.title}`);
  log("system", `› worktree ~/.cadence/wt/${task.number}`);
  emitState();
  timer = setTimeout(() => advance(task), STEP_MS);
}

function settle() {
  if (!run) return;
  const busy = run.tasks.some((t) => t.state === "queued" || t.state === "running");
  const waiting = run.tasks.some((t) => t.state === "review");
  if (run.state === "review" && !busy && !waiting) {
    run.state = "done";
    run.endedAt = new Date().toISOString();
  }
  emitState();
}

export const devMock = {
  latest: () => run,

  start(issues: number[]): string {
    if (run && (run.state === "running" || run.state === "prepare"))
      throw new Error("L'agent développeur travaille déjà : attendez la fin ou arrêtez-le.");
    const tasks: DevTask[] = data.issues
      .filter((i) => issues.includes(i.number) && i.status !== "done")
      .map((i) => ({
        number: i.number,
        title: i.title,
        issueType: i.issueType,
        estimate: i.estimate,
        state: "queued",
        step: 0,
        stepNotes: [],
        branch: branchName(i.issueType, i.number, i.title),
        worktree: null,
        pr: null,
        prUrl: null,
        files: [],
        additions: 0,
        deletions: 0,
        summary: null,
        error: null,
        startedAt: null,
      }));
    run = {
      run: `dev-${Date.now()}`,
      kind: "dev",
      state: "running",
      autonomy: data.config.agents.dev.autonomy,
      version: null,
      model: data.config.agents.dev.model,
      startedAt: new Date().toISOString(),
      endedAt: null,
      items: [],
      tasks,
      log: [],
      error: null,
      paused: false,
      deploy: null,
    };
    log("system", `› copie de ${data.project.repo.fullName} (main)…`);
    emitState();
    timer = setTimeout(next, STEP_MS);
    return run.run;
  },

  push(ids: string[]) {
    const applied: string[] = [];
    const failed: { id: string; message: string }[] = [];
    for (const id of ids) {
      const task = run?.tasks.find((t) => `push-${t.number}` === id && t.state === "review");
      if (!task) {
        failed.push({ id, message: "Rien à pousser." });
        continue;
      }
      log("write", `push ${task.branch}`);
      note(task, "1 commit(s) poussé(s)");
      task.step = 6;
      log("success", `✓ PR #${task.pr} prête pour la revue`);
      note(task, `PR #${task.pr} sortie du brouillon`);
      task.state = "done";
      applied.push(id);
    }
    settle();
    return { applied, failed };
  },

  skip(numbers: number[]) {
    for (const t of run?.tasks ?? []) if (numbers.includes(t.number) && t.state !== "done") t.state = "skipped";
    settle();
  },

  stop() {
    if (!run) return;
    if (timer) clearTimeout(timer);
    for (const t of run.tasks) if (t.state === "running") {
      t.state = "skipped";
      t.error = "Arrêtée à votre demande.";
    }
    run.state = run.tasks.some((t) => t.state === "review") ? "review" : "discarded";
    run.endedAt = new Date().toISOString();
    log("system", "› arrêtée à votre demande");
    emitState();
  },

  pause(paused: boolean) {
    if (!run) return;
    run.paused = paused;
    log("system", paused ? "› pause demandée (après l'étape en cours)" : "› reprise");
    emitState();
  },

  message(text: string) {
    log("message", `vous › ${text}`);
  },
};
