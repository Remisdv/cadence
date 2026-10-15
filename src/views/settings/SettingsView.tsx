/**
 * Réglages (maquette Settings.dc.html) : navigation à gauche, section à droite.
 *
 * « Projet et mapping » et « Agents » éditent `.cadence/config.toml` du dépôt (écrit par un
 * commit sur la branche par défaut) ; « Comptes et jetons » et « Machine locale » montrent
 * ce qui reste sur cette machine (trousseau, `settings.json`).
 */
import { useState, type ReactNode } from "react";
import type { AgentConfig, AgentEngine, Autonomy, CliAgentInfo, Config } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import type { SettingsSection } from "@/stores/navigation";
import { Page } from "@/components/layout/Page";
import { ErrorState, LoadingState } from "@/components/layout/EmptyState";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Note, SettingRow } from "@/components/domain/Panels";
import { VersionBadge } from "@/components/domain/Badges";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { Input, Select } from "@/components/ui/Field";
import { Kbd } from "@/components/ui/Kbd";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { Switch } from "@/components/ui/Switch";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { agentOrder, agents, autonomyLabels } from "@/lib/domain/agents";
import { modKey } from "@/lib/hooks/usePlatform";
import {
  useAuthStatus,
  useCurrentProject,
  useSaveSettings,
  useSettings,
  useSignOut,
} from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";

const sections: { id: SettingsSection; label: string; icon: IconName }[] = [
  { id: "accounts", label: "Comptes et jetons", icon: "key" },
  { id: "project", label: "Projet et mapping", icon: "tag" },
  { id: "agents", label: "Agents", icon: "bot" },
  { id: "machine", label: "Machine locale", icon: "layers" },
  { id: "appearance", label: "Apparence", icon: "eye" },
  { id: "shortcuts", label: "Raccourcis", icon: "sparkle" },
];

export default function SettingsView({ view }: ViewProps<"settings">) {
  const navigate = useNavigation((s) => s.navigate);
  const project = useCurrentProject().data;
  const section = view.section ?? "project";

  return (
    <Page
      title="Réglages"
      subtitle={
        project
          ? `${project.repo.fullName} · les réglages de projet sont stockés dans .cadence/config.toml`
          : "les réglages de projet sont stockés dans .cadence/config.toml"
      }
      padding="none"
    >
      <nav className="flex w-[214px] shrink-0 flex-col gap-[3px] border-r border-line bg-panel px-[11px] py-3.5">
        <SectionLabel className="px-2.5 pb-[7px]">Réglages</SectionLabel>
        {sections.map((s) => (
          <button
            key={s.id}
            type="button"
            aria-current={s.id === section ? "page" : undefined}
            onClick={() => navigate({ id: "settings", section: s.id })}
            className={cn(
              "flex h-8 w-full items-center gap-[9px] rounded-control px-2.5 text-left text-body-sm",
              s.id === section ? "bg-p3 text-tx" : "text-tx2 hover:bg-p2 hover:text-tx",
            )}
          >
            <span className={cn("flex", s.id === section ? "text-clay" : "text-tx3")}>
              <Icon name={s.icon} size={15} />
            </span>
            {s.label}
          </button>
        ))}
        <div className="mt-auto border-t border-line px-2.5 pt-[11px] text-tiny leading-[1.6] text-tx3">
          Cadence
          <br />
          Tauri 2 · React 19
        </div>
      </nav>

      <div className="min-w-0 grow overflow-y-auto px-[26px] py-5">
        <div className="max-w-[780px]">
          {section === "accounts" && <AccountsSection />}
          {(section === "project" || section === "agents") && <ConfigSection section={section} />}
          {section === "machine" && <MachineSection />}
          {section === "appearance" && <AppearanceSection />}
          {section === "shortcuts" && <ShortcutsSection />}
        </div>
      </div>
    </Page>
  );
}

function SectionTitle({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <>
      <h2 className="font-display text-title font-semibold">{title}</h2>
      {children && <p className="mt-[7px] text-body-sm leading-[1.6] text-tx2">{children}</p>}
    </>
  );
}

/* ---------- Projet et agents : .cadence/config.toml ---------- */

function ConfigSection({ section }: { section: "project" | "agents" }) {
  const settings = useSettings();
  // Ici et non dans le formulaire : son état (succès, erreur) survit au remontage ci-dessous.
  const save = useSaveSettings();
  if (settings.isPending) return <LoadingState label="Lecture de .cadence/config.toml…" />;
  if (settings.isError) return <ErrorState error={settings.error} />;
  // `key` : repart des valeurs enregistrées quand elles changent (autre dépôt, enregistrement).
  return <ConfigForm key={JSON.stringify(settings.data)} saved={settings.data} section={section} save={save} />;
}

function ConfigForm({
  saved,
  section,
  save,
}: {
  saved: Config;
  section: "project" | "agents";
  save: ReturnType<typeof useSaveSettings>;
}) {
  const project = useCurrentProject().data;
  const [draft, setDraft] = useState<Config>(saved);
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const patch = <K extends keyof Config>(key: K, value: Partial<Config[K]>) =>
    setDraft((d) => ({ ...d, [key]: { ...d[key], ...value } }));

  return (
    <>
      {section === "project" ? (
        <ProjectFields draft={draft} patch={patch} />
      ) : (
        <AgentFields
          draft={draft}
          onAgent={(kind, value) =>
            setDraft((d) => ({ ...d, agents: { ...d.agents, [kind]: { ...d.agents[kind], ...value } } }))
          }
        />
      )}

      {project && !project.hasConfig && (
        <Note className="mt-5">
          Ce dépôt n'a pas encore de <span className="mono">.cadence/config.toml</span> : les valeurs
          affichées sont les valeurs par défaut. Enregistrer crée le fichier.
        </Note>
      )}
      {save.isError && (
        <Callout tone="danger" className="mt-4">
          {save.error.message}
        </Callout>
      )}
      <div className="mt-[22px] flex items-center gap-2.5">
        <Button variant="ghost" disabled={!dirty || save.isPending} onClick={() => setDraft(saved)}>
          Réinitialiser
        </Button>
        <Button
          variant="primary"
          icon={save.isPending ? undefined : "check"}
          disabled={!dirty || save.isPending}
          onClick={() => save.mutate(draft)}
        >
          {save.isPending && <Spinner />}
          Enregistrer
        </Button>
        <span className="text-caption text-tx3">
          {save.isSuccess && !dirty ? (
            "Enregistré : un commit a été poussé sur la branche par défaut."
          ) : (
            <>
              Écrit dans <span className="mono">.cadence/config.toml</span> et versionné avec le dépôt.
            </>
          )}
        </span>
      </div>
    </>
  );
}

function MappingRow({ title, description, control }: { title: string; description: string; control: ReactNode }) {
  return <SettingRow title={title} description={description} control={control} />;
}

function ProjectFields({
  draft,
  patch,
}: {
  draft: Config;
  patch: <K extends keyof Config>(key: K, value: Partial<Config[K]>) => void;
}) {
  return (
    <>
      <SectionTitle title="Projet et mapping GitHub">
        Cadence ne stocke aucun état métier de son côté : chaque notion de l'application correspond à
        un objet GitHub existant.
      </SectionTitle>

      <SectionLabel className="mt-[18px]">Correspondance</SectionLabel>
      <div className="mt-1">
        <MappingRow
          title="Version / sprint"
          description="Un milestone par version. La date de fin du milestone est la fin de sprint."
          control={
            <Select size="sm" className="w-[200px]" value="milestones" disabled title="Projects v2 n'est pas encore pris en charge">
              <option value="milestones">Milestones</option>
            </Select>
          }
        />
        <MappingRow
          title="Version livrée"
          description="Un tag Git est posé à la clôture, avec un brouillon de release si demandé."
          control={
            <>
              <span className="mono text-small text-tx3">préfixe</span>
              <Select
                size="sm"
                className="w-[90px]"
                aria-label="Préfixe des tags"
                value={draft.project.tagPrefix}
                onChange={(e) => patch("project", { tagPrefix: e.target.value })}
              >
                <option value="v">v</option>
                <option value="">aucun</option>
              </Select>
            </>
          }
        />
        <MappingRow
          title="Type d'issue"
          description="Label appliqué à la création."
          control={<span className="mono text-small text-clay">type/*</span>}
        />
        <MappingRow
          title="Trace d'agent"
          description="Label et commentaire signés posés par les agents."
          control={<span className="mono text-small text-clay">agent/*</span>}
        />
      </div>

      <SectionLabel className="mt-[22px]">Numérotation des versions</SectionLabel>
      <Card className="mt-[9px] rounded-[11px] px-4 py-[15px]">
        <div className="flex flex-wrap items-center gap-[11px] text-body-sm text-tx2">
          Clôture de sprint :
          <VersionBadge version="0.1.2" tone="muted" />
          <span className="text-tx3">→</span>
          <VersionBadge version="0.1.3" />
          <span className="text-line2">·</span>
          Palier manuel :
          <VersionBadge version="0.2.0" tone="blue" />
          <span className="text-tx3">ou</span>
          <VersionBadge version="1.0.0" tone="purple" />
        </div>
        <div className="mt-[13px] flex flex-col gap-[11px]">
          <ToggleRow
            checked
            disabled
            label="Incrémenter le dernier segment à chaque sprint"
            hint="Compatible avec le tri sémantique des tags Git"
          />
          <ToggleRow
            checked={draft.versioning.confirmMajorBump}
            onChange={(v) => patch("versioning", { confirmMajorBump: v })}
            label="Demander confirmation avant un palier"
          />
          <div className="flex items-center gap-3">
            <Select
              size="sm"
              className="w-[90px]"
              aria-label="Durée d'un sprint"
              value={String(draft.versioning.sprintDays)}
              onChange={(e) => patch("versioning", { sprintDays: Number(e.target.value) })}
            >
              {[...new Set([7, 14, 21, 28, draft.versioning.sprintDays])]
                .sort((a, b) => a - b)
                .map((d) => (
                  <option key={d} value={d}>
                    {d} jours
                  </option>
                ))}
            </Select>
            <div className="min-w-0">
              <div className="text-body text-tx">Durée d'un sprint</div>
              <div className="mt-0.5 text-caption text-tx3">
                Échéance du milestone créé à la clôture du sprint précédent
              </div>
            </div>
          </div>
        </div>
      </Card>

      <SectionLabel className="mt-[22px]">Règles de création d'issue</SectionLabel>
      <div className="mt-1">
        <MappingRow
          title="Titre obligatoire"
          description="Une issue ne peut pas être créée sans titre."
          control={<Badge tone="neutral">verrouillé</Badge>}
        />
        <MappingRow
          title="Version obligatoire"
          description="Une issue doit toujours être rattachée à un milestone."
          control={<Badge tone="neutral">verrouillé</Badge>}
        />
        <MappingRow
          title="Passage par l'agent CTO"
          description="Envoyer chaque nouvelle issue à l'agent CTO à la création (actif avec l'agent, en 0.1.3)."
          control={
            <Switch
              aria-label="Passage par l'agent CTO"
              checked={draft.issues.sendToCtoOnCreate}
              onCheckedChange={(v) => patch("issues", { sendToCtoOnCreate: v })}
            />
          }
        />
        <MappingRow
          title="Modèle d'issue"
          description="Squelette utilisé par l'agent CTO pour structurer la description."
          control={
            <Select
              size="sm"
              className="w-[230px]"
              aria-label="Modèle d'issue"
              value={draft.issues.template}
              onChange={(e) => patch("issues", { template: e.target.value })}
            >
              <option value="contexte-attendu-criteres">Contexte / Attendu / Critères</option>
              <option value="libre">Libre</option>
              {!["contexte-attendu-criteres", "libre"].includes(draft.issues.template) && (
                <option value={draft.issues.template}>{draft.issues.template}</option>
              )}
            </Select>
          }
        />
      </div>
    </>
  );
}

function ToggleRow({
  checked,
  onChange,
  disabled,
  label,
  hint,
}: {
  checked: boolean;
  onChange?: (v: boolean) => void;
  disabled?: boolean;
  label: string;
  hint?: string;
}) {
  return (
    <div className="flex items-center gap-3">
      <Switch checked={checked} onCheckedChange={onChange ?? (() => {})} disabled={disabled} aria-label={label} />
      <div className="min-w-0">
        <div className="text-body text-tx">{label}</div>
        {hint && <div className="mt-0.5 text-caption text-tx3">{hint}</div>}
      </div>
    </div>
  );
}

type AgentKey = "cto" | "ceo" | "dev" | "deploy";

function AgentFields({
  draft,
  onAgent,
}: {
  draft: Config;
  onAgent: (kind: AgentKey, value: Partial<AgentConfig>) => void;
}) {
  return (
    <>
      <SectionTitle title="Agents">
        Moteur, modèle, autonomie et durée maximale d'une session, pour chaque agent de ce dépôt. Les
        agents arrivent à partir de la version 0.1.3 ; ces réglages sont déjà versionnés.
      </SectionTitle>
      <div className="mt-[18px] flex flex-col gap-3">
        {agentOrder.map((kind) => {
          const agent = draft.agents[kind];
          const locked = kind === "ceo";
          return (
            <Card key={kind} className="px-4 py-3.5">
              <div className="flex items-center gap-2.5">
                <AgentBadge agent={kind} size={22} />
                <div className="min-w-0 grow">
                  <div className="text-body font-semibold">{agents[kind].name}</div>
                  <div className="text-caption text-tx3">{agents[kind].role}</div>
                </div>
              </div>
              <div className="mt-3 grid grid-cols-4 gap-2.5">
                <LabeledControl label="Moteur">
                  <Select
                    size="sm"
                    value={agent.engine}
                    onChange={(e) => onAgent(kind, { engine: e.target.value as AgentEngine })}
                  >
                    <option value="claude-code">Claude Code</option>
                    <option value="codex">Codex CLI</option>
                  </Select>
                </LabeledControl>
                <LabeledControl label="Modèle">
                  <Input
                    size="sm"
                    mono
                    placeholder="par défaut"
                    value={agent.model ?? ""}
                    onChange={(e) => onAgent(kind, { model: e.target.value.trim() || null })}
                  />
                </LabeledControl>
                <LabeledControl label="Autonomie">
                  <Select
                    size="sm"
                    value={agent.autonomy}
                    disabled={locked}
                    title={locked ? "L'agent CEO propose seulement" : undefined}
                    onChange={(e) => onAgent(kind, { autonomy: e.target.value as Autonomy })}
                  >
                    {(Object.keys(autonomyLabels) as Autonomy[]).map((a) => (
                      <option key={a} value={a}>
                        {autonomyLabels[a]}
                      </option>
                    ))}
                  </Select>
                </LabeledControl>
                <LabeledControl label="Session max (min)">
                  <Input
                    size="sm"
                    type="number"
                    min={1}
                    max={240}
                    value={agent.maxSessionMinutes}
                    onChange={(e) => onAgent(kind, { maxSessionMinutes: Number(e.target.value) || 0 })}
                  />
                </LabeledControl>
              </div>
            </Card>
          );
        })}
      </div>
    </>
  );
}

function LabeledControl({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-caption text-tx3">{label}</span>
      {children}
    </label>
  );
}

/* ---------- Comptes et jetons ---------- */

function AccountsSection() {
  const auth = useAuthStatus().data;
  const signOut = useSignOut();
  const github = auth?.github;
  const name = github?.name ?? github?.login ?? "Compte GitHub";
  return (
    <>
      <SectionTitle title="Comptes et jetons">
        Le jeton GitHub vit dans le trousseau du système : jamais dans un fichier, jamais dans
        l'interface.
      </SectionTitle>
      <Card className="mt-[18px] px-4 py-3.5">
        <div className="flex items-center gap-3">
          <Avatar size={26} initials={initialsOf(name)} src={github?.avatarUrl} />
          <div className="min-w-0 grow">
            <div className="truncate text-body font-semibold">{name}</div>
            {github?.login && <div className="mono text-caption text-tx3">@{github.login}</div>}
          </div>
          <StatusDot tone={github?.status === "connected" ? "green" : "amber"} size={8} />
          <span className="text-caption text-tx2">
            {github?.status === "connected" ? "Connecté" : "Non connecté"}
          </span>
        </div>
        {github && github.scopes.length > 0 && (
          <div className="mt-3 flex flex-wrap items-center gap-1.5">
            <span className="text-caption text-tx3">Portées du jeton :</span>
            {github.scopes.map((s) => (
              <Badge key={s} tone="muted" mono>
                {s}
              </Badge>
            ))}
          </div>
        )}
        <div className="mt-3.5 flex items-center gap-2.5 border-t border-line pt-3">
          <span className="grow text-caption text-tx3">
            Se déconnecter retire le jeton du trousseau et efface le cache local des dépôts.
          </span>
          <Button variant="danger" size="sm" disabled={signOut.isPending} onClick={() => signOut.mutate()}>
            Se déconnecter
          </Button>
        </div>
      </Card>
    </>
  );
}

/* ---------- Machine locale ---------- */

function MachineSection() {
  const auth = useAuthStatus().data;
  return (
    <>
      <SectionTitle title="Machine locale">
        Ce qui ne quitte pas cette machine : agents de code détectés, chemins des CLI, choix du dépôt
        ouvert (<span className="mono">settings.json</span> dans le dossier de configuration de
        l'app).
      </SectionTitle>
      <div className="mt-[18px] flex flex-col gap-3">
        <CliRow name="Claude Code" info={auth?.claudeCode} />
        <CliRow name="Codex CLI" info={auth?.codex} />
      </div>
    </>
  );
}

function CliRow({ name, info }: { name: string; info?: CliAgentInfo }) {
  const ok = info?.status === "connected";
  return (
    <Card className="flex items-center gap-3 px-4 py-3">
      <StatusDot tone={ok ? "green" : info?.status === "not-found" ? "neutral" : "amber"} size={8} />
      <div className="min-w-0 grow">
        <div className="text-body font-medium">
          {name}
          {info?.version && <span className="ml-2 mono text-caption text-tx3">{info.version}</span>}
        </div>
        <div className="truncate mono text-caption text-tx3">{info?.binaryPath ?? "introuvable"}</div>
      </div>
      {info?.authMethod && <Badge tone="muted">{info.authMethod}</Badge>}
    </Card>
  );
}

/* ---------- Apparence et raccourcis ---------- */

function AppearanceSection() {
  return (
    <>
      <SectionTitle title="Apparence">
        Cadence est dessinée pour un thème sombre unique, aux couleurs des maquettes. Un thème clair
        n'est pas prévu pour la 1.0.
      </SectionTitle>
    </>
  );
}

const shortcuts: [string, ReactNode][] = [
  ["Palette de commandes", <Kbd key="k">{modKey}K</Kbd>],
  ["Fermer une fenêtre", <Kbd key="esc">Échap</Kbd>],
  ["Ouvrir la ligne choisie", <Kbd key="enter">Entrée</Kbd>],
  ["Ajouter un label (Nouvelle issue)", <Kbd key="lab">Entrée</Kbd>],
];

function ShortcutsSection() {
  return (
    <>
      <SectionTitle title="Raccourcis" />
      <div className="mt-[18px]">
        {shortcuts.map(([label, keys]) => (
          <SettingRow key={label} title={label} control={keys} />
        ))}
      </div>
    </>
  );
}
