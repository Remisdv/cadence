/**
 * Onboarding 3/3 — Projet et mapping (maquette Onboarding-Projet.dc.html).
 *
 * Choix du dépôt, aperçu de ce que Cadence importera (milestones → versions, sprint en cours),
 * rappel de la correspondance avec les objets GitHub, puis ouverture du projet.
 * Rien n'est écrit sur GitHub à cette étape.
 */
import { useEffect, useState, type ReactNode } from "react";
import type { ProjectPreview, Repo } from "@/lib/types";
import { OnboardingLayout } from "@/components/layout/OnboardingLayout";
import { Icon, type IconName } from "@/components/icons/Icon";
import { VersionBadge } from "@/components/domain/Badges";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { nextVersions, versionStage } from "@/lib/domain/versions";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";
import { useOpenProject, useProjectPreview, useRepos } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";

export default function ProjectView() {
  const [query, setQuery] = useState("");
  const repos = useRepos(useDebouncedValue(query));
  const [selected, setSelected] = useState<string | null>(null);
  const preview = useProjectPreview(selected);
  const open = useOpenProject();
  const navigate = useNavigation((s) => s.navigate);
  const replace = useNavigation((s) => s.replace);

  // Sélectionne le premier résultat tant que rien (de visible) n'est choisi.
  const list = repos.data;
  useEffect(() => {
    if (list?.length && !list.some((r) => r.fullName === selected)) {
      setSelected(list[0]!.fullName);
    }
  }, [list, selected]);

  const openProject = () => {
    if (!selected) return;
    // `replace` : le retour arrière ne ramène pas dans l'onboarding.
    open.mutate(selected, { onSuccess: () => replace({ id: "sprint" }) });
  };

  return (
    <OnboardingLayout
      step={2}
      title="Choisir un projet et vérifier le mapping"
      intro="Cadence n'invente aucun champ : il lit et écrit des objets GitHub existants. Vérifiez la correspondance avant d'ouvrir le projet — elle reste modifiable dans les réglages."
    >
      <div className="flex max-w-[1000px] gap-6">
        <div className="flex w-[400px] shrink-0 flex-col">
          <label className="flex h-9 items-center gap-2 rounded-lg border border-line2 bg-p2 px-[11px] text-tx3 focus-within:border-clay">
            <Icon name="search" size={15} />
            <input
              aria-label="Filtrer les dépôts"
              placeholder="Filtrer les dépôts…"
              value={query}
              autoFocus
              onChange={(e) => setQuery(e.target.value)}
              className="min-w-0 grow bg-transparent text-body text-tx outline-none placeholder:text-tx3"
            />
            {repos.isFetching && <Spinner size={13} />}
          </label>

          <SectionLabel className="mt-3">
            Vos dépôts{list ? ` · ${list.length} résultat${list.length > 1 ? "s" : ""}` : ""}
          </SectionLabel>

          <div className="mt-1.5 flex flex-col gap-0.5">
            {repos.isPending ? (
              <div className="flex h-24 items-center justify-center">
                <Spinner />
              </div>
            ) : repos.isError ? (
              <Callout tone="danger">{repos.error.message}</Callout>
            ) : list?.length === 0 ? (
              <p className="px-3 py-4 text-body-sm text-tx3">
                Aucun dépôt ne correspond à « {query} ».
              </p>
            ) : (
              list?.map((repo) => (
                <RepoItem
                  key={repo.fullName}
                  repo={repo}
                  selected={repo.fullName === selected}
                  onSelect={() => setSelected(repo.fullName)}
                />
              ))
            )}
          </div>
        </div>

        <div className="flex min-w-0 grow flex-col">
          <DetectionCard
            preview={preview.data}
            loading={preview.isPending && selected !== null}
            error={preview.error}
          />

          <SectionLabel className="mt-4">Correspondance des objets</SectionLabel>
          <MappingCard />

          <NumberingCard current={preview.data?.currentVersion ?? null} />

          {open.error && (
            <Callout tone="danger" className="mt-4">
              {open.error.message}
            </Callout>
          )}

          <div className="mt-[18px] flex items-center gap-3.5">
            <Button
              variant="primary"
              size="lg"
              iconRight={open.isPending ? undefined : "arrowRight"}
              disabled={!preview.data || open.isPending}
              onClick={openProject}
            >
              {open.isPending && <Spinner className="text-clay-ink" />}
              Ouvrir le projet
            </Button>
            <button
              type="button"
              className="text-body text-tx3 hover:text-tx"
              onClick={() => navigate({ id: "onboarding-agents" })}
            >
              Retour
            </button>
          </div>
        </div>
      </div>
    </OnboardingLayout>
  );
}

/* ---------- Dépôts ---------- */

function RepoItem({
  repo,
  selected,
  onSelect,
}: {
  repo: Repo;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        "flex w-full items-start gap-[11px] rounded-field border px-3 py-[11px] text-left",
        selected ? "border-line2 bg-p3" : "border-transparent hover:bg-p2",
      )}
    >
      <Avatar size={26} tone="neutral" initials={initialsOf(repo.owner)} />
      <div className="min-w-0 grow">
        <div
          className={cn(
            "flex items-center gap-1.5 text-body",
            selected ? "font-semibold" : "font-medium",
          )}
        >
          <span className="truncate">{repo.fullName}</span>
          {repo.private && <Badge tone="neutral">privé</Badge>}
        </div>
        <div className="mt-0.5 truncate text-caption text-tx3">
          {repo.description ?? "Sans description"}
        </div>
      </div>
      <span className="mt-[3px] mono text-tiny whitespace-nowrap text-tx3">
        {repo.milestones} milestone{repo.milestones > 1 ? "s" : ""}
      </span>
    </button>
  );
}

/* ---------- Ce qui sera importé ---------- */

function DetectionCard({
  preview,
  loading,
  error,
}: {
  preview?: ProjectPreview;
  loading: boolean;
  error: Error | null;
}) {
  let body;
  if (loading) {
    body = (
      <div className="flex items-center gap-2 text-body-sm text-tx3">
        <Spinner /> Lecture des milestones…
      </div>
    );
  } else if (error) {
    body = <Callout tone="danger">{error.message}</Callout>;
  } else if (!preview) {
    body = <div className="text-body-sm text-tx3">Choisissez un dépôt.</div>;
  } else if (preview.versions.length === 0) {
    body = (
      <>
        <Header icon="warning" tone="text-amber">
          Aucun milestone semver dans ce dépôt
        </Header>
        <p className="mt-2 text-caption leading-[1.6] text-tx3">
          Vous créerez la première version depuis Cadence, au format{" "}
          <span className="mono text-tx2">0.1.0</span>.
        </p>
      </>
    );
  } else {
    const n = preview.versions.length;
    body = (
      <>
        <Header icon="check" tone="text-green">
          {n} milestone{n > 1 ? "s" : ""} détecté{n > 1 ? "s" : ""} — importé{n > 1 ? "s" : ""}{" "}
          comme versions
        </Header>
        <div className="mt-[13px] flex flex-wrap gap-[7px]">
          {preview.versions.map((v) => (
            <VersionBadge
              key={v.title}
              version={v.title}
              tag
              stage={versionStage(v, preview.currentVersion)}
            />
          ))}
          {preview.currentVersion && (
            <span className="self-center text-caption text-tx3">
              → {preview.currentVersion} devient le sprint en cours
            </span>
          )}
        </div>
      </>
    );
  }

  return (
    <Card className="px-5 py-[18px]">
      {body}
      {preview && !loading && !error && (
        <div className="mt-3 flex flex-col gap-1 border-t border-line pt-3 text-caption text-tx3">
          {preview.ignoredMilestones.length > 0 && (
            <span>
              Ignoré{preview.ignoredMilestones.length > 1 ? "s" : ""} (titre non semver) :{" "}
              <span className="text-tx2">{preview.ignoredMilestones.join(", ")}</span>
            </span>
          )}
          <span>
            <span className="mono text-tx2">.cadence/config.toml</span>{" "}
            {preview.hasConfig
              ? "trouvé : ses réglages seront repris."
              : "absent : les réglages par défaut s'appliquent."}
          </span>
        </div>
      )}
    </Card>
  );
}

function Header({ icon, tone, children }: { icon: IconName; tone: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <span className={cn("flex", tone)}>
        <Icon name={icon} size={16} />
      </span>
      <span className="text-body font-medium">{children}</span>
    </div>
  );
}

/* ---------- Correspondance et numérotation ---------- */

const mapping: { icon: IconName; notion: string; github: string }[] = [
  { icon: "sprint", notion: "Version / sprint", github: "Milestone" },
  { icon: "tag", notion: "Version livrée", github: "Tag Git + Release" },
  { icon: "issue", notion: "Type d'issue", github: "Label type/*" },
  { icon: "layers", notion: "État d'avancement", github: "Label status/*" },
  { icon: "bot", notion: "Trace d'agent", github: "Label agent/* + commentaire" },
];

function MappingCard() {
  return (
    <Card className="mt-2 px-[18px] py-1">
      {mapping.map((row, i) => (
        <div
          key={row.notion}
          className={cn(
            "flex items-center gap-3 py-3",
            i < mapping.length - 1 && "border-b border-line",
          )}
        >
          <span className="flex text-tx3">
            <Icon name={row.icon} size={16} />
          </span>
          <span className="w-[140px] shrink-0 text-body-sm">{row.notion}</span>
          <span className="flex text-tx3">
            <Icon name="arrowRight" size={14} />
          </span>
          <span className="grow mono text-body-sm text-clay">{row.github}</span>
          <Badge tone="neutral">modifiable</Badge>
        </div>
      ))}
    </Card>
  );
}

function NumberingCard({ current }: { current: string | null }) {
  const base = current ?? "0.1.0";
  const [patch, minor, major] = nextVersions(base) ?? ["0.1.1", "0.2.0", "1.0.0"];
  return (
    <Card className="mt-4 px-[18px] py-4">
      <div className="text-body-sm font-medium">Schéma de numérotation</div>
      <div className="mt-[11px] flex flex-wrap items-center gap-2.5 text-body">
        <VersionBadge version={base} tone="muted" />
        <span className="text-tx3">→ sprint suivant →</span>
        <VersionBadge version={patch} stage="current" />
        <span className="text-line2">|</span>
        <span className="text-tx3">palier →</span>
        <VersionBadge version={minor} stage="milestone" />
        <span className="text-tx3">ou</span>
        <VersionBadge version={major} stage="vision" />
      </div>
      <div className="mt-[11px] text-caption leading-[1.6] text-tx3">
        Le troisième segment s'incrémente à chaque clôture de sprint. Le passage à un palier se
        décide à la main, au moment de clôturer.
      </div>
    </Card>
  );
}
