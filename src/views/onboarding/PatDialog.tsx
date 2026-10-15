import { useId, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog, DialogBody, DialogFooter, DialogHeader } from "@/components/ui/Dialog";
import { Field, Input } from "@/components/ui/Field";
import { Spinner } from "@/components/ui/Spinner";
import { openExternal } from "@/lib/platform";
import { useSetGithubToken } from "@/lib/query/queries";

const NEW_TOKEN_URL =
  "https://github.com/settings/tokens/new?scopes=repo,read:org,workflow,read:user&description=Cadence";

export interface PatDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Repli par jeton personnel (GitHub Enterprise Server, ou pas d'application OAuth).
 * Le jeton part vers Rust, qui le vérifie puis le range dans le trousseau ; il n'est jamais relu.
 */
export function PatDialog({ open, onOpenChange }: PatDialogProps) {
  const id = useId();
  const [token, setToken] = useState("");
  const save = useSetGithubToken();

  const close = (next: boolean) => {
    if (!next) {
      setToken("");
      save.reset();
    }
    onOpenChange(next);
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    save.mutate(token, { onSuccess: () => close(false) });
  };

  return (
    <Dialog open={open} onOpenChange={close} width={540}>
      <form onSubmit={submit}>
        <DialogHeader title="Utiliser un jeton personnel" icon="key" />
        <DialogBody>
          <p className="text-body-sm leading-[1.6] text-tx2">
            Créez un jeton classique avec les portées <span className="mono text-tx">repo</span>,{" "}
            <span className="mono text-tx">read:org</span> et{" "}
            <span className="mono text-tx">workflow</span>, puis collez-le ici. Cadence le vérifie
            auprès de GitHub et le range dans le trousseau du système.
          </p>
          <div>
            <Button variant="ghost" icon="github" onClick={() => openExternal(NEW_TOKEN_URL)}>
              Créer le jeton sur GitHub
            </Button>
          </div>
          <Field label="Jeton" htmlFor={id} error={save.error?.message}>
            <Input
              id={id}
              type="password"
              mono
              autoFocus
              autoComplete="off"
              spellCheck={false}
              placeholder="ghp_…"
              value={token}
              invalid={save.isError}
              onChange={(e) => setToken(e.target.value)}
            />
          </Field>
        </DialogBody>
        <DialogFooter>
          <span className="text-caption text-tx3">Jamais écrit dans l'app ni dans le dépôt.</span>
          <div className="grow" />
          <Button variant="ghost" onClick={() => close(false)}>
            Annuler
          </Button>
          <Button
            type="submit"
            variant="primary"
            icon={save.isPending ? undefined : "check"}
            disabled={!token.trim() || save.isPending}
          >
            {save.isPending && <Spinner className="text-clay-ink" />}
            Valider le jeton
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
