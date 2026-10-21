import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Markdown } from "./Markdown";

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
});
