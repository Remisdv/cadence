/**
 * Réglages › Services › Dépôts GitHub (V2-Settings.dc.html) : compte connecté au service,
 * emplacement des clones (sous la configuration de l'app), dépôts suivis par chaque plugin qui
 * déclare `services = ["repos"]`.
 */
import { useState } from "react";
import type { InstalledPluginView } from "@/lib/types";
import { Icon } from "@/components/icons/Icon";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Input } from "@/components/ui/Field";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { SettingRow } from "@/components/domain/Panels";
import { PluginTile } from "@/components/domain/PluginTile";
import { parseRepoInput } from "@/lib/domain/onboarding";
import {
  useAppSettings,
  useAppStatus,
  useAuthStatus,
  useSaveAppSettings,
  useSignOut,
} from "@/lib/query/queries";
import { BrowserSignInCard } from "../onboarding/GithubView";
import { PatDialog } from "../onboarding/PatDialog";
import { SectionTitle } from "./SectionTitle";

export function ReposSettings({ plugins }: { plugins: InstalledPluginView[] }) {
  const auth = useAuthStatus().data;
  const dirs = useAppStatus().data?.dirs;
  const settings = useAppSettings().data;
  const signOut = useSignOut();
  const [patOpen, setPatOpen] = useState(false);
  const github = auth?.github;
  const connected = github?.status === "connected";
  const users = plugins.filter((p) => p.enabled && p.services.includes("repos"));
  const tracked = settings?.trackedRepos ?? {};
  const all = [...new Set(Object.values(tracked).flat())];

  return (
    <div className="flex gap-6">
      <div className="max-w-[760px] min-w-0 grow">
        <SectionTitle title="Services">
          Briques fournies par Cadence et activées par les plugins qui les déclarent dans leur
          manifeste. Un service n'apparaît que si un plugin activé l'utilise.
        </SectionTitle>

        <div className="mt-[18px] overflow-hidden rounded-card border border-line bg-panel">
          <div className="flex items-center gap-3 border-b border-line bg-p2 px-4 py-3">
            <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-control bg-clay text-clay-ink">
              <Icon name="github" size={15} />
            </span>
            <div className="min-w-0 grow">
              <div className="text-body font-semibold">Dépôts GitHub</div>
              <div className="truncate text-caption text-tx3">
                <span className="mono">repos</span> · connexion GitHub, dépôts suivis, clones et
                worktrees
              </div>
            </div>
            <Badge tone={connected ? "green" : "amber"}>
              {connected ? "Connecté" : "Non connecté"}
            </Badge>
          </div>
          <div className="px-4">
            {connected && github ? (
              <SettingRow
                title="Compte connecté"
                control={
                  <>
                    <Avatar
                      size={24}
                      initials={initialsOf(github.name ?? github.login ?? "GH")}
                      src={github.avatarUrl}
                    />
                    <div className="mr-2">
                      <div className="text-body-sm font-medium">@{github.login}</div>
                      <div className="text-caption text-tx3">jeton dans le trousseau</div>
                    </div>
                    <Button
                      variant="danger"
                      size="sm"
                      disabled={signOut.isPending}
                      onClick={() => signOut.mutate()}
                    >
                      Déconnecter
                    </Button>
                  </>
                }
              />
            ) : (
              <div className="border-b border-line py-4">
                {auth && (
                  <BrowserSignInCard
                    bare
                    sources={auth.githubSources}
                    onUseCode={() => setPatOpen(true)}
                    onUsePat={() => setPatOpen(true)}
                  />
                )}
              </div>
            )}
            <SettingRow
              title="Emplacement des clones"
              description="Clone à la demande, seulement quand un skill doit toucher au code ; un worktree par tâche à côté."
              control={
                <Input
                  size="sm"
                  mono
                  readOnly
                  aria-label="Emplacement des clones"
                  value={dirs?.repos ?? ""}
                  className="w-[300px] text-caption text-tx2"
                />
              }
            />
            <SettingRow
              title="Un worktree par tâche"
              description="Supprimé quand la PR de la tâche est fusionnée ou fermée."
              control={<Badge tone="muted">toujours</Badge>}
            />
            <div className="flex items-center gap-3.5 py-[13px]">
              <div className="min-w-0 grow">
                <div className="text-body-sm">Accès de l'IA</div>
                <div className="mt-[3px] text-caption leading-normal text-tx3">
                  Le worktree de la tâche seulement, pour la durée de sa session : --add-dir (Claude
                  Code), racine inscriptible (Codex CLI), permission de chemin (OpenCode).
                </div>
              </div>
              <Badge tone="muted">par session</Badge>
            </div>
          </div>
        </div>

        <SectionLabel className="mt-[22px]">Dépôts suivis</SectionLabel>
        {users.length === 0 ? (
          <Callout tone="info" icon="info" className="mt-2">
            Aucun plugin activé n'utilise le service : il s'activera avec le premier plugin qui
            déclare <span className="mono text-tx">services = ["repos"]</span>.
          </Callout>
        ) : (
          users.map((plugin) => (
            <TrackedRepos key={plugin.id} plugin={plugin} repos={tracked[plugin.id] ?? []} />
          ))
        )}
        {all.length > 0 && (
          <p className="mt-2 text-caption leading-[1.6] text-tx3">
            Aucun clone pour l'instant : le premier skill qui touche au code clonera le dépôt sous
            l'emplacement ci-dessus.
          </p>
        )}
      </div>

      <div className="flex w-[260px] shrink-0 flex-col gap-3 pt-1 max-[1320px]:hidden">
        <SectionLabel>Utilisé par</SectionLabel>
        <div className="rounded-tile border border-line bg-panel p-3">
          {users.length === 0 ? (
            <div className="text-caption text-tx3">Aucun plugin activé.</div>
          ) : (
            users.map((p) => (
              <div key={p.id} className="flex items-center gap-2 py-1 text-body-sm">
                <PluginTile color={p.color} icon={p.railIcon} size={18} />
                <span className="min-w-0 grow truncate">{p.name}</span>
                <span className="text-caption text-tx3">
                  {(tracked[p.id] ?? []).length} suivi{(tracked[p.id] ?? []).length > 1 ? "s" : ""}
                </span>
              </div>
            ))
          )}
        </div>
        <SectionLabel className="mt-1">Manifeste du plugin</SectionLabel>
        <pre className="rounded-[11px] border border-line bg-panel px-3.5 py-3 mono text-caption leading-[1.7] whitespace-pre-wrap text-tx">
          <span className="text-tx3"># plugin.toml</span>
          {"\nservices = ["}
          <span className="text-green">"repos"</span>
          {"]"}
        </pre>
        <div className="text-caption leading-[1.6] text-tx3">
          Le serveur du plugin reçoit <span className="mono text-tx2">CADENCE_GITHUB_TOKEN</span> et{" "}
          <span className="mono text-tx2">CADENCE_REPOS</span>, puis une notification à chaque
          changement de la liste.
        </div>
      </div>

      <PatDialog open={patOpen} onOpenChange={setPatOpen} />
    </div>
  );
}

function TrackedRepos({ plugin, repos }: { plugin: InstalledPluginView; repos: string[] }) {
  const save = useSaveAppSettings();
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const update = (next: string[]) => save.mutate({ trackedRepos: { [plugin.id]: next } });
  const add = () => {
    const name = parseRepoInput(input);
    if (!name) {
      setError("Adresse GitHub ou owner/nom attendu.");
      return;
    }
    setError(null);
    setInput("");
    if (!repos.some((r) => r.toLowerCase() === name.toLowerCase())) update([...repos, name]);
  };

  return (
    <div className="mt-2 overflow-hidden rounded-card border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-3.5 py-2.5">
        <PluginTile color={plugin.color} icon={plugin.railIcon} size={18} />
        <span className="grow text-body-sm font-medium">{plugin.name}</span>
        <span className="text-caption text-tx3">
          {repos.length} dépôt{repos.length > 1 ? "s" : ""} suivi{repos.length > 1 ? "s" : ""}
        </span>
      </div>
      {repos.map((repo) => {
        const [owner, name] = repo.split("/");
        return (
          <div key={repo} className="flex h-11 items-center gap-3 border-b border-line px-3.5">
            <span className="inline-flex size-[26px] shrink-0 items-center justify-center rounded-control bg-clay-d mono text-[10px] font-semibold text-clay">
              {`${owner?.[0] ?? ""}${name?.[0] ?? ""}`.toUpperCase()}
            </span>
            <span className="min-w-0 grow truncate text-body-sm">
              <span className="text-tx3">{owner}/</span>
              {name}
            </span>
            <span className="text-caption text-tx3">pas encore cloné</span>
            <Button
              variant="quiet"
              size="xs"
              iconOnly
              aria-label={`Ne plus suivre ${repo}`}
              disabled={save.isPending}
              onClick={() => update(repos.filter((r) => r !== repo))}
            >
              <Icon name="close" size={13} />
            </Button>
          </div>
        );
      })}
      <form
        className="flex items-center gap-2 px-3.5 py-2.5"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <Input
          size="sm"
          mono
          aria-label={`Suivre un dépôt avec ${plugin.name}`}
          placeholder="https://github.com/owner/nom ou owner/nom"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          className="min-w-0 grow"
        />
        <Button type="submit" size="sm" icon="plus" disabled={!input.trim() || save.isPending}>
          Suivre
        </Button>
      </form>
      {(error || save.error) && (
        <div className="px-3.5 pb-2.5 text-caption text-red">{error ?? save.error?.message}</div>
      )}
    </div>
  );
}
