/**
 * Clôturer la version (maquette Version-Bump.dc.html) : choix de la version suivante, sort
 * des issues encore ouvertes, aperçu exact de ce qui sera écrit sur GitHub. Non réversible.
 * L'agent déploiement (déclenché après la clôture) arrive en 0.2.1.
 */
import { useState, type ReactNode } from "react";
import type { CarryOver, CloseReport } from "@/lib/types";
import { DialogBody, DialogClose, DialogFooter, DialogHeader } from "@/components/ui/Dialog";
import { Input } from "@/components/ui/Field";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { RadioCard, RadioCardGroup, RadioOption } from "@/components/ui/RadioCard";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { SwitchField } from "@/components/ui/Switch";
import { Icon, type IconName } from "@/components/icons/Icon";
import { VersionBadge } from "@/components/domain/Badges";
import { EmptyState } from "@/components/layout/EmptyState";
import { formatRange } from "@/lib/domain/format";
import { isOpenPr } from "@/lib/domain/prs";
import { compareVersions, nextSprintRange, nextVersions, parseSemver } from "@/lib/domain/versions";
import { openExternal } from "@/lib/platform";
import {
  useCloseVersion,
  useCurrentProject,
  useIssues,
  usePullRequests,
  useVersions,
} from "@/lib/query/queries";

const choices = [
  { title: "Sprint suivant", description: "Incrémente le dernier segment. Le choix par défaut.", tone: "clay" },
  { title: "Nouveau palier", description: "Pour un ensemble de fonctionnalités qui change l'usage.", tone: "blue" },
  { title: "Version majeure", description: "Réservé à une ouverture publique ou une rupture.", tone: "purple" },
] as const;

export function VersionCloseDialog({ onDone }: { onDone: () => void }) {
  const project = useCurrentProject().data;
  const current = project?.currentVersion ?? null;
  const versions = useVersions().data ?? [];
  const issues = useIssues().data ?? [];
  const prs = usePullRequests().data ?? [];
  const close = useCloseVersion();

  const candidates = current ? nextVersions(current) : null;
  const [choice, setChoice] = useState("0");
  const [custom, setCustom] = useState("");
  const [carry, setCarry] = useState<CarryOver>("move-to-next");
  const [notes, setNotes] = useState(true);

  if (!current || !candidates) {
    return (
      <>
        <DialogHeader title="Clôturer la version" icon="tag" />
        <DialogBody>
          <EmptyState icon="tag" title="Aucune version en cours">
            La version en cours est le plus petit milestone daté encore ouvert du dépôt.
          </EmptyState>
        </DialogBody>
        <DialogFooter>
          <div className="grow" />
          <DialogClose asChild>
            <Button variant="ghost">Fermer</Button>
          </DialogClose>
        </DialogFooter>
      </>
    );
  }

  if (close.isSuccess) return <CloseDone report={close.data} onDone={onDone} />;

  const customSemver = custom.trim() ? parseSemver(custom.trim().replace(/^v/, "")) : null;
  const next = custom.trim() ? (customSemver ? customSemver.join(".") : null) : candidates[Number(choice)]!;
  const nextError =
    custom.trim() && !customSemver
      ? "Saisissez une version semver : 0.3.0"
      : next && compareVersions(next, current) <= 0
        ? `Doit être plus grande que ${current}.`
        : next && versions.some((v) => v.title === next && v.state === "closed")
          ? `La version ${next} est déjà close.`
          : null;

  const currentVersion = versions.find((v) => v.title === current);
  const existingNext = versions.find((v) => v.title === next);
  const range = nextSprintRange(currentVersion?.dueOn ?? null);
  const openIssues = issues.filter((i) => i.version === current && i.status !== "done");
  const openPrs = prs.filter((p) => isOpenPr(p) && p.version === current);
  const tag = `v${current}`;
  const branch = project?.repo.defaultBranch ?? "main";
  const n = openIssues.length;

  const carryLine: Record<CarryOver, string> = {
    "move-to-next": `${n} issue${n > 1 ? "s" : ""} déplacée${n > 1 ? "s" : ""} vers ${next ?? "…"}`,
    unversioned: `${n} issue${n > 1 ? "s" : ""} retirée${n > 1 ? "s" : ""} de la version`,
    "close-as-not-planned": `${n} issue${n > 1 ? "s" : ""} fermée${n > 1 ? "s" : ""} comme non retenue${n > 1 ? "s" : ""}`,
  };

  const submit = () => {
    if (!next || nextError) return;
    close.mutate({ next, carryOver: carry, generateNotes: notes });
  };

  return (
    <>
      <DialogHeader title={`Clôturer la version ${current}`} icon="tag" />
      <div className="flex gap-4 px-[18px] py-4">
        <div className="min-w-0 flex-1">
          <SectionLabel>Version suivante</SectionLabel>
          <RadioCardGroup
            value={custom.trim() ? "" : choice}
            onValueChange={(v) => {
              setChoice(v);
              setCustom("");
            }}
            aria-label="Version suivante"
            className="mt-2"
          >
            {candidates.map((title, i) => (
              <RadioCard
                key={title}
                value={String(i)}
                leading={<VersionBadge version={title} tone={choices[i]!.tone} />}
                title={choices[i]!.title}
                description={choices[i]!.description}
              />
            ))}
          </RadioCardGroup>
          <Input
            size="sm"
            mono
            aria-label="Version personnalisée"
            placeholder="ou saisir : 0.3.0"
            value={custom}
            invalid={Boolean(custom.trim() && nextError)}
            onChange={(e) => setCustom(e.target.value)}
            className="mt-2 w-full"
          />
          {nextError && <p className="mt-1.5 text-caption text-red">{nextError}</p>}
        </div>

        <div className="flex min-w-0 flex-1 flex-col gap-3.5">
          <div>
            <SectionLabel>
              {n === 0 ? "Aucune issue ouverte" : `${n} issue${n > 1 ? "s" : ""} encore ouverte${n > 1 ? "s" : ""}`}
            </SectionLabel>
            {n > 0 && (
              <RadioCardGroup
                value={carry}
                onValueChange={(v) => setCarry(v as CarryOver)}
                aria-label="Issues encore ouvertes"
                className="mt-2"
              >
                <RadioOption value="move-to-next">Reporter sur {next ?? "la version suivante"}</RadioOption>
                <RadioOption value="unversioned">Laisser sans version</RadioOption>
                <RadioOption value="close-as-not-planned">Fermer comme non retenues</RadioOption>
              </RadioCardGroup>
            )}
          </div>

          <div>
            <SectionLabel>Ce qui sera écrit sur GitHub</SectionLabel>
            <div className="mt-1">
              <WriteLine icon="check" tone="text-green">
                Milestone <span className="mono">{current}</span> fermé
              </WriteLine>
              {next && (
                <WriteLine icon="plus" tone="text-blue">
                  Milestone <span className="mono">{next}</span>{" "}
                  {existingNext?.dueOn
                    ? "repris tel quel"
                    : `${existingNext ? "daté" : "créé"} (${formatRange(range.start, range.end)})`}
                </WriteLine>
              )}
              {n > 0 && (
                <WriteLine icon="arrowRight" tone="text-blue">
                  {carryLine[carry]}
                </WriteLine>
              )}
              <WriteLine icon="tag" tone="text-clay">
                Tag <span className="mono">{tag}</span> posé sur <span className="mono">{branch}</span>
              </WriteLine>
              {notes && (
                <WriteLine icon="book" tone="text-amber">
                  Brouillon de release avec notes générées
                </WriteLine>
              )}
            </div>
          </div>
        </div>
      </div>

      {(openPrs.length > 0 || close.isError) && (
        <div className="flex flex-col gap-2.5 px-[18px] pb-4">
          {openPrs.length > 0 && (
            <Callout tone="warning">
              {openPrs.length > 1 ? `${openPrs.length} PR de cette version sont` : "Une PR de cette version est"}{" "}
              encore ouverte{openPrs.length > 1 ? "s" : ""} (
              {openPrs.map((p, i) => (
                <span key={p.number}>
                  {i > 0 && ", "}
                  <span className="mono">#{p.number}</span>
                </span>
              ))}
              ). Le tag sera posé sur l'état actuel de <span className="mono">{branch}</span> —{" "}
              {openPrs.length > 1 ? "elles atterriront" : "elle atterrira"} donc dans {next ?? "la version suivante"}.
            </Callout>
          )}
          {close.isError && <Callout tone="danger">{close.error.message}</Callout>}
        </div>
      )}

      <DialogFooter>
        <SwitchField checked={notes} onCheckedChange={setNotes} label="Générer les notes de version" />
        <div className="grow" />
        <DialogClose asChild>
          <Button variant="ghost" disabled={close.isPending}>
            Annuler
          </Button>
        </DialogClose>
        <Button
          variant="primary"
          icon={close.isPending ? undefined : "check"}
          disabled={!next || Boolean(nextError) || close.isPending}
          onClick={submit}
        >
          {close.isPending && <Spinner />}
          Clôturer et passer en {next ?? "…"}
        </Button>
      </DialogFooter>
    </>
  );
}

function WriteLine({ icon, tone, children }: { icon: IconName; tone: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-[9px] py-[7px] text-small text-tx2">
      <span className={`flex ${tone}`}>
        <Icon name={icon} size={13} />
      </span>
      <span>{children}</span>
    </div>
  );
}

function CloseDone({ report, onDone }: { report: CloseReport; onDone: () => void }) {
  return (
    <>
      <DialogHeader title={`Version ${report.closed} clôturée`} icon="tag" />
      <DialogBody>
        <Callout tone="success">
          Milestone <span className="mono">{report.closed}</span> fermé, tag{" "}
          <span className="mono">{report.tag}</span> posé
          {report.movedIssues > 0 &&
            `, ${report.movedIssues} issue${report.movedIssues > 1 ? "s" : ""} traitée${report.movedIssues > 1 ? "s" : ""}`}
          . La version en cours est maintenant <span className="mono">{report.opened}</span>.
        </Callout>
        {report.releaseUrl ? (
          <p className="text-body-sm text-tx2">
            Les notes de version attendent dans un brouillon de release : relisez-les puis publiez-les
            depuis GitHub.
          </p>
        ) : null}
      </DialogBody>
      <DialogFooter>
        <div className="grow" />
        {report.releaseUrl && (
          <Button icon="github" onClick={() => openExternal(report.releaseUrl!)}>
            Ouvrir le brouillon
          </Button>
        )}
        <Button variant="primary" onClick={onDone}>
          Terminer
        </Button>
      </DialogFooter>
    </>
  );
}
