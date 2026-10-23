/**
 * Configuration (V2-Onboarding-Setup.dc.html, V2-Onboarding-Repos.dc.html) : les écrans de
 * configuration des plugins activés, à la suite, dans l'ordre du rail. Le service Dépôts
 * GitHub s'intercale à l'étape du premier plugin qui le déclare (`services = ["repos"]`) :
 * connexion GitHub, puis dépôts suivis ; viennent ensuite les secrets et les vues `[[setup]]`.
 */
import { useMemo, useState, type ReactNode } from "react";
import type { InstalledPluginView, Repo } from "@/lib/types";
import { cn } from "@/lib/cn";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { PluginTile } from "@/components/domain/PluginTile";
import { OnboardingActions, OnboardingFrame } from "@/components/layout/OnboardingFrame";
import { LoadingState } from "@/components/layout/EmptyState";
import { formatAgo } from "@/lib/domain/format";
import {
  parseRepoInput,
  setupTasks,
  stepIndex,
  taskKey,
  taskTitle,
  type SetupTask,
} from "@/lib/domain/onboarding";
import {
  useAppSettings,
  useAuthStatus,
  usePlugins,
  usePluginSecret,
  useRepoOwners,
  useRepos,
  useSaveAppSettings,
  useSetPluginSecret,
  useSignOut,
} from "@/lib/query/queries";
import { PluginFrameSlot } from "@/plugin-host/FramePool";
import {
  BrowserSignInCard,
  DeviceFlowCard,
  LinkButton,
  PermissionRow,
  permissions,
} from "../GithubView";
import { PatDialog } from "../PatDialog";
import type { StepProps } from "../OnboardingView";

export function SetupStep({ onNext, onBack }: StepProps) {
  const plugins = usePlugins();
  const tasks = useMemo(() => setupTasks(plugins.data ?? []), [plugins.data]);
  const [current, setCurrent] = useState<string | null>(null);

  const frame = (children: ReactNode) => (
    <OnboardingFrame
      step={stepIndex("setup")}
      title="Configurer vos plugins"
      intro="Chaque plugin apporte ses écrans de configuration, affichés à la suite dans l'ordre du rail. Les services fournis par Cadence, comme la connexion GitHub, s'intercalent à l'étape du plugin qui les utilise."
    >
      {children}
    </OnboardingFrame>
  );

  if (plugins.isPending) return frame(<LoadingState />);

  if (tasks.length === 0) {
    return frame(
      <div className="flex flex-col gap-3.5">
        <div className="rounded-card border border-dashed border-line2 px-4 py-6 text-center text-body-sm text-tx3">
          {(plugins.data ?? []).some((p) => p.enabled)
            ? "Vos plugins n'ont rien à configurer : ils sont prêts."
            : "Aucun plugin activé : rien à configurer pour l'instant."}
        </div>
        <OnboardingActions onBack={onBack}>
          <Button variant="primary" size="lg" iconRight="arrowRight" autoFocus onClick={onNext}>
            Continuer
          </Button>
        </OnboardingActions>
      </div>,
    );
  }

  const found = tasks.findIndex((t) => taskKey(t) === current);
  const index = found < 0 ? 0 : found;
  const task = tasks[index]!;
  const following = tasks[index + 1] ?? null;
  const goTo = (i: number) => {
    if (i < 0) onBack();
    else if (i >= tasks.length) onNext();
    else setCurrent(taskKey(tasks[i]!));
  };
  const nav: TaskNav = {
    next: () => goTo(index + 1),
    back: () => goTo(index - 1),
    restart: (kind) => {
      const i = tasks.findIndex((t) => t.kind === kind);
      if (i >= 0) goTo(i);
    },
  };

  return frame(
    <div className="flex flex-col gap-3.5">
      <div className="flex items-center gap-2.5">
        <PluginTile color={task.plugin.color} icon={task.plugin.railIcon} size={22} />
        <span className="text-body-lg font-semibold">{task.plugin.name}</span>
        <span className="flex text-tx3">
          <Icon name="chevronRight" size={14} />
        </span>
        <span className="text-body-lg text-tx2">{taskTitle(task)}</span>
        <div className="grow" />
        <span className="mono text-tiny text-tx3">
          étape {index + 1} sur {tasks.length}
        </span>
      </div>

      <TaskBody key={taskKey(task)} task={task} nav={nav} following={following} />
    </div>,
  );
}

interface TaskNav {
  next: () => void;
  back: () => void;
  /** Revient au premier écran de ce type (« Changer de compte » → connexion GitHub). */
  restart: (kind: SetupTask["kind"]) => void;
}

function TaskBody({
  task,
  nav,
  following,
}: {
  task: SetupTask;
  nav: TaskNav;
  following: SetupTask | null;
}) {
  switch (task.kind) {
    case "github":
      return <GithubTask plugin={task.plugin} nav={nav} following={following} />;
    case "repos":
      return <ReposTask plugin={task.plugin} nav={nav} />;
    case "secret":
      return <SecretTask task={task} nav={nav} following={following} />;
    case "view":
      return <ViewTask task={task} nav={nav} following={following} />;
  }
}

/* ---------- Écran suivant ---------- */

function NextCard({ task, onClick }: { task: SetupTask; onClick: () => void }) {
  const hint: Record<SetupTask["kind"], string> = {
    github: "Connectez le compte GitHub que les plugins utiliseront.",
    repos:
      "Choisissez les dépôts que le plugin suit. Vous passerez de l'un à l'autre, ou les verrez tous ensemble, depuis la barre du plugin.",
    secret: "Une valeur que le plugin garde au trousseau du système.",
    view: "Un écran de configuration fourni par le plugin.",
  };
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-3.5 rounded-card border border-dashed border-line2 px-4 py-3.5 text-left text-tx hover:border-tx3"
    >
      <PluginTile color={task.plugin.color} icon={task.plugin.railIcon} size={30} />
      <span className="min-w-0 grow">
        <span className="block text-tiny font-semibold tracking-[0.09em] text-tx3 uppercase">
          Étape suivante
        </span>
        <span className="mt-[3px] block text-body-lg font-medium">
          {task.plugin.name} · {taskTitle(task)}
        </span>
        <span className="mt-0.5 block text-small text-tx3">
          {task.kind === "secret" && task.secret.description
            ? task.secret.description
            : hint[task.kind]}
        </span>
      </span>
      <span className="flex text-tx3">
        <Icon name="chevronRight" size={16} />
      </span>
    </button>
  );
}

/* ---------- Service Dépôts GitHub : connexion ---------- */

function ServiceHeader({ plugin, status }: { plugin: InstalledPluginView; status: ReactNode }) {
  return (
    <div className="flex h-9 items-center gap-2 border-b border-line bg-p2 px-3.5 text-caption text-tx3">
      <span
        aria-hidden
        className="inline-flex size-4 shrink-0 items-center justify-center rounded-badge bg-clay text-clay-ink"
      >
        <Icon name="github" size={10} />
      </span>
      <span className="font-semibold text-tx">Service Dépôts GitHub</span>
      <Badge tone="clay">fourni par Cadence</Badge>
      <span>utilisé par</span>
      <span className="inline-flex items-center gap-1.5 text-tx2">
        <PluginTile color={plugin.color} icon={plugin.railIcon} size={14} />
        {plugin.name}
      </span>
      <div className="grow" />
      {status}
    </div>
  );
}

function GithubTask({
  plugin,
  nav,
  following,
}: {
  plugin: InstalledPluginView;
  nav: TaskNav;
  following: SetupTask | null;
}) {
  const auth = useAuthStatus();
  const signOut = useSignOut();
  const [mode, setMode] = useState<"browser" | "code">("browser");
  const [patOpen, setPatOpen] = useState(false);
  const github = auth.data?.github;
  const connected = github?.status === "connected";

  return (
    <>
      <div className="overflow-hidden rounded-card border border-line2 bg-panel">
        <ServiceHeader
          plugin={plugin}
          status={
            <span className="inline-flex items-center gap-1.5">
              <StatusDot tone={connected ? "green" : "neutral"} />
              {connected ? "Connecté" : "Non connecté"}
            </span>
          }
        />
        <div className="flex gap-[26px] px-6 py-5">
          <div className="w-[380px] shrink-0 max-[1320px]:w-[320px]">
            {auth.isPending ? (
              <div className="flex h-40 items-center justify-center">
                <Spinner />
              </div>
            ) : connected && github ? (
              <div>
                <div className="flex items-center gap-[9px] text-body-sm text-green">
                  <Icon name="check" size={16} />
                  Compte connecté
                </div>
                <div className="mt-3.5 flex items-center gap-2.5 rounded-field border border-line bg-p2 px-3.5 py-3">
                  <Avatar
                    size={24}
                    initials={initialsOf(github.name ?? github.login ?? "GH")}
                    src={github.avatarUrl}
                  />
                  <div className="min-w-0 grow">
                    <div className="truncate text-body font-medium">@{github.login}</div>
                    <div className="text-caption text-tx3">Jeton dans le trousseau du système</div>
                  </div>
                  <button
                    type="button"
                    disabled={signOut.isPending}
                    onClick={() => signOut.mutate()}
                    className="text-small whitespace-nowrap text-clay hover:text-clay-hover"
                  >
                    Changer de compte
                  </button>
                </div>
              </div>
            ) : mode === "browser" && auth.data ? (
              <BrowserSignInCard
                bare
                sources={auth.data.githubSources}
                onUseCode={() => setMode("code")}
                onUsePat={() => setPatOpen(true)}
              />
            ) : (
              <DeviceFlowCard bare onUsePat={() => setPatOpen(true)} />
            )}
            {!connected && !auth.isPending && (
              <div className="mt-3 text-small leading-[1.6] text-tx3">
                {mode === "code" ? (
                  <>
                    <LinkButton onClick={() => setMode("browser")}>
                      Se connecter plutôt via le navigateur
                    </LinkButton>{" "}
                    ou <LinkButton onClick={() => setPatOpen(true)}>jeton personnel</LinkButton>.
                  </>
                ) : (
                  <>
                    Pas de navigateur ?{" "}
                    {auth.data?.githubSources.deviceFlow && (
                      <>
                        <LinkButton onClick={() => setMode("code")}>Code d'appairage</LinkButton>{" "}
                        ou{" "}
                      </>
                    )}
                    <LinkButton onClick={() => setPatOpen(true)}>jeton personnel</LinkButton>.
                  </>
                )}
              </div>
            )}
          </div>
          <div className="min-w-0 grow">
            <SectionLabel>Permissions demandées</SectionLabel>
            <div className="mt-1">
              {permissions.map((p) => (
                <PermissionRow key={p.scope} {...p} granted={github?.scopes.includes(p.scope)} />
              ))}
            </div>
            <div className="mt-3 flex items-start gap-2 text-caption leading-[1.55] text-tx3">
              <span className="mt-px flex">
                <Icon name="key" size={14} />
              </span>
              <span>
                Jeton rangé par Cadence dans le trousseau du système. Le serveur d'un plugin qui
                déclare le service le reçoit dans{" "}
                <span className="mono text-tx2">CADENCE_GITHUB_TOKEN</span> ; son interface ne le
                voit jamais.
              </span>
            </div>
          </div>
        </div>
      </div>

      <Callout tone="info" icon="info">
        Cette connexion appartient à Cadence, pas au plugin. Elle apparaît ici parce que{" "}
        {plugin.name} déclare <span className="mono text-tx">services = ["repos"]</span>, et servira
        à tout autre plugin qui l'utilise, sans nouvelle connexion.
      </Callout>

      {following && <NextCard task={following} onClick={nav.next} />}

      <OnboardingActions onBack={nav.back}>
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          disabled={!connected}
          onClick={nav.next}
        >
          Continuer
        </Button>
        <Button variant="ghost" onClick={nav.next}>
          Configurer plus tard
        </Button>
      </OnboardingActions>

      <PatDialog open={patOpen} onOpenChange={setPatOpen} />
    </>
  );
}

/* ---------- Service Dépôts GitHub : dépôts suivis ---------- */

const serviceFacts: { icon: IconName; title: string; text: ReactNode }[] = [
  {
    icon: "eye",
    title: "Lecture par l'API GitHub",
    text: "Issues, PR et revues s'affichent sans rien cloner.",
  },
  {
    icon: "download",
    title: "Clone à la demande",
    text: (
      <>
        Seulement quand un skill doit toucher au code, sous{" "}
        <span className="mono text-tx2">repos/&lt;owner&gt;/&lt;nom&gt;/</span>, ou dans un clone
        que vous désignez.
      </>
    ),
  },
  {
    icon: "branch",
    title: "Un worktree par tâche",
    text: "L'IA n'y a accès que pour la session de cette tâche.",
  },
];

function ReposTask({ plugin, nav }: { plugin: InstalledPluginView; nav: TaskNav }) {
  const auth = useAuthStatus();
  const settings = useAppSettings();
  const save = useSaveAppSettings();
  const [query, setQuery] = useState("");
  const [owner, setOwner] = useState("");
  const repos = useRepos("");
  const github = auth.data?.github;
  const connected = github?.status === "connected";
  const owners = useRepoOwners(connected);
  const [picked, setPicked] = useState<string[] | null>(null);
  const [adding, setAdding] = useState(false);
  const [url, setUrl] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);

  const selected = picked ?? settings.data?.trackedRepos[plugin.id] ?? [];
  const isSelected = (name: string) => selected.some((r) => r.toLowerCase() === name.toLowerCase());
  const toggle = (name: string) =>
    setPicked(
      isSelected(name)
        ? selected.filter((r) => r.toLowerCase() !== name.toLowerCase())
        : [...selected, name],
    );

  const all = repos.data ?? [];
  const extra = selected.filter(
    (name) => !all.some((r) => r.fullName.toLowerCase() === name.toLowerCase()),
  );
  const q = query.trim().toLowerCase();
  const visible = all.filter(
    (r) =>
      (!owner || r.owner === owner) &&
      (!q || r.fullName.toLowerCase().includes(q) || r.description?.toLowerCase().includes(q)),
  );

  const addUrl = () => {
    const name = parseRepoInput(url);
    if (!name) {
      setUrlError("Adresse GitHub ou owner/nom attendu.");
      return;
    }
    if (!isSelected(name)) setPicked([...selected, name]);
    setUrl("");
    setUrlError(null);
    setAdding(false);
  };

  const follow = () =>
    save.mutate({ trackedRepos: { [plugin.id]: selected } }, { onSuccess: nav.next });

  if (!connected) {
    return (
      <>
        <Callout tone="warning">
          Le service Dépôts GitHub n'est pas connecté : connectez un compte pour choisir les dépôts
          suivis, ou configurez-les plus tard depuis la barre du plugin.
        </Callout>
        <OnboardingActions onBack={nav.back}>
          <Button variant="primary" size="lg" icon="github" onClick={() => nav.restart("github")}>
            Connecter GitHub
          </Button>
          <Button variant="ghost" onClick={nav.next}>
            Configurer plus tard
          </Button>
        </OnboardingActions>
      </>
    );
  }

  return (
    <>
      <div className="flex items-center gap-[11px] rounded-tile border border-green-line bg-green-d px-3.5 py-2.5">
        <span className="inline-flex size-[22px] items-center justify-center rounded-full bg-green text-ink">
          <Icon name="check" size={13} />
        </span>
        <span className="min-w-0 grow text-body-sm text-tx">
          Service Dépôts GitHub connecté : <span className="font-semibold">@{github.login}</span>{" "}
          <span className="text-tx3">· jeton dans le trousseau</span>
        </span>
        <button
          type="button"
          className="text-small text-clay hover:text-clay-hover"
          onClick={() => nav.restart("github")}
        >
          Changer de compte
        </button>
      </div>

      <div className="flex gap-5">
        <div className="min-w-0 grow overflow-hidden rounded-card border border-line bg-panel">
          <div className="flex h-12 items-center gap-2 border-b border-line px-3">
            <div className="flex h-[30px] w-[230px] min-w-0 items-center gap-2 rounded-control border border-line bg-p2 px-2.5 text-tx3 max-[1320px]:w-[170px]">
              <Icon name="search" size={14} />
              <input
                aria-label="Filtrer les dépôts"
                placeholder={`Filtrer vos ${all.length} dépôts…`}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="min-w-0 grow bg-transparent text-body-sm text-tx outline-none placeholder:text-tx3"
              />
            </div>
            <select
              aria-label="Propriétaire"
              value={owner}
              onChange={(e) => setOwner(e.target.value)}
              className="h-[30px] w-[180px] rounded-control border border-line2 bg-p2 px-2.5 text-body-sm text-tx max-[1320px]:w-[150px]"
            >
              <option value="">Tous les propriétaires</option>
              {(owners.data ?? []).map((o) => (
                <option key={o.login} value={o.login}>
                  {o.login}
                </option>
              ))}
            </select>
            <div className="grow" />
            <span className="text-small whitespace-nowrap text-tx3">
              <span className="font-semibold text-clay">{selected.length}</span> suivi
              {selected.length > 1 ? "s" : ""}
            </span>
          </div>
          <div className="max-h-[416px] overflow-y-auto">
            {repos.isPending && (
              <div className="flex h-24 items-center justify-center">
                <Spinner />
              </div>
            )}
            {extra.map((name) => (
              <RepoLine key={name} name={name} checked onToggle={() => toggle(name)} />
            ))}
            {visible.map((repo) => (
              <RepoLine
                key={repo.fullName}
                repo={repo}
                name={repo.fullName}
                checked={isSelected(repo.fullName)}
                onToggle={() => toggle(repo.fullName)}
              />
            ))}
            {!repos.isPending && visible.length === 0 && extra.length === 0 && (
              <div className="px-4 py-6 text-center text-body-sm text-tx3">
                {repos.isError ? repos.error.message : "Aucun dépôt ne correspond."}
              </div>
            )}
          </div>
        </div>

        <div className="flex w-[236px] shrink-0 flex-col gap-3 max-[1320px]:w-[200px]">
          <SectionLabel>Ce que fait le service</SectionLabel>
          <div className="-mt-2">
            {serviceFacts.map((f) => (
              <div key={f.title} className="flex gap-2.5 border-b border-line py-2.5">
                <span className="mt-px flex text-clay">
                  <Icon name={f.icon} size={15} />
                </span>
                <div>
                  <div className="text-body-sm">{f.title}</div>
                  <div className="mt-[3px] text-caption leading-[1.55] text-tx3">{f.text}</div>
                </div>
              </div>
            ))}
          </div>
          <SectionLabel className="mt-1">Dans la barre du plugin</SectionLabel>
          <div className="rounded-tile border border-line bg-panel p-2.5">
            <div className="flex h-9 items-center gap-2 rounded-lg border border-line2 bg-p2 px-2">
              <span className="inline-flex size-5 shrink-0 items-center justify-center rounded-md bg-p4 text-tx2">
                <Icon name="layers" size={12} />
              </span>
              <span className="min-w-0 grow truncate text-body-sm font-medium">
                {selected.length === 1 ? selected[0] : "Tous les dépôts"}
              </span>
              <span className="mono text-tiny text-tx3">{selected.length}</span>
              <span className="flex text-tx3">
                <Icon name="chevronDown" size={13} />
              </span>
            </div>
          </div>
          <div className="text-caption leading-[1.55] text-tx3">
            Un dépôt à la fois, ou « Tous les dépôts » pour les vues agrégées.
          </div>
        </div>
      </div>

      {adding && (
        <div className="flex max-w-[640px] items-center gap-2">
          <input
            autoFocus
            aria-label="Adresse du dépôt"
            placeholder="https://github.com/owner/nom ou owner/nom"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addUrl()}
            className="h-8 min-w-0 grow rounded-control border border-line2 bg-p2 px-2.5 mono text-body-sm text-tx outline-none focus:border-clay-line"
          />
          <Button onClick={addUrl}>Ajouter</Button>
          <Button variant="quiet" onClick={() => setAdding(false)}>
            Annuler
          </Button>
          {urlError && <span className="text-caption text-red">{urlError}</span>}
        </div>
      )}

      <OnboardingActions onBack={nav.back}>
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          disabled={save.isPending}
          onClick={follow}
        >
          {selected.length === 0
            ? "Continuer sans dépôt"
            : `Suivre ${selected.length} dépôt${selected.length > 1 ? "s" : ""} et continuer`}
        </Button>
        <Button variant="ghost" icon="link" onClick={() => setAdding(true)}>
          Ajouter par URL…
        </Button>
        {save.error && <span className="text-caption text-red">{save.error.message}</span>}
      </OnboardingActions>
    </>
  );
}

function RepoLine({
  repo,
  name,
  checked,
  onToggle,
}: {
  repo?: Repo;
  name: string;
  checked: boolean;
  onToggle: () => void;
}) {
  const [owner, short] = name.split("/") as [string, string];
  const initials = `${owner[0] ?? ""}${short[0] ?? ""}`.toUpperCase();
  return (
    <label
      className={cn(
        "flex h-[52px] cursor-pointer items-center gap-3 border-b border-line px-3.5 last:border-b-0",
        checked && "bg-p2",
      )}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        className="m-0 size-[15px] shrink-0 accent-clay"
      />
      <span
        aria-hidden
        className={cn(
          "inline-flex size-[26px] shrink-0 items-center justify-center rounded-control mono text-[10px] font-semibold",
          checked ? "bg-clay-d text-clay" : "bg-p3 text-tx3",
        )}
      >
        {initials}
      </span>
      <span className="min-w-0 grow">
        <span className="block truncate text-body-sm font-medium">
          <span className="font-normal text-tx3">{owner}/</span>
          {short}
        </span>
        <span className="mt-0.5 block truncate text-caption text-tx3">
          {repo
            ? [repo.description, repo.pushedAt && formatAgo(repo.pushedAt)]
                .filter(Boolean)
                .join(" · ") || "Sans description"
            : "Ajouté par adresse"}
        </span>
      </span>
      {repo && <Badge>{repo.private ? "Privé" : "Public"}</Badge>}
      <span className="flex w-11 shrink-0 items-center gap-[5px] text-caption text-tx2">
        {repo && (
          <>
            <Icon name="issue" size={13} />
            {repo.openIssues}
          </>
        )}
      </span>
      <span className="flex w-[150px] shrink-0 items-center gap-1.5 overflow-hidden text-caption whitespace-nowrap text-tx3 max-[1320px]:hidden">
        {checked && (
          <>
            <Icon name="download" size={13} />
            clone à la demande
          </>
        )}
      </span>
    </label>
  );
}

/* ---------- Secret d'un plugin ---------- */

function SecretTask({
  task,
  nav,
  following,
}: {
  task: Extract<SetupTask, { kind: "secret" }>;
  nav: TaskNav;
  following: SetupTask | null;
}) {
  const { plugin, secret } = task;
  const has = usePluginSecret(plugin.id, secret.id);
  const set = useSetPluginSecret(plugin.id, secret.id);
  const [value, setValue] = useState("");
  const stored = has.data === true;

  const save = () => {
    if (!value.trim()) {
      nav.next();
      return;
    }
    set.mutate(value.trim(), { onSuccess: nav.next });
  };

  return (
    <>
      <div className="rounded-card border border-line2 bg-panel px-6 py-5">
        <div className="flex items-center gap-2">
          <span className="flex text-clay">
            <Icon name="key" size={16} />
          </span>
          <h3 className="grow text-title-sm">{secret.label}</h3>
          {stored ? (
            <Badge tone="green" icon="check">
              au trousseau
            </Badge>
          ) : (
            <Badge tone={secret.required ? "clay" : "neutral"}>
              {secret.required ? "requis" : "optionnel"}
            </Badge>
          )}
        </div>
        {secret.description && (
          <p className="mt-2 text-body-sm leading-[1.6] text-tx3">{secret.description}</p>
        )}
        <input
          type="password"
          autoComplete="off"
          aria-label={secret.label}
          placeholder={
            stored ? "•••••••• (déjà enregistré, saisir pour remplacer)" : "Collez la valeur ici"
          }
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && save()}
          className="mt-3.5 h-9 w-full max-w-[520px] rounded-field border border-line2 bg-p2 px-3 mono text-body-sm text-tx outline-none focus:border-clay-line"
        />
        <div className="mt-3 flex items-start gap-2 text-caption leading-[1.55] text-tx3">
          <span className="mt-px flex">
            <Icon name="shield" size={14} />
          </span>
          <span>
            Rangé dans le trousseau du système. Le serveur de {plugin.name} le reçoit dans{" "}
            <span className="mono text-tx2">CADENCE_SECRET_{secret.id.toUpperCase()}</span> ; son
            interface ne le voit jamais.
          </span>
        </div>
        {set.error && <p className="mt-2 text-caption text-red">{set.error.message}</p>}
      </div>

      {following && <NextCard task={following} onClick={nav.next} />}

      <OnboardingActions onBack={nav.back}>
        <Button
          variant="primary"
          size="lg"
          iconRight="arrowRight"
          disabled={set.isPending || (secret.required && !stored && !value.trim())}
          onClick={save}
        >
          {set.isPending && <Spinner className="text-clay-ink" />}
          {value.trim() ? "Enregistrer et continuer" : "Continuer"}
        </Button>
        <Button variant="ghost" onClick={nav.next}>
          Configurer plus tard
        </Button>
      </OnboardingActions>
    </>
  );
}

/* ---------- Écran [[setup]] fourni par le plugin ---------- */

function ViewTask({
  task,
  nav,
  following,
}: {
  task: Extract<SetupTask, { kind: "view" }>;
  nav: TaskNav;
  following: SetupTask | null;
}) {
  const { plugin, step } = task;
  return (
    <>
      {plugin.uiEntry ? (
        <div className="flex h-[440px] flex-col overflow-hidden rounded-card border border-line2 bg-panel">
          <PluginFrameSlot plugin={plugin.id} view={step.view} params={{ setup: step.id }} />
        </div>
      ) : (
        <Callout tone="warning">
          {plugin.name} déclare l'écran « {step.title} » sans interface : il se configurera depuis
          ses réglages.
        </Callout>
      )}

      {following && <NextCard task={following} onClick={nav.next} />}

      <OnboardingActions onBack={nav.back}>
        <Button variant="primary" size="lg" iconRight="arrowRight" onClick={nav.next}>
          Continuer
        </Button>
        {!step.required && (
          <Button variant="ghost" onClick={nav.next}>
            Configurer plus tard
          </Button>
        )}
      </OnboardingActions>
    </>
  );
}
