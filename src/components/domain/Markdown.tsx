/**
 * Rendu du markdown des issues (sous-ensemble) : titres `##`, paragraphes, listes `-`/`1.`,
 * cases `- [ ]` (lecture seule : cocher écrirait sur GitHub), tableaux `| a | b |`, `**gras**`, `*italique*`,
 * `` `code` ``, liens. Le HTML brut n'est jamais interprété. `onLink` intercepte un clic sur un
 * lien (liens `cadence:` du chat) : il renvoie `true` s'il l'a traité.
 */
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

type Block =
  | { kind: "heading"; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "tasks"; items: { done: boolean; text: string }[] }
  | { kind: "quote"; text: string }
  | { kind: "code"; text: string }
  | { kind: "table"; head: string[]; rows: string[][] };

/** Cellules d'une ligne `| a | b |` (barres de bord facultatives). */
function cells(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => c.trim());
}

/** Mot assez long (chemin, URL) pour être coupé n'importe où dans une cellule de tableau. */
const LONG_WORD = /\S{32,}/;

const TABLE_RULE = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;

/** `breaks` : un simple retour à la ligne reste une ligne (messages du chat). */
export function parseMarkdown(source: string, { breaks = false } = {}): Block[] {
  const blocks: Block[] = [];
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (!line.trim()) {
      i++;
    } else if (line.startsWith("```")) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i]!.startsWith("```")) code.push(lines[i++]!);
      i++;
      blocks.push({ kind: "code", text: code.join("\n") });
    } else if (line.includes("|") && TABLE_RULE.test(lines[i + 1] ?? "")) {
      const head = cells(line);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i]!.includes("|") && lines[i]!.trim())
        rows.push(cells(lines[i++]!));
      blocks.push({ kind: "table", head, rows });
    } else if (/^#{1,6}\s/.test(line)) {
      blocks.push({ kind: "heading", text: line.replace(/^#+\s*/, "") });
      i++;
    } else if (/^\s*[-*]\s+\[[ xX]\]\s/.test(line)) {
      const items: { done: boolean; text: string }[] = [];
      while (i < lines.length && /^\s*[-*]\s+\[[ xX]\]\s/.test(lines[i]!)) {
        const m = /^\s*[-*]\s+\[([ xX])\]\s+(.*)$/.exec(lines[i++]!)!;
        items.push({ done: m[1] !== " ", text: m[2]! });
      }
      blocks.push({ kind: "tasks", items });
    } else if (/^\s*([-*+]|\d+\.)\s/.test(line)) {
      const ordered = /^\s*\d+\./.test(line);
      const items: string[] = [];
      while (
        i < lines.length &&
        /^\s*([-*+]|\d+\.)\s/.test(lines[i]!) &&
        !/\[[ xX]\]/.test(lines[i]!.slice(0, 8))
      ) {
        items.push(lines[i++]!.replace(/^\s*([-*+]|\d+\.)\s+/, ""));
      }
      blocks.push({ kind: "list", ordered, items });
    } else if (line.startsWith(">")) {
      const quote: string[] = [];
      while (i < lines.length && lines[i]!.startsWith(">"))
        quote.push(lines[i++]!.replace(/^>\s?/, ""));
      blocks.push({ kind: "quote", text: quote.join(" ") });
    } else {
      const para: string[] = [];
      while (
        i < lines.length &&
        lines[i]!.trim() &&
        !/^(#{1,6}\s|```|>|\s*([-*+]|\d+\.)\s)/.test(lines[i]!) &&
        !(lines[i]!.includes("|") && TABLE_RULE.test(lines[i + 1] ?? ""))
      ) {
        para.push(lines[i++]!.trim());
      }
      blocks.push({ kind: "paragraph", text: para.join(breaks ? "\n" : " ") });
    }
  }
  return blocks;
}

/**
 * `**gras**`, `*italique*`, `` `code` ``, `[texte](url)` ; le reste est du texte. Le gras et
 * l'italique se lisent à leur tour (`` **`owner/name`** `` : code en gras, sans accents graves).
 */
type LinkHandler = (href: string) => boolean;

function inlineWith(text: string, onLink?: LinkHandler): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const t = m[0];
    const key = `${m.index}`;
    if (t.startsWith("`"))
      out.push(
        <code key={key} className="mono text-small text-clay">
          {t.slice(1, -1)}
        </code>,
      );
    else if (t.startsWith("**"))
      out.push(
        <strong key={key} className="font-semibold text-tx">
          {inlineWith(t.slice(2, -2), onLink)}
        </strong>,
      );
    else if (t.startsWith("*")) out.push(<em key={key}>{inlineWith(t.slice(1, -1), onLink)}</em>);
    else {
      const [, label, href] = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(t)!;
      out.push(
        <a
          key={key}
          href={href}
          target="_blank"
          rel="noreferrer"
          onClick={(e) => {
            if (onLink?.(href!)) e.preventDefault();
          }}
          className="text-blue underline-offset-2 hover:underline"
        >
          {label}
        </a>,
      );
    }
    last = m.index + t.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({
  source,
  className,
  onLink,
  breaks,
}: {
  source: string;
  className?: string;
  onLink?: LinkHandler;
  breaks?: boolean;
}) {
  const blocks = parseMarkdown(source, { breaks });
  const inline = (text: string) => inlineWith(text, onLink);
  return (
    <div className={cn("flex flex-col gap-3 text-body leading-[1.72] text-tx2", className)}>
      {blocks.map((b, i) => {
        switch (b.kind) {
          case "heading":
            return (
              <p key={i} className="font-semibold text-tx">
                {inline(b.text)}
              </p>
            );
          case "paragraph":
            return (
              <p key={i} className={cn(breaks && "whitespace-pre-line")}>
                {inline(b.text)}
              </p>
            );
          case "quote":
            return (
              <p key={i} className="border-l-2 border-line2 pl-3">
                {inline(b.text)}
              </p>
            );
          case "code":
            return (
              <pre
                key={i}
                className="overflow-x-auto rounded-lg bg-console px-3 py-2 mono text-small text-tx2"
              >
                {b.text}
              </pre>
            );
          case "list": {
            const List = b.ordered ? "ol" : "ul";
            return (
              <List
                key={i}
                className={cn("flex flex-col gap-1 pl-5", b.ordered ? "list-decimal" : "list-disc")}
              >
                {b.items.map((item, j) => (
                  <li key={j}>{inline(item)}</li>
                ))}
              </List>
            );
          }
          case "table":
            // Les cellules ne se coupent qu'entre les mots (`wrap-break-word`, même sous un
            // parent en `wrap-anywhere`) et les en-têtes jamais : une colonne trop étroite
            // élargit le tableau, qui défile alors dans son cadre (recette R03 : « SP » en
            // « S / P »). Seule une cellule à très long mot (chemin, URL) se coupe n'importe où.
            return (
              <div key={i} className="max-w-full overflow-x-auto rounded-lg border border-line">
                <table className="w-full border-collapse text-body-sm wrap-break-word">
                  <thead className="bg-p2 text-left text-tx">
                    <tr>
                      {b.head.map((cell, j) => (
                        <th
                          key={j}
                          className="border-b border-line px-3 py-1.5 font-semibold whitespace-nowrap"
                        >
                          {inline(cell)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {b.rows.map((row, r) => (
                      <tr key={r} className={cn(r < b.rows.length - 1 && "border-b border-line")}>
                        {b.head.map((_, j) => (
                          <td
                            key={j}
                            className={cn(
                              "px-3 py-1.5 align-top",
                              LONG_WORD.test(row[j] ?? "") && "min-w-[14rem] wrap-anywhere",
                            )}
                          >
                            {inline(row[j] ?? "")}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          case "tasks":
            return (
              <div key={i} className="flex flex-col gap-[7px]">
                {b.items.map((item, j) => (
                  <label key={j} className="flex items-start gap-[9px] text-body-sm text-tx2">
                    <input
                      type="checkbox"
                      checked={item.done}
                      readOnly
                      disabled
                      className="mt-0.5 size-[13px] accent-clay"
                    />
                    <span className={cn(item.done && "text-tx3 line-through")}>
                      {inline(item.text)}
                    </span>
                  </label>
                ))}
              </div>
            );
        }
      })}
    </div>
  );
}
