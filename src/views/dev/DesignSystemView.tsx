/**
 * Galerie de composants (dev) : chaque brique du design system rendue avec les données
 * des maquettes. Sert de référence visuelle et de banc d'essai avant d'assembler un écran.
 * Accès : palette ⌘K → « Galerie de composants », ou `?view=design-system`.
 */
import { useState, type ReactNode } from "react";
import { Page } from "@/components/layout/Page";
import { Icon, iconNames } from "@/components/icons/Icon";
import { Avatar } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card, CardHeader } from "@/components/ui/Card";
import { Dialog, DialogBody, DialogFooter, DialogHeader } from "@/components/ui/Dialog";
import { Field, Input, Select, Textarea } from "@/components/ui/Field";
import { KeyValue } from "@/components/ui/KeyValue";
import { Kbd } from "@/components/ui/Kbd";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { RadioCard, RadioCardGroup, RadioOption } from "@/components/ui/RadioCard";
import { SectionLabel } from "@/components/ui/SectionLabel";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { Switch, SwitchField } from "@/components/ui/Switch";
import { ActivityItem, Em } from "@/components/domain/ActivityItem";
import { AgentBadge } from "@/components/domain/AgentBadge";
import { AgentStatusCard, RunLogRow } from "@/components/domain/AgentConsole";
import { AgentQuestion } from "@/components/domain/AgentQuestion";
import { TypeBadge, VersionBadge } from "@/components/domain/Badges";
import { Burndown } from "@/components/domain/Burndown";
import { DiffViewer, TextDiff } from "@/components/domain/Diff";
import { Markdown } from "@/components/domain/Markdown";
import { FilterChip, FilterInput, PillTabs } from "@/components/domain/Filters";
import {
  BranchRef,
  CiStatus,
  Diffstat,
  InlineMeta,
  IssueStateIcon,
  PrStateIcon,
} from "@/components/domain/Inline";
import { KanbanColumn } from "@/components/domain/IssueCard";
import {
  EmptyValue,
  FooterBar,
  Note,
  PropertyList,
  PropertyRow,
  SettingRow,
  TitledCard,
} from "@/components/domain/Panels";
import { SessionConsole } from "@/components/domain/SessionConsole";
import { MeterList, StatCard, StatStrip } from "@/components/domain/Stats";
import { OnboardingSteps, PipelineStages, StepTimeline } from "@/components/domain/Steps";
import { agentOrder, devSteps } from "@/lib/domain/agents";
import { issueTypes, statusOrder } from "@/lib/domain/issues";
import { currentVersion, issueDetail, issues } from "@/lib/ipc/mock/fixtures";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-title-sm">{title}</h2>
      {children}
    </section>
  );
}

function Row({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={`flex flex-wrap items-center gap-2.5 ${className ?? ""}`}>{children}</div>;
}

const swatches = [
  ["bg", "bg-bg"],
  ["panel", "bg-panel"],
  ["p2", "bg-p2"],
  ["p3", "bg-p3"],
  ["p4", "bg-p4"],
  ["line", "bg-line"],
  ["line2", "bg-line2"],
  ["tx", "bg-tx"],
  ["tx2", "bg-tx2"],
  ["tx3", "bg-tx3"],
  ["clay", "bg-clay"],
  ["clay-d", "bg-clay-d"],
  ["green", "bg-green"],
  ["green-d", "bg-green-d"],
  ["blue", "bg-blue"],
  ["blue-d", "bg-blue-d"],
  ["purple", "bg-purple"],
  ["purple-d", "bg-purple-d"],
  ["amber", "bg-amber"],
  ["amber-d", "bg-amber-d"],
  ["red", "bg-red"],
  ["red-d", "bg-red-d"],
] as const;

export default function DesignSystemView() {
  const [switchOn, setSwitchOn] = useState(true);
  const [next, setNext] = useState("0.1.3");
  const [carry, setCarry] = useState("move");
  const [tab, setTab] = useState("scope");
  const [answer, setAnswer] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const question = issueDetail(156).questions[0]!;

  return (
    <Page
      title="Galerie de composants"
      subtitle="Design system Cadence — tokens et composants des maquettes"
      actions={
        <Button variant="primary" icon="plus" onClick={() => setDialogOpen(true)}>
          Ouvrir une modale
        </Button>
      }
    >
      <div className="flex max-w-[1180px] flex-col gap-8 pb-10">
        <Section title="Couleurs">
          <div className="grid grid-cols-11 gap-2">
            {swatches.map(([name, cls]) => (
              <div key={name} className="flex flex-col gap-1">
                <div className={`h-10 rounded-lg border border-line ${cls}`} />
                <span className="mono text-micro text-tx3">{name}</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Typographie">
          <Card className="flex flex-col gap-2 p-4">
            <h1 className="text-display">Display 27 — Space Grotesk</h1>
            <h1 className="text-display-sm">Titre d'onboarding 24</h1>
            <h1 className="text-title">Titre de page 15</h1>
            <h3 className="text-body">Titre de carte 13</h3>
            <p className="text-body text-tx2">
              Corps 13 — IBM Plex Sans, tx2. Tout reste lisible sur github.com.
            </p>
            <p className="text-caption text-tx3">Légende 11.5 — tx3</p>
            <p className="mono text-tiny text-tx3">Mono 11 — #156 · feat/142-github-device-flow</p>
            <SectionLabel>Section label 10.5</SectionLabel>
          </Card>
        </Section>

        <Section title={`Icônes (${iconNames.length})`}>
          <div className="grid grid-cols-9 gap-2">
            {iconNames.map((name) => (
              <div
                key={name}
                className="flex flex-col items-center gap-1.5 rounded-lg border border-line bg-panel py-2.5"
              >
                <Icon name={name} size={17} />
                <span className="mono text-micro text-tx3">{name}</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Boutons">
          <Row>
            <Button variant="primary" icon="plus">
              Nouvelle issue
            </Button>
            <Button variant="secondary" icon="tag">
              Clôturer la version
            </Button>
            <Button variant="ghost">Annuler</Button>
            <Button variant="quiet" iconOnly icon="close" aria-label="Fermer" />
            <Button variant="success" icon="check">
              Approuver
            </Button>
            <Button variant="danger" icon="close">
              Demander des changements
            </Button>
            <Button variant="purple" icon="review">
              Examiner les propositions
            </Button>
          </Row>
          <Row>
            <Button variant="ghost" size="xs" icon="file">
              Éditer
            </Button>
            <Button variant="ghost" size="sm" icon="file">
              Modifier
            </Button>
            <Button variant="secondary" size="compact" icon="play">
              Ouvrir
            </Button>
            <Button variant="secondary" size="md" icon="refresh">
              Relancer le test
            </Button>
            <Button variant="primary" size="tall" icon="play">
              Lancer sur 2 tâches
            </Button>
            <Button variant="primary" size="lg" iconRight="arrowRight">
              Continuer
            </Button>
            <Button variant="primary" size="lg" icon="github">
              Ouvrir GitHub
            </Button>
          </Row>
        </Section>

        <Section title="Badges et indicateurs">
          <Row>
            <Badge tone="neutral">inactif</Badge>
            <Badge tone="muted">area/api</Badge>
            <Badge tone="clay">requis</Badge>
            <Badge tone="green">Connecté</Badge>
            <Badge tone="blue">type/feat</Badge>
            <Badge tone="purple">fusionnée</Badge>
            <Badge tone="amber" dot>
              5 à valider
            </Badge>
            <Badge tone="red">à revoir</Badge>
            <Badge tone="green" icon="issue">
              Ouverte
            </Badge>
            <Badge tone="green" dot>
              #142 · 04:12
            </Badge>
          </Row>
          <Row>
            {issueTypes.map((t) => (
              <TypeBadge key={t} type={t} />
            ))}
            <VersionBadge version="0.1.1" stage="shipped" />
            <VersionBadge version="0.1.2" stage="current" />
            <VersionBadge version="0.1.3" stage="planned" />
            <VersionBadge version="0.2.0" stage="milestone" />
            <VersionBadge version="1.0.0" stage="vision" />
            <VersionBadge version="0.1.2" stage="current" tag />
          </Row>
          <Row>
            <IssueStateIcon status="todo" />
            <IssueStateIcon status="done" />
            <PrStateIcon state="draft" />
            <PrStateIcon state="open" />
            <PrStateIcon state="merged" />
            <CiStatus state="pending" />
            <CiStatus state="success" passed={4} total={4} />
            <CiStatus state="failure" passed={3} total={4} />
            <InlineMeta icon="warning" tone="amber">
              description incomplète
            </InlineMeta>
            <InlineMeta icon="review" tone="amber">
              2 questions
            </InlineMeta>
            <InlineMeta icon="clock" dim>
              en attente
            </InlineMeta>
            <Diffstat additions={412} deletions={87} />
            <BranchRef head="feat/142-github-device-flow" base="main" />
          </Row>
          <Row>
            <StatusDot tone="neutral" />
            <StatusDot tone="amber" />
            <StatusDot tone="green" pulse />
            <StatusDot tone="clay" size={7} />
            <StatusDot tone="amber" size={8} pulse />
            <Kbd>Ctrl+K</Kbd>
            <Kbd boxed>esc</Kbd>
            <Spinner />
          </Row>
        </Section>

        <Section title="Avatars et agents">
          <Row>
            <Avatar initials="RL" size={19} />
            <Avatar initials="RL" size={24} />
            <Avatar initials="MB" size={21} tone="neutral" />
            <Avatar initials="CD" size={26} tone="neutral" />
            {agentOrder.map((a) => (
              <AgentBadge key={a} agent={a} size={17} />
            ))}
            {agentOrder.map((a) => (
              <AgentBadge key={a} agent={a} size={22} />
            ))}
            {agentOrder.map((a) => (
              <AgentBadge key={a} agent={a} size={32} />
            ))}
          </Row>
        </Section>

        <Section title="Formulaires">
          <Card className="grid grid-cols-2 gap-4 p-[18px]">
            <Field label="Titre" htmlFor="ds-title" required className="col-span-2">
              <Input
                id="ds-title"
                defaultValue="Message d'erreur lisible quand le quota API GitHub est atteint"
              />
            </Field>
            <Field
              label="Version"
              htmlFor="ds-version"
              required
              error="Une issue doit être rattachée à une version."
            >
              <Select id="ds-version" invalid defaultValue="">
                <option value="">Choisir une version…</option>
                <option>0.1.2 — sprint en cours</option>
              </Select>
            </Field>
            <Field label="Type" htmlFor="ds-type">
              <Select id="ds-type">
                {issueTypes.map((t) => (
                  <option key={t}>{t}</option>
                ))}
              </Select>
            </Field>
            <Field
              label="Description"
              htmlFor="ds-desc"
              aside="facultative — l'agent CTO la complétera"
              className="col-span-2"
            >
              <Textarea id="ds-desc" rows={3} placeholder="Deux lignes suffisent." />
            </Field>
            <SwitchField
              checked={switchOn}
              onCheckedChange={setSwitchOn}
              label="Envoyer à l'agent CTO à la création"
              hint="Il complétera la description et posera ses questions dans la foulée."
            />
            <Row>
              <Switch checked={false} onCheckedChange={() => {}} aria-label="Exemple éteint" />
              <Input size="sm" mono placeholder="ou saisir : 0.3.0" className="w-48" />
            </Row>
            <RadioCardGroup value={next} onValueChange={setNext} aria-label="Version suivante">
              <RadioCard
                value="0.1.3"
                leading={<VersionBadge version="0.1.3" />}
                title="Sprint suivant"
                description="Incrémente le dernier segment."
              />
              <RadioCard
                value="0.2.0"
                leading={<VersionBadge version="0.2.0" stage="milestone" />}
                title="Nouveau palier"
                description="Pour un ensemble de fonctionnalités qui change l'usage."
              />
            </RadioCardGroup>
            <RadioCardGroup value={carry} onValueChange={setCarry} aria-label="Issues ouvertes">
              <RadioOption value="move">Reporter sur 0.1.3</RadioOption>
              <RadioOption value="none">Laisser sans version</RadioOption>
              <RadioOption value="close">Fermer comme non retenues</RadioOption>
            </RadioCardGroup>
          </Card>
        </Section>

        <Section title="Filtres et onglets">
          <Row>
            <FilterInput placeholder="Filtrer les issues…" />
            <FilterChip icon="tag" label="Version" value="0.1.2 et 0.1.3" />
            <FilterChip icon="issue" label="État" />
            <FilterChip icon="user" label="Assigné" />
          </Row>
          <Row>
            <PillTabs
              aria-label="Détail de version"
              value={tab}
              onChange={setTab}
              items={[
                { value: "scope", label: "Portée" },
                { value: "notes", label: "Notes de version" },
                { value: "deploy", label: "Déploiement" },
              ]}
            />
            <PillTabs
              variant="mono"
              value="142"
              onChange={() => {}}
              items={[
                { value: "142", label: "#142" },
                { value: "147", label: "#147" },
              ]}
            />
          </Row>
        </Section>

        <Section title="KPI, jauges et burndown">
          <div className="flex gap-3">
            <StatCard label="Portée" value="18" hint="issues · 11 fermées" />
            <StatCard label="Avancement" value="61 %" hint="+3 aujourd'hui" tone="clay" />
            <StatCard label="PR ouvertes" value="4" hint="1 écrite par un agent" tone="green" />
            <StatCard label="Revues à faire" value="3" hint="la plus ancienne : 2 j" tone="amber" />
            <StatCard label="Rythme" value="2,4" hint="issues fermées / jour" tone="blue" />
          </div>
          <StatStrip
            items={[
              { label: "Fermées", value: "11 / 18", suffix: "61 %", tone: "clay" },
              { label: "PR fusionnées", value: "12", tone: "green" },
              { label: "Tests", value: "6", suffix: "sur 14" },
              { label: "Rythme", value: "1,2", suffix: "issue / jour", tone: "amber" },
            ]}
          />
          <div className="grid grid-cols-3 gap-3">
            <Card className="p-[15px]">
              <CardHeader
                title="Burndown"
                actions={<span className="text-tiny text-amber">1,5 j de retard</span>}
              />
              <div className="mt-3">
                <Burndown
                  points={currentVersion.burndown}
                  scope={18}
                  days={11}
                  start="2026-09-09"
                  end="2026-09-23"
                />
              </div>
            </Card>
            <Card className="p-[15px]">
              <CardHeader title="Progression" />
              <MeterList
                className="mt-3"
                items={[
                  { label: "Fermées", value: "11", percent: 61, tone: "green" },
                  { label: "En revue", value: "4", percent: 22, tone: "blue" },
                  { label: "En cours", value: "3", percent: 17, tone: "clay" },
                  { label: "À faire", value: "4", percent: 22, tone: "neutral" },
                ]}
              />
            </Card>
            <Card className="p-[15px]">
              <CardHeader title="Activité des agents" actions={<a href="#console">Console</a>} />
              <div className="mt-1.5">
                <ActivityItem agent="dev" time="il y a 4 min">
                  Branche <Em mono>feat/142-github-device-flow</Em> créée, PR #88 ouverte en
                  brouillon.
                </ActivityItem>
                <ActivityItem agent="ceo" time="il y a 1 h">
                  <Em>5 issues proposées</Em> pour la version 0.2.0.
                </ActivityItem>
              </div>
            </Card>
          </div>
        </Section>

        <Section title="Tableau du sprint">
          <Card className="p-[15px]">
            <CardHeader title="Tableau du sprint" actions={<Badge>Groupé par état</Badge>} />
            <div className="mt-[13px] flex items-start gap-[11px]">
              {statusOrder.map((status) => (
                <KanbanColumn
                  key={status}
                  status={status}
                  issues={issues.filter((i) => i.status === status).slice(0, 3)}
                />
              ))}
            </div>
          </Card>
        </Section>

        <Section title="Étapes">
          <div className="grid grid-cols-3 gap-3">
            <Card className="p-6 bg-aside">
              <OnboardingSteps
                steps={["Compte GitHub", "Agents de code", "Projet et mapping"]}
                current={1}
              />
            </Card>
            <Card className="p-4">
              <StepTimeline
                steps={devSteps.map((title, i) => ({
                  title,
                  state: i < 2 ? "done" : i === 2 ? "current" : "pending",
                  detail: i === 0 ? "18 fichiers lus · 3 modules identifiés" : undefined,
                }))}
              />
            </Card>
            <Card className="flex flex-col gap-3 p-4">
              <ProgressBar value={52} tone="green" track="green" height={3} />
              <ProgressBar value={61} height={4} />
              <ProgressBar value={34} tone="purple" height={5} />
              <KeyValue label="Binaire" value="/opt/homebrew/bin/claude" />
              <KeyValue label="Version" value="2.4.1" last />
            </Card>
          </div>
          <PipelineStages
            stages={[
              { title: "Pré-vol", detail: "18 issues fermées · CI verte", state: "success" },
              { title: "Build", detail: "3 artefacts", state: "success" },
              {
                title: "Tests de vérification",
                detail: "13 / 14 · 1 échec sur #128",
                state: "failure",
              },
              {
                title: "Documentation",
                detail: "Changelog rédigé · docs/ en cours",
                state: "running",
                icon: "book",
              },
              {
                title: "Tag et release",
                detail: "en attente des tests",
                state: "pending",
                icon: "tag",
              },
            ]}
          />
        </Section>

        <Section title="Console d'agent et diff">
          <div className="grid grid-cols-[320px_1fr] gap-3">
            <Card className="h-[300px] overflow-hidden">
              <SessionConsole
                className="h-full"
                header={
                  <>
                    <StatusDot tone="blue" />
                    <span className="grow text-small text-tx2">Session Claude Code</span>
                    <span className="mono text-micro text-tx3">02:41</span>
                  </>
                }
                footer={<span className="grow mono text-micro text-tx3">opus · 24 k contexte</span>}
                lines={[
                  { kind: "system", text: "› worktree ~/.cadence/wt/142" },
                  { kind: "read", text: "read  src-tauri/src/auth/mod.rs" },
                  { kind: "message", text: "Le flux PAT existe déjà. J'ajoute le device flow." },
                  { kind: "write", text: "write src-tauri/src/auth/device_flow.rs" },
                  { kind: "success", text: "✓ 142 lignes" },
                  { kind: "question", text: "? 2 points restent ambigus" },
                ]}
              />
            </Card>
            <Card className="h-[300px] overflow-hidden">
              <DiffViewer
                file="src/components/Rail.css"
                additions={2}
                deletions={1}
                lines={[
                  { type: "ctx", text: ".rail {" },
                  { type: "del", text: "  height: 100vh;" },
                  { type: "add", text: "  height: 100%;" },
                  { type: "add", text: "  min-height: 0;" },
                  { type: "ctx", text: "}" },
                ]}
              />
            </Card>
          </div>
          <Card className="p-4">
            <Markdown
              source={[
                "## Contexte",
                "Le client renvoie `403` avec **x-ratelimit-remaining: 0**.",
                "",
                "## Attendu",
                "- Distinguer quota et permissions",
                "- Afficher l'heure de réinitialisation",
                "",
                "- [ ] Bandeau, jamais de toast rouge",
                "- [x] Heure dans le fuseau local",
              ].join("\n")}
            />
          </Card>
          <TextDiff
            lines={[
              { type: "del", text: "Quand on dépasse le quota, l'app affiche une erreur brute." },
              { type: "add", text: "Contexte. Au-delà de 5 000 requêtes/h, GitHub renvoie 403." },
              {
                type: "add",
                text: "Attendu. Un bandeau daté indique l'heure de réinitialisation.",
              },
            ]}
          />
        </Section>

        <Section title="Console des agents">
          <div className="flex gap-3">
            <AgentStatusCard
              agent="ceo"
              autonomy="Propose seulement"
              status={{ label: "5 à valider", tone: "amber", dot: true }}
              description="Lit la roadmap et les versions livrées, puis dérive les issues probables des paliers à venir."
              note="5 issues proposées pour 0.2.0"
              footer="Dernier passage : il y a 1 h · cible 0.2.0"
              highlight="amber"
              onOpen={() => {}}
            />
            <AgentStatusCard
              agent="dev"
              autonomy="Applique après validation"
              status={{ label: "#142 · 04:12", tone: "green", dot: true }}
              description="Crée la branche et la PR en brouillon, puis implémente dans un worktree isolé."
              note="PR #88 ouverte en brouillon"
              footer="Étape 3/6 : implémentation"
              highlight="green"
              onOpen={() => {}}
            />
            <AgentStatusCard
              agent="deploy"
              autonomy="Applique après validation"
              status={{ label: "inactif", tone: "neutral" }}
              description="Publie la version, rédige la documentation et rejoue les tests d'acceptation."
              footer="Dernier passage : hier · v0.1.1"
              onOpen={() => {}}
            />
          </div>
          <Card className="overflow-hidden">
            <RunLogRow agent="dev" time="11:24" status={{ label: "en cours", tone: "blue" }} onClick={() => {}}>
              Tâche #142 · étape 3/6 · PR #88
            </RunLogRow>
            <RunLogRow agent="ceo" time="10:31" status={{ label: "à valider", tone: "amber" }}>
              Passage sur la version 0.2.0 · 5 à valider
            </RunLogRow>
            <RunLogRow agent="cto" time="hier" status={{ label: "appliqué", tone: "green" }}>
              Passage sur la version 0.1.2 · 18/18 traitées · 12 appliquées
            </RunLogRow>
          </Card>
        </Section>

        <Section title="Panneaux">
          <div className="grid grid-cols-2 gap-3">
            <TitledCard
              title="Portée"
              actions={
                <>
                  <Badge tone="green">7 ouvertes</Badge>
                  <Badge tone="purple">11 fermées</Badge>
                </>
              }
              footer={
                <FooterBar height={44} hint="Toutes les actions sont écrites sur GitHub">
                  <Button variant="ghost" icon="close">
                    Fermer
                  </Button>
                </FooterBar>
              }
            >
              <div className="px-3.5 py-2">
                <PropertyList>
                  <PropertyRow label="Version">
                    <VersionBadge version="0.1.2" />
                    <span className="text-tiny text-tx3">milestone #4</span>
                  </PropertyRow>
                  <PropertyRow label="Assigné">
                    <Avatar initials="RL" size={20} />
                    <span className="text-body-sm">vous</span>
                  </PropertyRow>
                  <PropertyRow label="Branche">
                    <EmptyValue />
                  </PropertyRow>
                </PropertyList>
              </div>
            </TitledCard>
            <Card className="flex flex-col gap-3 p-4">
              <SettingRow
                title="Préfixe des tags"
                description="Posé sur main à la clôture."
                control={
                  <Select size="sm" className="w-[90px]">
                    <option>v</option>
                    <option>aucun</option>
                  </Select>
                }
              />
              <SettingRow
                title="Convention de type"
                control={<span className="mono text-small text-clay">type/*</span>}
              />
              <SettingRow title="Version obligatoire" control={<Badge>verrouillé</Badge>} />
              <Note>2 questions sans réponse sur #156</Note>
              <Note tone="purple">
                L'agent CEO reste en « Proposer » : il ne crée jamais d'issue seul.
              </Note>
            </Card>
          </div>
          <Callout tone="info">
            Cadence n'écrit jamais sur GitHub sans confirmation tant que l'autonomie des agents
            reste sur <span className="text-tx">Proposer</span>.
          </Callout>
          <Callout tone="warning">
            Deux PR de cette version sont encore ouvertes (<span className="mono">#84</span>,{" "}
            <span className="mono">#88</span>).
          </Callout>
          <Card className="max-w-[420px] bg-p2 p-3.5">
            <div className="flex items-center gap-2">
              <AgentBadge agent="cto" size={22} />
              <span className="grow text-body-sm font-semibold">Agent CTO</span>
              <Badge tone="amber">2 questions</Badge>
            </div>
            <AgentQuestion
              question={question}
              value={answer}
              onChange={setAnswer}
              suggestion="Oui, avec l'heure exacte"
            />
          </Card>
        </Section>
      </div>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen} width={620}>
        <DialogHeader title="Clôturer la version 0.1.2" icon="tag" />
        <DialogBody>
          <p className="text-body-sm text-tx2">
            Exemple de modale : en-tête 50px, corps, pied 56px sur fond p2.
          </p>
        </DialogBody>
        <DialogFooter>
          <SwitchField
            checked={switchOn}
            onCheckedChange={setSwitchOn}
            label="Générer les notes de version"
          />
          <div className="grow" />
          <Button variant="ghost" onClick={() => setDialogOpen(false)}>
            Annuler
          </Button>
          <Button variant="primary" icon="check" onClick={() => setDialogOpen(false)}>
            Clôturer et passer en 0.1.3
          </Button>
        </DialogFooter>
      </Dialog>
    </Page>
  );
}
