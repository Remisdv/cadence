import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Markdown, MarkdownInline } from "./Markdown";

const html = (source: string) => renderToStaticMarkup(<Markdown source={source} breaks />);

describe("Markdown", () => {
  it("rend le code en gras sans accents graves (recette P07)", () => {
    const out = html("Dépôt **`Remisdv/linear-test`** suivi.");
    expect(out).toContain("<strong");
    expect(out).toMatch(/<strong[^>]*><code[^>]*>Remisdv\/linear-test<\/code><\/strong>/);
    expect(out).not.toContain("`");
  });

  it("lit aussi le code et les liens dans l'italique et le gras", () => {
    expect(html("*voir `a.ts`*")).toMatch(/<em>voir <code[^>]*>a\.ts<\/code><\/em>/);
    expect(html("**[Roadmap](cadence:gestion-projet/roadmap)**")).toMatch(/<strong[^>]*><a /);
    expect(html("`**pas du gras**`")).toMatch(/<code[^>]*>\*\*pas du gras\*\*<\/code>/);
  });

  it("ne coupe ni les en-têtes ni les mots courts d'un tableau (recette R03)", () => {
    const out = html(
      [
        "| Issue | SP | Base |",
        "| --- | --- | --- |",
        "| #26 — Option `--bis` | 1 | `release/0.3.0` |",
        "| #27 | 1 | C:/Users/x/AppData/Roaming/app.cadence.desktop.qa/repos/linear-test |",
      ].join("\n"),
    );
    expect(out).not.toContain("break-all");
    // Défilement horizontal du tableau, cellules coupées entre les mots seulement.
    expect(out).toMatch(
      /<div class="[^"]*overflow-x-auto[^"]*"><table class="[^"]*wrap-break-word/,
    );
    const heads = [...out.matchAll(/<th class="([^"]*)">/g)].map((m) => m[1]);
    expect(heads).toHaveLength(3);
    for (const c of heads) expect(c).toContain("whitespace-nowrap");
    const cellsOf = [...out.matchAll(/<td class="([^"]*)">/g)].map((m) => m[1]!);
    expect(cellsOf.filter((c) => c.includes("wrap-anywhere"))).toHaveLength(1);
    expect(cellsOf[2]).not.toContain("wrap-anywhere");
    expect(cellsOf[5]).toContain("wrap-anywhere");
  });

  it("rend le code en ligne d'un message de l'hôte sans accents graves (R08)", () => {
    const out = renderToStaticMarkup(<MarkdownInline text="`ui/index.html` absent." />);
    expect(out).toBe('<code class="mono text-small text-clay">ui/index.html</code> absent.');
  });
});
