import {
  Dialog,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogClose,
} from "@/components/ui/Dialog";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/layout/EmptyState";
import { useUiStore, type GlobalDialog } from "@/stores/ui";

const dialogs: Record<
  Exclude<GlobalDialog, null>,
  { title: string; icon: "issue" | "tag"; width: number; mockup: string }
> = {
  "issue-create": {
    title: "Nouvelle issue",
    icon: "issue",
    width: 664,
    mockup: "Issue-Create.dc.html",
  },
  "version-close": {
    title: "Clôturer la version",
    icon: "tag",
    width: 620,
    mockup: "Version-Bump.dc.html",
  },
};

/**
 * Modales globales, ouvertes depuis n'importe quel écran ou la palette (`openDialog`).
 * Chaque modale sera remplacée par son composant réel lors de son implémentation.
 */
export function DialogHost() {
  const dialog = useUiStore((s) => s.dialog);
  const close = useUiStore((s) => s.closeDialog);
  if (!dialog) return null;
  const def = dialogs[dialog];

  return (
    <Dialog open onOpenChange={(open) => !open && close()} width={def.width}>
      <DialogHeader title={def.title} icon={def.icon} />
      <DialogBody>
        <EmptyState title="Modale à implémenter">
          Maquette de référence : <span className="mono text-tx2">{def.mockup}</span>
        </EmptyState>
      </DialogBody>
      <DialogFooter>
        <div className="grow" />
        <DialogClose asChild>
          <Button variant="ghost">Fermer</Button>
        </DialogClose>
      </DialogFooter>
    </Dialog>
  );
}
