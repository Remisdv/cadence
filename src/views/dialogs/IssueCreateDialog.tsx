/**
 * Nouvelle issue (maquette Issue-Create.dc.html) : titre et version obligatoires, créée sur
 * GitHub avec le milestone de la version, puis envoyée à l'agent CTO si demandé. L'agent
 * développeur arrive en 0.2.0.
 */
import { useId, useState, type KeyboardEvent } from "react";
import type { IssueType, Version } from "@/lib/types";
import { DialogBody, DialogClose, DialogFooter, DialogHeader } from "@/components/ui/Dialog";
import { Field, Input, Select, Textarea } from "@/components/ui/Field";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { Callout } from "@/components/ui/Callout";
import { Spinner } from "@/components/ui/Spinner";
import { SwitchField } from "@/components/ui/Switch";
import { Icon } from "@/components/icons/Icon";
import { issueTypes } from "@/lib/domain/issues";
import { compareVersions } from "@/lib/domain/versions";
import {
  useAuthStatus,
  useCreateIssue,
  useCurrentProject,
  useSettings,
  useVersions,
} from "@/lib/query/queries";

/** « 0.1.2 — sprint en cours », « 0.1.3 — sprint suivant », « 0.2.0 — palier ». */
export function versionOptions(versions: Version[], current: string | null) {
  const open = versions
    .filter((v) => v.state === "open")
    .sort((a, b) => compareVersions(a.title, b.title));
  const nextSprint = open.find(
    (v) => v.kind === "sprint" && current !== null && compareVersions(v.title, current) > 0,
  );
  return open.map((v) => ({
    value: v.title,
    label: `${v.title} — ${
      v.title === current
        ? "sprint en cours"
        : v === nextSprint
          ? "sprint suivant"
          : v.kind === "sprint"
            ? "sprint"
            : "palier"
    }`,
  }));
}

export function IssueCreateDialog({ onDone }: { onDone: () => void }) {
  const project = useCurrentProject().data;
  const versions = useVersions();
  const me = useAuthStatus().data?.github.login ?? null;
  const create = useCreateIssue();
  const ids = { title: useId(), version: useId(), type: useId(), body: useId(), labels: useId(), assignee: useId() };

  const [title, setTitle] = useState("");
  const [version, setVersion] = useState("");
  const [issueType, setIssueType] = useState<IssueType>("feat");
  const [body, setBody] = useState("");
  const [labels, setLabels] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [assignToMe, setAssignToMe] = useState(true);
  const sendDefault = useSettings().data?.issues.sendToCtoOnCreate ?? true;
  const [sendToCto, setSendToCto] = useState<boolean | null>(null);
  const [submitted, setSubmitted] = useState(false);

  const options = versionOptions(versions.data ?? [], project?.currentVersion ?? null);
  const titleError = submitted && !title.trim() ? "Une issue doit avoir un titre." : null;
  const versionError = submitted && !version ? "Une issue doit être rattachée à une version." : null;

  const addLabel = (raw: string) => {
    const label = raw.trim().replace(/,$/, "");
    if (label && !labels.includes(label)) setLabels([...labels, label]);
    setDraft("");
  };
  const onLabelKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      addLabel(draft);
    } else if (e.key === "Backspace" && draft === "" && labels.length > 0) {
      setLabels(labels.slice(0, -1));
    }
  };

  const submit = () => {
    setSubmitted(true);
    if (!title.trim() || !version) return;
    create.mutate(
      {
        title: title.trim(),
        version,
        issueType,
        body: body.trim() || undefined,
        labels: draft.trim() ? [...labels, draft.trim()] : labels,
        assignee: assignToMe && me ? me : undefined,
        sendToCto: sendToCto ?? sendDefault,
      },
      { onSuccess: onDone },
    );
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <DialogHeader title="Nouvelle issue" icon="sprint" meta={project?.repo.fullName} />
      <DialogBody>
        <Field label="Titre" htmlFor={ids.title} required error={titleError}>
          <Input
            id={ids.title}
            autoFocus
            value={title}
            invalid={Boolean(titleError)}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Message d'erreur lisible quand le quota API GitHub est atteint"
          />
        </Field>

        <div className="flex gap-3">
          <Field label="Version" htmlFor={ids.version} required error={versionError} className="flex-1">
            <Select
              id={ids.version}
              value={version}
              invalid={Boolean(versionError)}
              onChange={(e) => setVersion(e.target.value)}
              disabled={versions.isPending}
            >
              <option value="">{versions.isPending ? "Lecture des versions…" : "Choisir une version…"}</option>
              {options.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Type" htmlFor={ids.type} className="flex-1">
            <Select id={ids.type} value={issueType} onChange={(e) => setIssueType(e.target.value as IssueType)}>
              {issueTypes.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </Select>
          </Field>
        </div>

        <Field label="Description" htmlFor={ids.body} aside="facultative — l'agent CTO la complétera">
          <Textarea
            id={ids.body}
            rows={4}
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder="Deux lignes suffisent. L'agent CTO reprendra la main pour structurer, poser les questions manquantes et proposer des critères d'acceptation."
          />
        </Field>

        <div className="flex gap-3">
          <Field label="Labels" htmlFor={ids.labels} className="flex-1">
            <div className="flex min-h-9 flex-wrap items-center gap-1.5 rounded-lg border border-line2 bg-p2 px-[9px] py-[7px] focus-within:border-clay">
              {labels.map((l) => (
                <Badge key={l} tone="muted">
                  {l}
                  <button
                    type="button"
                    aria-label={`Retirer ${l}`}
                    onClick={() => setLabels(labels.filter((x) => x !== l))}
                    className="flex text-tx3 hover:text-tx"
                  >
                    <Icon name="close" size={11} />
                  </button>
                </Badge>
              ))}
              <input
                id={ids.labels}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={onLabelKey}
                onBlur={() => draft && addLabel(draft)}
                placeholder={labels.length ? "" : "+ ajouter (Entrée)"}
                className="min-w-[90px] grow bg-transparent text-small text-tx outline-none placeholder:text-tx3"
              />
            </div>
          </Field>
          <Field label="Assigné" htmlFor={ids.assignee} className="flex-1">
            <Select
              id={ids.assignee}
              value={assignToMe ? "me" : "none"}
              onChange={(e) => setAssignToMe(e.target.value === "me")}
            >
              <option value="me">vous{me ? ` (${me})` : ""}</option>
              <option value="none">personne</option>
            </Select>
          </Field>
        </div>

        <div className="flex flex-col gap-[11px] rounded-[10px] border border-line bg-p2 px-3.5 py-[13px]">
          <SwitchField
            checked={sendToCto ?? sendDefault}
            onCheckedChange={setSendToCto}
            label="Envoyer à l'agent CTO à la création"
            hint="Il complétera la description et posera ses questions dans la foulée."
          />
          <SwitchField
            checked={false}
            onCheckedChange={() => {}}
            disabled
            label="Lancer l'agent développeur ensuite"
            hint="Ne s'active qu'une fois la description validée (version 0.2.0)."
          />
        </div>

        {create.isError && <Callout tone="danger">{create.error.message}</Callout>}
      </DialogBody>
      <DialogFooter>
        <span className="text-caption text-tx3">Sera créée sur GitHub avec le milestone correspondant.</span>
        <div className="grow" />
        <DialogClose asChild>
          <Button variant="ghost">Annuler</Button>
        </DialogClose>
        <Button type="submit" variant="primary" icon={create.isPending ? undefined : "plus"} disabled={create.isPending}>
          {create.isPending && <Spinner />}
          Créer l'issue
        </Button>
      </DialogFooter>
    </form>
  );
}
