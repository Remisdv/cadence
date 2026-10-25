/**
 * Création d'un dépôt GitHub depuis un sélecteur de dépôt (`RepoPicker`, `RepoSwitcher`).
 * Le dépôt est créé sous le compte ou l'organisation choisi ; l'appelant l'ouvre ensuite.
 */
import { useEffect, useId, useState, type FormEvent } from "react";
import { Icon } from "@/components/icons/Icon";
import { Button } from "@/components/ui/Button";
import { Input, Label, Select } from "@/components/ui/Field";
import { Spinner } from "@/components/ui/Spinner";
import { SwitchField } from "@/components/ui/Switch";
import type { Repo } from "@/lib/types";
import { useCreateRepo, useRepoOwners } from "@/lib/query/queries";

const VALID_NAME = /^[A-Za-z0-9._-]{1,100}$/;

export interface CreateRepoFormProps {
  /** Nom pré-rempli (ce qui était tapé dans la recherche). */
  initialName: string;
  onBack: () => void;
  /** Dépôt créé : l'appelant l'ouvre. */
  onCreated: (repo: Repo) => void;
  /** Ouverture en cours après la création (bouton désactivé). */
  opening?: boolean;
}

export function CreateRepoForm({ initialName, onBack, onCreated, opening }: CreateRepoFormProps) {
  const ids = { owner: useId(), name: useId(), description: useId() };
  const owners = useRepoOwners(true);
  const create = useCreateRepo();

  const [owner, setOwner] = useState("");
  // GitHub remplace les espaces par des tirets : on le fait dès la saisie.
  const [name, setName] = useState(initialName.trim().replace(/\s+/g, "-"));
  const [description, setDescription] = useState("");
  const [isPrivate, setPrivate] = useState(true);
  const [autoInit, setAutoInit] = useState(true);

  // Propriétaire par défaut : le compte personnel.
  useEffect(() => {
    if (!owner && owners.data?.[0]) setOwner(owners.data[0].login);
  }, [owner, owners.data]);

  const nameValid = VALID_NAME.test(name) && name !== "." && name !== "..";
  const busy = create.isPending || opening === true;
  const canSubmit = nameValid && owner !== "" && !busy;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    create.mutate(
      { owner, name, description: description.trim() || undefined, private: isPrivate, autoInit },
      { onSuccess: onCreated },
    );
  };

  return (
    <form onSubmit={submit}>
      <div className="flex h-11 items-center gap-2 border-b border-line px-2">
        <Button
          variant="quiet"
          size="sm"
          iconOnly
          aria-label="Revenir à la liste"
          onClick={onBack}
          className="[&_svg]:rotate-180"
          icon="arrowRight"
        />
        <span className="font-display text-title-sm font-semibold">Nouveau dépôt</span>
      </div>

      <div className="flex flex-col gap-3 p-3">
        <div className="flex flex-col gap-1.5">
          <Label htmlFor={ids.owner}>Propriétaire</Label>
          <Select
            id={ids.owner}
            size="sm"
            value={owner}
            disabled={owners.isPending}
            onChange={(e) => setOwner(e.target.value)}
          >
            {owners.isPending && <option value="">Chargement…</option>}
            {owners.data?.map((o) => (
              <option key={o.login} value={o.login}>
                {o.login}
                {o.kind === "organization" ? " (organisation)" : ""}
              </option>
            ))}
          </Select>
          {owners.isError && <p className="text-caption text-red">{owners.error.message}</p>}
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor={ids.name} required>
            Nom
          </Label>
          <Input
            id={ids.name}
            size="sm"
            mono
            autoFocus
            spellCheck={false}
            placeholder="mon-projet"
            value={name}
            invalid={name !== "" && !nameValid}
            onChange={(e) => setName(e.target.value.replace(/\s+/g, "-"))}
          />
          <p className="text-caption text-tx3">
            {name !== "" && !nameValid ? (
              <span className="text-red">Lettres, chiffres, « - », « _ » et « . » uniquement.</span>
            ) : (
              <>
                github.com/
                <span className="mono text-tx2">
                  {owner || "…"}/{name || "…"}
                </span>
              </>
            )}
          </p>
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor={ids.description} aside="facultative">
            Description
          </Label>
          <Input
            id={ids.description}
            size="sm"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </div>

        <SwitchField
          checked={isPrivate}
          onCheckedChange={setPrivate}
          label="Dépôt privé"
          hint="Visible seulement par vous et les personnes invitées."
        />
        <SwitchField
          checked={autoInit}
          onCheckedChange={setAutoInit}
          label="Initialiser avec un README"
          hint="Crée la branche par défaut tout de suite."
        />

        {create.error && (
          <p role="alert" className="flex items-start gap-1.5 text-caption text-red">
            <Icon name="warning" size={13} className="mt-px" />
            {create.error.message}
          </p>
        )}
      </div>

      <div className="flex h-12 items-center gap-2 border-t border-line bg-p2 px-3">
        <div className="grow" />
        <Button variant="ghost" size="sm" onClick={onBack}>
          Annuler
        </Button>
        <Button
          type="submit"
          variant="primary"
          size="sm"
          icon={busy ? undefined : "plus"}
          disabled={!canSubmit}
        >
          {busy && <Spinner size={12} className="text-clay-ink" />}
          Créer et ouvrir
        </Button>
      </div>
    </form>
  );
}
