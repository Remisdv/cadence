import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { ChatThread, CommandCard } from "./ChatThread";
import { buildThread, type ChatItem } from "@/lib/domain/chat";
import type { AiEvent } from "@/lib/types";
import { MOCK_QUESTIONS } from "@/lib/ipc/mock/chat";

const LONG =
  'cd "C:/Users/lhuil/AppData/Roaming/app.cadence.desktop/repos/.worktrees/remisdv/go-alternance-crm-poc/extraction-issues" && ls -la && echo "--- TREE ---" && find . -path ./.git -prune -o -type f -print | head -100';

const thread = (events: AiEvent[]): ChatItem[] =>
  buildThread(events.map((event, i) => ({ seq: i + 1, turn: 1, at: "", event })));

const items = thread([
  { type: "user-message", text: "Regarde le dépôt" },
  { type: "message-completed", message: "m:0", text: "Je regarde l'arborescence." },
  { type: "command", call: "c1", command: LONG, exitCode: 0, output: "total 48" },
  { type: "command", call: "c2", command: "git status --short", exitCode: 0, output: " M a.go" },
  {
    type: "tool-started",
    call: "t1",
    source: { kind: "builtin" },
    name: "Read",
    input: { file_path: "a.go" },
  },
  { type: "tool-completed", call: "t1", ok: true, summary: null, output: null },
  { type: "file-changed", path: "src/a.go", change: "modified" },
  { type: "file-changed", path: "src/b.go", change: "added" },
  { type: "message-completed", message: "m:1", text: "Deux fichiers corrigés." },
  { type: "turn-completed", ok: true, interrupted: false, error: null, durationMs: 4000 },
]);

const render = (showCommands: boolean) =>
  renderToStaticMarkup(<ChatThread items={items} ai="Claude Code" showCommands={showCommands} />);

/** Échappement d'attribut de `renderToStaticMarkup`. */
const attr = (text: string) =>
  text.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("'", "&#x27;");

/** Attribut `class` de la première balise qui commence par `marker` (`details `, `summary `…). */
function classOf(html: string, marker: string): string {
  const tag = html.split("<").find((t) => t.startsWith(marker) || t.includes(` ${marker}`));
  expect(tag, `balise avec ${marker}`).toBeDefined();
  return /class="([^"]*)"/.exec(tag!)?.[1] ?? "";
}

describe("ChatThread : commandes masquées", () => {
  it("montre par défaut chaque commande, outil et fichier", () => {
    const html = render(true);
    expect(html).toContain("git status --short");
    expect(html).toContain("src/b.go");
    expect(html).not.toContain("2 commandes · 1 outil");
  });

  it("résume l'activité du tour en une ligne repliée, messages gardés", () => {
    const html = render(false);
    expect(html).toContain("2 commandes · 1 outil · 2 fichiers modifiés");
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain("git status --short");
    expect(html).not.toContain("src/b.go");
    expect(html).toContain("Je regarde l&#x27;arborescence.");
    expect(html).toContain("Deux fichiers corrigés.");
    // Le résumé prend la place de la première commande, entre les deux messages.
    const at = (text: string) => html.indexOf(text);
    expect(at("Je regarde")).toBeLessThan(at("2 commandes"));
    expect(at("2 commandes")).toBeLessThan(at("Deux fichiers"));
  });

  it("ne résume rien pour un tour sans activité", () => {
    const quiet = thread([
      { type: "user-message", text: "Bonjour" },
      { type: "message-completed", message: "m:0", text: "Bonjour !" },
    ]);
    const html = renderToStaticMarkup(
      <ChatThread items={quiet} ai="Claude Code" showCommands={false} />,
    );
    expect(html).not.toContain("aria-expanded");
  });
});

describe("CommandCard : une commande très longue reste dans le fil", () => {
  const card = thread([
    { type: "command", call: "c1", command: LONG, exitCode: 0, output: "total 48\n./go.mod" },
  ])[0] as Extract<ChatItem, { kind: "command" }>;
  const html = renderToStaticMarkup(<CommandCard item={card} />);

  it("tronque la ligne de résumé (ellipse, rétrécissable)", () => {
    const summary = classOf(html, `title="${attr(LONG)}"`);
    expect(summary.split(" ")).toEqual(expect.arrayContaining(["min-w-0", "truncate"]));
    expect(summary.split(" ")).not.toContain("shrink-0");
    expect(classOf(html, "summary ").split(" ")).toContain("min-w-0");
  });

  it("dépliée, coupe la commande n'importe où et fait défiler la sortie", () => {
    const pres = html.split("<pre").slice(1);
    expect(pres).toHaveLength(2);
    const [command, output] = pres.map((p) => /class="([^"]*)"/.exec(p)?.[1]?.split(" ") ?? []);
    expect(command).toEqual(expect.arrayContaining(["break-all", "whitespace-pre-wrap"]));
    expect(output).toEqual(expect.arrayContaining(["overflow-auto", "whitespace-pre", "min-w-0"]));
  });

  it("borne la carte et son contenu à la largeur du fil", () => {
    expect(classOf(html, "details ").split(" ")).toContain("min-w-0");
  });
});

describe("carte de questions à choix", () => {
  const questions = MOCK_QUESTIONS;
  const asked: AiEvent = {
    type: "question",
    request: "q1",
    text: questions.map((q) => q.question).join("\n"),
    options: [],
    questions,
  };

  it("montre chaque question, ses choix (unique ou multiple), « Autre » et Envoyer", () => {
    const html = renderToStaticMarkup(
      <ChatThread items={thread([asked])} ai="Claude Code" onAnswer={() => {}} />,
    );
    expect(html).toContain("2 questions de l&#x27;IA");
    expect(html.match(/role="radio"/g)).toHaveLength(2);
    expect(html.match(/role="checkbox"/g)).toHaveLength(3);
    expect(html).toContain("Plusieurs choix possibles");
    expect(html.match(/placeholder="Autre réponse…"/g)).toHaveLength(2);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Envoyer/s);
  });

  it("une fois répondue, montre les réponses données", () => {
    const html = renderToStaticMarkup(
      <ChatThread
        items={thread([
          asked,
          {
            type: "question-answered",
            request: "q1",
            answer: "Format : Résumé · Sections : Formules",
            answers: [
              { id: questions[0]!.id, answers: ["Résumé"] },
              { id: questions[1]!.id, answers: ["Formules", "Exercices"] },
            ],
          },
        ])}
        ai="Claude Code"
      />,
    );
    expect(html).toContain("Répondu");
    expect(html).toContain("Formules, Exercices");
    expect(html).not.toContain("Envoyer");
    expect(html).not.toContain('role="radio"');
  });
});

describe("carte d'accord d'un outil de plugin (N12)", () => {
  it("nomme l'outil et son plugin, avec les arguments résumés", () => {
    const card = thread([
      {
        type: "approval-requested",
        request: "r1",
        kind: "tool",
        title: "Gestion de projet › merge_pull_request · pull_number: 22, delete_branch: true",
        detail: { pull_number: 22, delete_branch: true },
        tool: "mcp__cadence__gestion-projet__merge_pull_request",
      },
    ]);
    const html = renderToStaticMarkup(<ChatThread items={card} ai="Claude Code" />);
    expect(html).toContain("cet outil de plugin");
    expect(html).toContain("merge_pull_request · pull_number: 22, delete_branch: true");
    expect(html).toContain("gestion-projet");
    expect(html).toContain("Autoriser pour la session");
  });
});

describe("ChatThread : tour arrêté pendant un accord (recette du 21/10, R05)", () => {
  const stopped = thread([
    { type: "user-message", text: "Développe le sprint" },
    {
      type: "tool-started",
      call: "t1",
      source: { kind: "cadence", plugin: "gestion-projet", tool: "get_repository" },
      name: "mcp__cadence__gestion-projet__get_repository",
      input: { repo: "o/n" },
    },
    {
      type: "approval-requested",
      request: "r1",
      kind: "tool",
      title: "Gestion de projet › get_repository",
      detail: {},
      tool: "mcp__cadence__gestion-projet__get_repository",
      scope: "outil gestion-projet__get_repository",
    },
    {
      type: "tool-completed",
      call: "t1",
      ok: false,
      summary:
        "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file).",
      output: null,
    },
    { type: "turn-completed", ok: false, interrupted: true, error: null, durationMs: 48000 },
  ]);
  const html = renderToStaticMarkup(
    <ChatThread items={stopped} ai="Claude Code" showCommands onApprove={() => {}} />,
  );

  it("ferme la carte d'accord restée en attente (« Annulée », sans boutons)", () => {
    expect(html).toContain("Annulée");
    expect(html).not.toContain("Autoriser pour la session");
    expect(html).not.toContain("Refuser");
  });

  it("dit « Tour arrêté » en français, sans le message de la CLI", () => {
    expect(html).toContain("Tour arrêté");
    expect(html).not.toContain("doesn&#x27;t want to proceed");
    expect(html).not.toContain("error_during_execution");
    expect(html).toContain("Annulé : le tour a été arrêté.");
  });

  it("un ancien tour en échec montre la clé traduite", () => {
    const old = thread([
      { type: "user-message", text: "x" },
      {
        type: "turn-completed",
        ok: false,
        interrupted: false,
        error: "Tour en échec (error_during_execution).",
        durationMs: 0,
      },
    ]);
    const out = renderToStaticMarkup(<ChatThread items={old} ai="Claude Code" showCommands />);
    expect(out).toContain("Le tour s&#x27;est arrêté sur une erreur de Claude Code.");
    expect(out).not.toContain("error_during_execution");
  });
});
