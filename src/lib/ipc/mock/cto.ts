/**
 * Simulation de l'agent CTO en mode mock : une file d'issues traitée une par une, avec des
 * lignes de console qui défilent, des propositions et des questions. Même contrat que Rust.
 */
import type { AgentEventKind, AgentRunDetail, CtoAnswer, CtoItem } from "@/lib/types";
import { mockBus } from "./bus";
import * as data from "./fixtures";

let run: AgentRunDetail | null = null;
/** Descriptions écrites par « Appliquer », pour le détail d'issue. */
const appliedBodies = new Map<number, string>();
let timer: ReturnType<typeof setTimeout> | null = null;

const STEP_MS = 450;

function emitState() {
  if (!run) return;
  const done = run.items.filter((i) => i.state !== "queued" && i.state !== "running").length;
  mockBus.emit("agent://state", { run: run.run, state: run.state, step: done, steps: run.items.length });
}

function log(kind: AgentEventKind, text: string) {
  if (!run) return;
  run.log.push({ kind, text });
  mockBus.emit("agent://event", { run: run.run, kind, text, payload: null });
}

function proposalFor(item: CtoItem): string {
  return [
    "## Contexte",
    `${item.title}. Aujourd'hui, le comportement n'est pas défini : le code concerné vit dans \`src/\` et ne couvre pas ce cas.`,
    "",
    "## Attendu",
    "- Décrire le comportement attendu dans l'interface",
    "- Gérer le cas d'erreur sans message brut",
    "- Garder le cache local cohérent",
    "",
    "## Critères d'acceptation",
    "- [ ] Le cas nominal est couvert par un test",
    "- [ ] Aucun message d'erreur brut n'est affiché",
  ].join("\n");
}

const script: [AgentEventKind, string][] = [
  ["read", "read  src/api/github/mod.rs"],
  ["read", "read  src/lib/ipc/errors.ts"],
  ["read", 'grep  "ratelimit"'],
  ["message", "Le client remonte l'erreur telle quelle jusqu'à la vue."],
];

/** Traite la prochaine issue en file, puis enchaîne. */
function next() {
  if (!run || run.state !== "running") return;
  const item = run.items.find((i) => i.state === "queued");
  if (!item) {
    run.state = run.items.some((i) => i.state === "ready" || i.state === "questions") ? "review" : "done";
    if (run.state === "done") run.endedAt = new Date().toISOString();
    log("system", "› file terminée");
    emitState();
    return;
  }
  item.state = "running";
  emitState();
  log("system", `› #${item.number} ${item.title}`);
  let step = 0;
  const tick = () => {
    if (!run || item.state !== "running") return;
    if (step < script.length) {
      const [kind, text] = script[step++]!;
      log(kind, text);
      timer = setTimeout(tick, STEP_MS);
      return;
    }
    finish(item, item.number % 2 === 0 && item.questions.length === 0);
    timer = setTimeout(next, STEP_MS);
  };
  timer = setTimeout(tick, STEP_MS);
}

function finish(item: CtoItem, ask: boolean) {
  item.after = proposalFor(item);
  item.estimate = item.number % 3 === 0 ? "L" : "M";
  item.estimateHours = item.estimate === "L" ? 8 : 4;
  item.labels = ["area/api"];
  item.risk = "faible";
  item.related = [129];
  item.questions = ask
    ? [
        { id: "q1", question: "Le bandeau reste-t-il visible tant que le quota n'est pas rétabli ?", answer: null },
        { id: "q2", question: "Les agents suspendus reprennent-ils seuls ?", answer: null },
      ]
    : [];
  item.state = ask ? "questions" : "ready";
  if (ask) log("question", "? 2 points restent ambigus");
  else log("success", `✓ description rédigée (${item.after.length} car.)`);
  emitState();
}

export const ctoMock = {
  latest: () => run,
  appliedBody: (number: number) => appliedBodies.get(number),
  item: (number: number) => run?.items.find((i) => i.number === number),

  start(only?: number[]): string {
    if (run && (run.state === "running" || run.state === "prepare")) {
      if (only) {
        const known = run.items.map((i) => i.number);
        run.items.push(...toItems(only.filter((n) => !known.includes(n))));
        log("system", `› ajoutée(s) à la file : ${only.map((n) => `#${n}`).join(", ")}`);
        emitState();
      }
      return run.run;
    }
    const current = data.project.currentVersion;
    const pending = run?.items.filter((i) => i.state === "ready" || i.state === "questions") ?? [];
    const fresh = toItems(
      only ??
        data.issues
          .filter((i) => i.version === current && i.status !== "done" && !i.flags.includes("cto-reviewed"))
          .slice(0, 6)
          .map((i) => i.number),
    ).filter((i) => !pending.some((p) => p.number === i.number));
    const items: CtoItem[] = [...pending, ...fresh];
    run = {
      run: `cto-${Date.now()}`,
      kind: "cto",
      state: "running",
      autonomy: data.config.agents.cto.autonomy,
      version: only ? null : current,
      model: data.config.agents.cto.model,
      startedAt: new Date().toISOString(),
      endedAt: null,
      items,
      tasks: [],
      log: [],
      error: null,
      paused: false,
      deploy: null,
    };
    log("system", `› copie de ${data.project.repo.fullName} (main)…`);
    log("system", `› ${items.length} issue${items.length > 1 ? "s" : ""} à traiter`);
    emitState();
    timer = setTimeout(next, STEP_MS);
    return run.run;
  },


  stop() {
    if (!run) return;
    if (timer) clearTimeout(timer);
    for (const item of run.items) if (item.state === "running") item.state = "queued";
    run.state = run.items.some((i) => i.state === "ready" || i.state === "questions") ? "review" : "discarded";
    run.endedAt = new Date().toISOString();
    log("system", "› arrêtée à votre demande");
    emitState();
  },

  apply(ids: string[]) {
    const applied: string[] = [];
    const failed: { id: string; message: string }[] = [];
    for (const id of ids) {
      const item = run?.items.find((i) => `update-${i.number}` === id);
      if (!item || item.state !== "ready") {
        failed.push({ id, message: "Pas de proposition prête." });
        continue;
      }
      item.state = "applied";
      item.before = item.after ?? "";
      appliedBodies.set(item.number, item.before);
      const issue = data.issues.find((i) => i.number === item.number);
      if (issue) {
        issue.flags = ["cto-reviewed"];
        issue.estimate = item.estimate;
        issue.labels = [...new Set([...issue.labels, ...item.labels])];
      }
      log("write", `write #${item.number} : description et labels`);
      applied.push(id);
    }
    settle();
    return { applied, failed };
  },

  reject(numbers: number[]) {
    for (const item of run?.items ?? []) if (numbers.includes(item.number)) item.state = "skipped";
    settle();
  },

  edit(number: number, body: string) {
    const item = run?.items.find((i) => i.number === number);
    if (item) item.after = body;
    emitState();
  },

  answer(number: number, answers: CtoAnswer[]) {
    const item = run?.items.find((i) => i.number === number && i.state === "questions");
    if (!item) throw new Error(`L'agent n'attend pas de réponse sur #${number}.`);
    for (const a of answers) {
      const q = item.questions.find((q) => q.id === a.questionId);
      if (q) q.answer = a.answer.trim() || null;
    }
    item.state = "running";
    log("system", `› réponses reçues pour #${number}`);
    emitState();
    setTimeout(() => {
      if (!run) return;
      item.after = `${proposalFor(item)}\n- [ ] ${item.questions.map((q) => q.answer).filter(Boolean).join(" ; ")}`;
      item.questions = item.questions.filter((q) => q.answer);
      item.state = "ready";
      log("success", "✓ description complétée avec vos réponses");
      settle();
    }, 1200);
  },
};

function toItems(numbers: number[]): CtoItem[] {
  return data.issues
    .filter((i) => numbers.includes(i.number))
    .map((i) => ({
      number: i.number,
      title: i.title,
      version: i.version,
      issueType: i.issueType,
      state: "queued",
      before: i.flags.includes("incomplete-description")
        ? ""
        : "Quand on dépasse le quota, l'app affiche une erreur brute. Il faut quelque chose de lisible.",
      after: null,
      questions: [],
      estimate: null,
      estimateHours: null,
      labels: [],
      risk: null,
      related: [],
      error: null,
    }));
}

function settle() {
  if (!run) return;
  const busy = run.items.some((i) => i.state === "queued" || i.state === "running");
  const waiting = run.items.some((i) => i.state === "ready" || i.state === "questions");
  if (run.state === "review" && !busy && !waiting) {
    run.state = "done";
    run.endedAt = new Date().toISOString();
  }
  emitState();
}
