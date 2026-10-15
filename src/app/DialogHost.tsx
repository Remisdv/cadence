import { Dialog } from "@/components/ui/Dialog";
import { useUiStore } from "@/stores/ui";
import { IssueCreateDialog } from "@/views/dialogs/IssueCreateDialog";
import { VersionCloseDialog } from "@/views/dialogs/VersionCloseDialog";

/** Modales globales, ouvertes depuis n'importe quel écran ou la palette (`openDialog`). */
export function DialogHost() {
  const dialog = useUiStore((s) => s.dialog);
  const close = useUiStore((s) => s.closeDialog);
  if (!dialog) return null;

  return (
    <Dialog open onOpenChange={(open) => !open && close()} width={dialog === "issue-create" ? 664 : 620}>
      {dialog === "issue-create" ? <IssueCreateDialog onDone={close} /> : <VersionCloseDialog onDone={close} />}
    </Dialog>
  );
}
