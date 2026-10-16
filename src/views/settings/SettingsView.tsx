/**
 * Réglages (maquette Settings.dc.html) : navigation à gauche, section à droite.
 *
 * « Comptes et jetons » et « Machine locale » montrent ce qui reste sur cette machine
 * (trousseau, `settings.json`). Les réglages de Cadence 2 (IA, plugins, outils) suivront la
 * maquette V2-Settings.dc.html.
 */
import type { ReactNode } from "react";
import type { CliAgentInfo } from "@/lib/types";
import type { ViewProps } from "@/views/registry";
import type { SettingsSection } from "@/stores/navigation";
import { Page } from "@/components/layout/Page";
import { Icon, type IconName } from "@/components/icons/Icon";
import { SettingRow } from "@/components/domain/Panels";
import { Avatar, initialsOf } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Kbd } from "@/components/ui/Kbd";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { StatusDot } from "@/components/ui/StatusDot";
import { cn } from "@/lib/cn";
import { modKey } from "@/lib/hooks/usePlatform";
import { useAuthStatus, useSignOut } from "@/lib/query/queries";
import { useNavigation } from "@/stores/navigation";
import { AboutSection, AppVersionLine } from "./AboutSection";

const sections: { id: SettingsSection; label: string; icon: IconName }[] = [
  { id: "accounts", label: "Comptes et jetons", icon: "key" },
  { id: "machine", label: "Machine locale", icon: "layers" },
  { id: "appearance", label: "Apparence", icon: "eye" },
  { id: "shortcuts", label: "Raccourcis", icon: "sparkle" },
  { id: "about", label: "À propos", icon: "rocket" },
];

export default function SettingsView({ view }: ViewProps<"settings">) {
  const navigate = useNavigation((s) => s.navigate);
  const section = view.section ?? "accounts";

  return (
    <Page title="Réglages" subtitle="préférences de cette machine" padding="none">
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
          <AppVersionLine onOpen={() => navigate({ id: "settings", section: "about" })} />
          <br />
          Tauri 2 · React 19
        </div>
      </nav>

      <div className="min-w-0 grow overflow-y-auto px-[26px] py-5">
        <div className="max-w-[780px]">
          {section === "accounts" && <AccountsSection />}
          {section === "machine" && <MachineSection />}
          {section === "appearance" && <AppearanceSection />}
          {section === "shortcuts" && <ShortcutsSection />}
          {section === "about" && <AboutSection />}
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
        Ce qui ne quitte pas cette machine : IA de code détectées, chemins des CLI, choix du dépôt
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
