import { Icon } from "@/components/icons/Icon";
import { cn } from "@/lib/cn";
import { Diffstat } from "./Inline";

export interface DiffLine {
  type: "ctx" | "add" | "del";
  text: string;
}

const lineClass = {
  ctx: "text-tx2",
  add: "bg-diff-add text-green",
  del: "bg-diff-del text-red",
};

export interface DiffViewerProps {
  file: string;
  additions: number;
  deletions: number;
  lines: DiffLine[];
}

/** Diff de code unifié (revue de PR) : gouttière +/- de 10px, sans numéros de ligne. */
export function DiffViewer({ file, additions, deletions, lines }: DiffViewerProps) {
  return (
    <div className="flex min-h-0 flex-col">
      <div className="flex h-[38px] shrink-0 items-center gap-[9px] border-b border-line px-3.5">
        <span className="flex text-tx3">
          <Icon name="file" size={15} />
        </span>
        <span className="grow truncate mono text-small">{file}</span>
        <Diffstat additions={additions} deletions={deletions} />
      </div>
      <div className="min-h-0 grow overflow-auto py-2">
        {lines.map((line, i) => (
          <div
            key={i}
            className={cn(
              "flex gap-2.5 px-2.5 py-px mono text-caption leading-[1.75] whitespace-pre",
              lineClass[line.type],
            )}
          >
            <span className="w-2.5 shrink-0 text-tx3">
              {line.type === "add" ? "+" : line.type === "del" ? "-" : ""}
            </span>
            {line.text}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Diff de texte (description proposée par l'agent CTO) : lignes qui passent à la ligne. */
export function TextDiff({ lines, className }: { lines: DiffLine[]; className?: string }) {
  return (
    <div className={cn("overflow-hidden rounded-[10px] border border-line", className)}>
      <div className="mono text-small leading-[1.8]">
        {lines.map((line, i) => (
          <div key={i} className={cn("px-3 py-0.5 whitespace-pre-wrap", lineClass[line.type])}>
            {line.type === "add" ? "+ " : line.type === "del" ? "− " : "  "}
            {line.text}
          </div>
        ))}
      </div>
    </div>
  );
}
