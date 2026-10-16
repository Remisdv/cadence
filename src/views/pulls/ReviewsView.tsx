/**
 * Revues à faire (maquette Reviews.dc.html) : à gauche les PR qui attendent votre revue et
 * celles qui attendent quelqu'un d'autre, à droite la PR choisie, son diff et ses fichiers.
 *
 * Lecture seule : commenter, demander des changements et approuver arrivent avec l'écriture ;
 * le résumé reviendra comme action « Résumer » du plugin Gestion de projet.
 */
import { useMemo, useState } from "react";
import type { PrDetail, PrFile, PrRow, ReviewRow } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import { Page } from "@/components/layout/Page";
import { EmptyState, ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon } from "@/components/icons/Icon";
import { VersionBadge } from "@/components/domain/Badges";
import { DiffViewer } from "@/components/domain/Diff";
import { CiStatus, Diffstat } from "@/components/domain/Inline";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { cn } from "@/lib/cn";
import { formatAgo } from "@/lib/domain/format";
import { prStateMeta } from "@/lib/domain/issues";
import { isOpenPr, parsePatch, waitingFor } from "@/lib/domain/prs";
import { openExternal } from "@/lib/platform";
import {
  useIssues,
  usePendingReviews,
  usePrDetail,
  usePullRequests,
} from "@/lib/query/queries";
import { useUiStore } from "@/stores/ui";

export default function ReviewsView({ view }: ViewProps<"reviews">) {
  const reviews = usePendingReviews({ live: true });
  const openDialog = useUiStore((s) => s.openDialog);
  const [picked, setPicked] = useState<number | null>(null);

  const rows = useMemo(() => reviews.data ?? [], [reviews.data]);
  const mine = rows.filter((r) => r.audience === "you");
  const others = rows.filter((r) => r.audience === "others");
  const selected = picked ?? view.pr ?? mine[0]?.pr.number ?? others[0]?.pr.number ?? null;
  const detail = usePrDetail(selected);

  const subtitle = reviews.data
    ? `${mine.length} PR attend${mine.length > 1 ? "ent" : ""} votre revue · ${others.length} attend${others.length > 1 ? "ent" : ""} quelqu'un d'autre`
    : undefined;

  return (
    <Page
      title="Revues à faire"
      subtitle={subtitle}
      actions={
        <>
          <Button
            icon="github"
            disabled={!detail.data}
            onClick={() => detail.data && openExternal(detail.data.url)}
          >
            Ouvrir sur GitHub
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openDialog("issue-create")}>
            Nouvelle issue
          </Button>
        </>
      }
      className="flex gap-3.5"
    >
      <div className="flex w-[346px] shrink-0 flex-col gap-3 overflow-y-auto">
        {reviews.isPending ? (
          <LoadingState label="Lecture des revues…" />
        ) : reviews.isError ? (
          <ErrorState error={reviews.error} />
        ) : (
          <>
            <ReviewGroup
              title="Vous êtes relecteur"
              count={mine.length}
              rows={mine}
              selected={selected}
              onSelect={setPicked}
              empty="Aucune revue ne vous est demandée."
            />
            <ReviewGroup
              title="En attente des autres"
              rows={others}
              selected={selected}
              onSelect={setPicked}
              empty="Aucune de vos PR n'attend de revue."
            />
          </>
        )}
      </div>

      <div className="flex min-w-0 grow flex-col gap-3">
        {selected === null ? (
          <Card className="grow">
            <EmptyState icon="review" title="Rien à relire">
              Les PR dont vous êtes relecteur, et vos PR qui attendent une revue, apparaîtront ici.
            </EmptyState>
          </Card>
        ) : detail.isPending ? (
          <Card className="grow">
            <LoadingState label={`Lecture de la PR #${selected}…`} />
          </Card>
        ) : detail.isError ? (
          <Card className="grow">
            <ErrorState error={detail.error} />
          </Card>
        ) : (
          <PrReview key={selected} detail={detail.data} />
        )}
      </div>
    </Page>
  );
}

function ReviewGroup({
  title,
  count,
  rows,
  selected,
  onSelect,
  empty,
}: {
  title: string;
  count?: number;
  rows: ReviewRow[];
  selected: number | null;
  onSelect: (pr: number) => void;
  empty: string;
}) {
  return (
    <section>
      <div className="flex items-center gap-2">
        <SectionLabel>{title}</SectionLabel>
        {count !== undefined && count > 0 && <span className="mono text-tiny text-red">{count}</span>}
      </div>
      <div className="mt-2 flex flex-col gap-2">
        {rows.length === 0 ? (
          <p className="text-caption text-tx3">{empty}</p>
        ) : (
          rows.map((row) => (
            <ReviewCard
              key={row.pr.number}
              row={row}
              active={row.pr.number === selected}
              onClick={() => onSelect(row.pr.number)}
            />
          ))
        )}
      </div>
    </section>
  );
}

function ReviewCard({ row, active, onClick }: { row: ReviewRow; active: boolean; onClick: () => void }) {
  const { pr } = row;
  const wait = waitingFor(row.requestedAt);
  const status =
    pr.state === "draft"
      ? { text: "bientôt prête", cls: "text-tx3" }
      : { text: `en attente · ${wait.label}`, cls: wait.tone === "red" ? "text-red" : "text-amber" };
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "block w-full rounded-[10px] border px-3.5 py-[13px] text-left",
        active ? "border-line2 bg-p3" : "border-line bg-p2 hover:bg-p3",
      )}
    >
      <div className="flex items-center gap-2">
        <span className="mono text-tiny text-tx3">#{pr.number}</span>
        <div className="grow" />
        <span className={cn("text-tiny", status.cls)}>{status.text}</span>
      </div>
      <div className="mt-1.5 text-body leading-[1.45]">{pr.title}</div>
      <div className="mt-[9px] flex min-w-0 items-center gap-[9px] text-tiny text-tx3">
        <Icon name="user" size={12} />
        {pr.author.isSelf ? "vous" : initialsOf(pr.author.name ?? pr.author.login)}
        {" · "}
        {pr.changesRequested > 0 ? (
          <span className="text-amber">
            {pr.changesRequested} changement{pr.changesRequested > 1 ? "s" : ""} demandé
            {pr.changesRequested > 1 ? "s" : ""}
          </span>
        ) : (
          <>
            <Icon name="branch" size={12} />
            <span className="truncate mono">{pr.branch}</span>
          </>
        )}
      </div>
      <div className="mt-[9px] flex items-center gap-2.5">
        <Diffstat additions={pr.additions} deletions={pr.deletions} />
        <span className="text-tiny text-tx3">
          {pr.changedFiles} fichier{pr.changedFiles > 1 ? "s" : ""}
        </span>
      </div>
    </button>
  );
}

function PrReview({ detail }: { detail: PrDetail }) {
  const { pr, files } = detail;
  const [path, setPath] = useState(() => (files.find((f) => f.patch) ?? files[0])?.path ?? null);
  const file = files.find((f) => f.path === path) ?? null;
  const reviewHint = "Les revues s'écrivent depuis l'app à partir de la version 0.1.2";

  return (
    <>
      <Card className="shrink-0 px-[17px] py-[15px]">
        <div className="flex items-center gap-[9px]">
          <Badge tone={prStateMeta[pr.state].tone} icon="pullRequest">
            {prStateMeta[pr.state].label.charAt(0).toUpperCase() + prStateMeta[pr.state].label.slice(1)}
          </Badge>
          <h3 className="min-w-0 truncate text-title-sm">{pr.title}</h3>
          <div className="grow" />
          <span className="mono text-small text-tx3">#{pr.number}</span>
        </div>
        <div className="mt-[9px] flex flex-wrap items-center gap-2.5 text-caption text-tx3">
          <Avatar
            size={19}
            tone={pr.author.isSelf ? "clay" : "neutral"}
            initials={initialsOf(pr.author.name ?? pr.author.login)}
            src={pr.author.avatarUrl}
          />
          {pr.author.isSelf ? "Vous" : (pr.author.name ?? pr.author.login)} · ouverte{" "}
          {formatAgo(pr.createdAt)} ·
          <span className="inline-flex items-center gap-1">
            <span className="mono">{pr.branch}</span>
            <Icon name="arrowRight" size={12} />
            <span className="mono">{pr.base}</span>
          </span>
          · <CiStatus state={pr.ci} passed={pr.ciPassed} total={pr.ciTotal || undefined} />
          {pr.version && (
            <>
              · <VersionBadge version={pr.version} />
            </>
          )}
        </div>
      </Card>

      <div className="flex min-h-0 grow gap-3">
        <Card className="flex min-w-0 grow flex-col overflow-hidden">
          {file ? (
            file.patch ? (
              <DiffViewer
                file={file.path}
                additions={file.additions}
                deletions={file.deletions}
                lines={parsePatch(file.patch)}
              />
            ) : (
              <EmptyState icon="file" title={file.path}>
                Pas d'aperçu : fichier binaire, renommé sans changement ou trop gros pour GitHub.
              </EmptyState>
            )
          ) : (
            <EmptyState icon="file" title="Aucun fichier modifié" />
          )}
          <div className="mt-auto flex h-[42px] shrink-0 items-center gap-2 border-t border-line bg-p2 px-3.5">
            <Button icon="review" disabled title={reviewHint}>
              Commenter
            </Button>
            <div className="grow" />
            <Button variant="danger" icon="close" disabled title={reviewHint}>
              Demander des changements
            </Button>
            <Button variant="success" icon="check" disabled title={reviewHint}>
              Approuver
            </Button>
          </div>
        </Card>

        <div className="flex w-[300px] shrink-0 flex-col gap-3 overflow-y-auto">
          <Card className="p-3.5">
            <div className="text-small font-semibold">Description</div>
            {detail.body?.trim() ? (
              <p className="mt-2.5 text-small leading-[1.65] whitespace-pre-wrap text-tx2">
                {detail.body.trim()}
              </p>
            ) : (
              <p className="mt-2.5 text-small text-tx3">Aucune description.</p>
            )}
          </Card>

          <Card className="p-3.5">
            <SectionLabel>Fichiers · {files.length}</SectionLabel>
            <div className="mt-[9px] flex flex-col gap-[7px]">
              {files.map((f) => (
                <FileRow key={f.path} file={f} active={f.path === path} onClick={() => setPath(f.path)} />
              ))}
            </div>
          </Card>

          <VersionImpact pr={pr} />
        </div>
      </div>
    </>
  );
}

function FileRow({ file, active, onClick }: { file: PrFile; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "flex items-center gap-2 rounded-control px-2 py-1.5 text-left",
        active ? "bg-p3" : "hover:bg-p2",
      )}
    >
      <span className="flex text-tx3">
        <Icon name="file" size={13} />
      </span>
      <span className="min-w-0 grow truncate mono text-caption" title={file.path}>
        {file.path}
      </span>
      <Diffstat additions={file.additions} deletions={file.deletions} plain className="text-micro" />
    </button>
  );
}

/** Effet de la fusion sur la version de la PR : issues closes, clôture encore bloquée. */
function VersionImpact({ pr }: { pr: PrRow }) {
  const issues = useIssues().data;
  const prs = usePullRequests().data;
  if (!pr.version || !issues || !isOpenPr(pr)) return null;

  const scope = issues.filter((i) => i.version === pr.version);
  const closed = scope.filter((i) => i.status === "done").length;
  const closes = scope.some((i) => i.number === pr.linkedIssue && i.status !== "done") ? 1 : 0;
  const stillOpen = scope.length - closed - closes;
  const openPrs = (prs ?? []).filter((p) => isOpenPr(p) && p.version === pr.version).length;

  return (
    <Card className="p-3.5">
      <SectionLabel>Impact version</SectionLabel>
      <div className="mt-2.5 flex items-center gap-2 text-small text-tx2">
        <Icon name="tag" size={14} />
        {openPrs <= 1 ? "Dernière PR ouverte de" : `${openPrs} PR ouvertes sur`}
        <VersionBadge version={pr.version} />
      </div>
      <p className="mt-[9px] text-caption leading-[1.6] text-tx3">
        {closes > 0
          ? `Une fois fusionnée, ${closed + closes} des ${scope.length} issues de la version seront closes.`
          : `Elle ne ferme aucune issue ouverte de la version (${closed} sur ${scope.length} closes).`}{" "}
        {stillOpen > 0
          ? `La clôture de version restera bloquée tant que ${stillOpen} issue${stillOpen > 1 ? "s sont ouvertes" : " est ouverte"}.`
          : "La version pourra ensuite être clôturée."}
      </p>
    </Card>
  );
}
