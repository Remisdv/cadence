/**
 * Simulation de l'agent déploiement en mode mock (maquette Agent-Deploy) : cinq étapes sur la
 * version 0.1.1, un test d'acceptation par issue, un échec sur #128 à trancher, puis la
 * publication de la release en attente de votre validation. Même contrat que Rust.
 */
import type {
  AgentEventKind,
  AgentRunDetail,
  ApplyReport,
  DeployState,
  DeployTest,
  IssueType,
} from "@/lib/types";
import { mockBus } from "./bus";
import * as data from "./fixtures";

let run: AgentRunDetail | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
const STEP_MS = 650;

/** Issues closes de 0.1.1 (la version de la maquette), avec leurs critères d'acceptation. */
const delivered: {
  number: number;
  title: string;
  type: IssueType;
  criteria: string[];
  fails?: string;
}[] = [
  {
    number: 128,
    title: "La palette de commandes s'ouvre au raccourci global",
    type: "feat",
    criteria: [
      "⌘K ouvre la palette depuis n'importe quel écran",
      "Le raccourci survit à une mise en veille",
    ],
    fails: "Le raccourci ne se réenregistre pas après une mise en veille sur macOS.",
  },
  {
    number: 127,
    title: "Les PR d'un milestone apparaissent dans la vue Revues",
    type: "feat",
    criteria: ["Une PR du milestone apparaît dans Revues"],
  },
  {
    number: 125,
    title: "Le cache SQLite survit à un redémarrage",
    type: "feat",
    criteria: ["Les issues sont affichées hors ligne au lancement"],
  },
  {
    number: 124,
    title: "Une issue sans version ne peut pas être créée",
    type: "feat",
    criteria: ["Le bouton Créer reste inactif sans version"],
  },
  {
    number: 122,
    title: "La liste des issues se recharge après une action",
    type: "fix",
    criteria: ["La liste reflète l'action sans rechargement"],
  },
  {
    number: 120,
    title: "Le jeton est relu depuis le trousseau au lancement",
    type: "fix",
    criteria: ["Aucune reconnexion demandée au lancement"],
  },
  {
    number: 118,
    title: "La fenêtre restaure sa taille précédente",
    type: "fix",
    criteria: ["La taille est la même après redémarrage"],
  },
];

function emitState() {
  if (!run?.deploy) return;
  mockBus.emit("agent://state", {
    run: run.run,
    state: run.state,
    step: run.deploy.step,
    steps: 5,
  });
}

function log(kind: AgentEventKind, text: string) {
  if (!run) return;
  run.log.push({ kind, text });
  mockBus.emit("agent://event", { run: run.run, kind, text, payload: null });
}

const inNotes = (t: DeployTest) =>
  t.status === "passed" ||
  t.status === "unverifiable" ||
  (t.status === "failed" && t.decision === "ignored");

const sections: [string, (IssueType | null)[]][] = [
  ["Nouveautés", ["feat"]],
  ["Corrections", ["fix"]],
  ["Performances", ["perf"]],
  ["Documentation", ["docs"]],
  ["Maintenance", ["chore", null]],
];

/** Même rédaction que Rust (`deploy::release_notes`). */
export function releaseNotes(tag: string, name: string | null, tests: DeployTest[]): string {
  let out = name ? `## ${tag} — ${name}\n` : `## ${tag}\n`;
  let any = false;
  for (const [title, types] of sections) {
    const lines = tests
      .filter((t) => inNotes(t) && types.includes(t.issueType))
      .map((t) => `- ${t.title} (#${t.number})`);
    if (lines.length > 0) {
      any = true;
      out += `\n### ${title}\n${lines.join("\n")}\n`;
    }
  }
  return any ? out : `${out}\nAucune issue livrée sur cette version.\n`;
}

function regenerate(dp: DeployState) {
  if (dp.notes !== null && !dp.notesEdited && !dp.published)
    dp.notes = releaseNotes(dp.tag, dp.name, dp.tests);
}

function settle() {
  if (!run?.deploy) return;
  const dp = run.deploy;
  const waiting =
    !dp.published || dp.tests.some((t) => t.status === "failed" && t.decision === null);
  if (run.state === "review" && !waiting) {
    run.state = "done";
    run.endedAt = new Date().toISOString();
  }
  emitState();
}

function stage(n: number, state: DeployState["stages"][number]["state"], detail: string) {
  if (!run?.deploy) return;
  run.deploy.stages[n - 1] = { state, detail };
}

/** Avance d'un cran : une étape, ou un test pendant l'étape 3. */
function tick() {
  if (!run?.deploy || run.state !== "running") return;
  const dp = run.deploy;
  if (run.paused) {
    timer = setTimeout(tick, STEP_MS);
    return;
  }
  switch (dp.step) {
    case 0:
      dp.step = 1;
      stage(1, "running", "lecture du tag et de la release…");
      break;
    case 1:
      stage(
        1,
        "success",
        `${dp.tests.length} issues fermées · ${dp.tests.length} avec critères · brouillon présent`,
      );
      log("system", `› pré-vol ${dp.tag} : brouillon présent`);
      dp.releaseUrl = `https://github.com/${data.project.repo.fullName}/releases/tag/untagged-${dp.tag}`;
      dp.step = 2;
      dp.worktree = `C:/Users/vous/.cadence/wt/cadence-app/desktop/deploy-${dp.tag.replace(/\./g, "-")}`;
      stage(2, "running", `worktree sur ${dp.tag}…`);
      log("system", `› worktree ~/.cadence/wt/deploy-${dp.tag}`);
      log("read", "bash  pnpm install --frozen-lockfile");
      log("read", "bash  pnpm tauri build");
      break;
    case 2:
      dp.artifacts = [
        { path: "src-tauri/target/release/bundle/dmg/Cadence_0.1.1_aarch64.dmg", sizeKb: 18_842 },
        {
          path: "src-tauri/target/release/bundle/nsis/Cadence_0.1.1_x64-setup.exe",
          sizeKb: 21_606,
        },
        {
          path: "src-tauri/target/release/bundle/appimage/cadence_0.1.1_amd64.AppImage",
          sizeKb: 25_293,
        },
      ];
      stage(2, "success", "macOS arm64, Windows x64, Linux AppImage");
      log("success", "✓ build réussi (3 artefacts)");
      dp.step = 3;
      stage(3, "running", `0 / ${dp.tests.length}`);
      break;
    case 3: {
      const test = dp.tests.find((t) => t.status === "queued" || t.status === "running");
      if (test && test.status === "queued") {
        test.status = "running";
        log("system", `› test #${test.number} ${test.title}`);
        break;
      }
      if (test) {
        const spec = delivered.find((d) => d.number === test.number);
        test.status = spec?.fails ? "failed" : "passed";
        test.detail = spec?.fails ?? "Chaque critère vérifié sur le binaire construit.";
        test.durationMs = 800;
        log(
          spec?.fails ? "error" : "success",
          `${spec?.fails ? "✕" : "✓"} #${test.number} ${test.detail}`,
        );
        const done = dp.tests.filter((t) => t.status !== "queued" && t.status !== "running").length;
        stage(3, "running", `${done} / ${dp.tests.length}`);
        break;
      }
      const failed = dp.tests.filter((t) => t.status === "failed");
      const passed = dp.tests.filter((t) => t.status === "passed").length;
      stage(
        3,
        failed.length ? "failure" : "success",
        [
          `${passed} / ${dp.tests.length}`,
          ...(failed.length
            ? [`${failed.length} échec sur ${failed.map((t) => `#${t.number}`).join(", ")}`]
            : []),
        ].join(" · "),
      );
      dp.step = 4;
      stage(4, "running", "rédaction des notes…");
      break;
    }
    case 4: {
      dp.notes = releaseNotes(dp.tag, dp.name, dp.tests);
      const kept = dp.tests.filter(inNotes).length;
      const removed = dp.tests.length - kept;
      stage(
        4,
        "success",
        `Notes rédigées · ${kept} issues${removed ? ` · ${removed} retirée${removed > 1 ? "s" : ""}` : ""}`,
      );
      dp.step = 5;
      stage(5, "running", `${dp.tag} · en attente de votre validation`);
      run.state = "review";
      log("question", `? relisez les notes et les tests, puis publiez ${dp.tag}`);
      emitState();
      return;
    }
  }
  emitState();
  timer = setTimeout(tick, STEP_MS);
}

export const deployMock = {
  latest: () => run,

  start(version?: string | null): string {
    if (run && (run.state === "running" || run.state === "prepare"))
      throw new Error("L'agent déploiement travaille déjà : attendez la fin ou arrêtez-le.");
    const target = version ?? "0.1.1";
    const v = data.versions.find((x) => x.title === target);
    if (!v) throw new Error(`La version ${target} n'existe pas sur ce dépôt.`);
    // La clôture mockée ne ferme pas le milestone des fixtures : la version en cours passe aussi.
    if (v.state !== "closed" && target !== data.project.currentVersion)
      throw new Error(`La version ${target} n'est pas close : clôturez-la avant de la déployer.`);
    const tag = `${data.config.project.tagPrefix}${target}`;
    run = {
      run: `deploy-${Date.now()}`,
      kind: "deploy",
      state: "running",
      autonomy: data.config.agents.deploy.autonomy,
      version: target,
      model: data.config.agents.deploy.model,
      startedAt: new Date().toISOString(),
      endedAt: null,
      items: [],
      tasks: [],
      log: [],
      error: null,
      paused: false,
      deploy: {
        tag,
        name: v.name,
        nextVersion: data.versions.find((x) => x.state === "open")?.title ?? null,
        stages: Array.from({ length: 5 }, () => ({ state: "pending" as const, detail: null })),
        step: 0,
        tests: delivered.map((d) => ({
          number: d.number,
          title: d.title,
          issueType: d.type,
          criteria: d.criteria,
          status: "queued" as const,
          detail: null,
          durationMs: null,
          decision: null,
        })),
        artifacts: [],
        notes: null,
        notesEdited: false,
        releaseUrl: null,
        published: false,
        worktree: null,
      },
    };
    log("system", `› déploiement de ${tag}`);
    emitState();
    timer = setTimeout(tick, STEP_MS);
    return run.run;
  },

  apply(ids: string[]): ApplyReport {
    const report: ApplyReport = { applied: [], failed: [] };
    const dp = run?.deploy;
    for (const id of ids) {
      if (!run || !dp) {
        report.failed.push({ id, message: "Aucune exécution." });
        continue;
      }
      if (run.autonomy === "propose") {
        report.failed.push({
          id,
          message: "L'agent déploiement est réglé sur « propose seulement » : rien n'est écrit.",
        });
        continue;
      }
      if (id === "publish-release") {
        if (dp.published || dp.step < 5 || dp.notes === null) {
          report.failed.push({ id, message: "Rien à publier." });
          continue;
        }
        dp.published = true;
        dp.releaseUrl = `https://github.com/${data.project.repo.fullName}/releases/tag/${dp.tag}`;
        stage(5, "success", `${dp.tag} publiée`);
        log("write", `release ${dp.tag} publiée`);
        report.applied.push(id);
        continue;
      }
      const number = Number(id.replace("reopen-", ""));
      const test = dp.tests.find(
        (t) => t.number === number && t.status === "failed" && t.decision === null,
      );
      if (!id.startsWith("reopen-") || !test) {
        report.failed.push({ id, message: "Action inconnue." });
        continue;
      }
      test.decision = "reopened";
      regenerate(dp);
      log("write", `write #${number} : rouverte sur ${dp.nextVersion}`);
      report.applied.push(id);
    }
    settle();
    return report;
  },

  ignore(numbers: number[]) {
    const dp = run?.deploy;
    if (!dp) return;
    for (const t of dp.tests)
      if (numbers.includes(t.number) && t.status === "failed" && t.decision === null)
        t.decision = "ignored";
    regenerate(dp);
    settle();
  },

  editNotes(notes: string) {
    const dp = run?.deploy;
    if (!dp || dp.notes === null) throw new Error("Les notes ne sont pas encore rédigées.");
    if (dp.published) throw new Error("La release est déjà publiée : éditez-la sur GitHub.");
    if (!notes.trim()) throw new Error("Les notes de version ne peuvent pas être vides.");
    dp.notes = notes.trim();
    dp.notesEdited = true;
    emitState();
  },

  stop() {
    if (!run?.deploy) return;
    if (timer) clearTimeout(timer);
    const dp = run.deploy;
    dp.stages.forEach((s, i) => {
      if (s.state === "running" && i < 4)
        Object.assign(s, { state: "failure", detail: "Arrêté à votre demande." });
    });
    for (const t of dp.tests) if (t.status === "running") t.status = "queued";
    run.state = dp.step >= 5 ? "review" : "discarded";
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
