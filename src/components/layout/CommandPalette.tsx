import { useEffect } from "react";
import { Command } from "cmdk";
import { Dialog as DialogPrimitive } from "radix-ui";
import { Icon, type IconName } from "@/components/icons/Icon";
import { Kbd } from "@/components/ui/Kbd";
import { cn } from "@/lib/cn";
import { hasModKey } from "@/lib/hooks/usePlatform";
import { navigate, type View } from "@/stores/navigation";
import { useUiStore, type GlobalDialog } from "@/stores/ui";

interface PaletteCommand {
  id: string;
  label: string;
  icon: IconName;
  group: string;
  run: () => void;
  shortcut?: string;
}

function buildCommands(
  openDialog: (d: Exclude<GlobalDialog, null>) => void,
  openPicker: () => void,
): PaletteCommand[] {
  const go = (view: View) => () => navigate(view);
  const commands: PaletteCommand[] = [
    {
      id: "switch-repo",
      group: "Actions",
      label: "Changer de dépôt…",
      icon: "folder",
      run: openPicker,
    },
    {
      id: "new-issue",
      group: "Actions",
      label: "Nouvelle issue",
      icon: "plus",
      run: () => openDialog("issue-create"),
    },
    {
      id: "close-version",
      group: "Actions",
      label: "Clôturer la version en cours",
      icon: "tag",
      run: () => openDialog("version-close"),
    },
    {
      id: "go-sprint",
      group: "Aller à",
      label: "Sprint en cours",
      icon: "sprint",
      run: go({ id: "sprint" }),
    },
    {
      id: "go-roadmap",
      group: "Aller à",
      label: "Roadmap",
      icon: "roadmap",
      run: go({ id: "roadmap" }),
    },
    {
      id: "go-versions",
      group: "Aller à",
      label: "Versions",
      icon: "tag",
      run: go({ id: "version" }),
    },
    {
      id: "go-issues",
      group: "Aller à",
      label: "Issues",
      icon: "issue",
      run: go({ id: "issues" }),
    },
    {
      id: "go-prs",
      group: "Aller à",
      label: "Pull requests",
      icon: "pullRequest",
      run: go({ id: "pull-requests" }),
    },
    {
      id: "go-reviews",
      group: "Aller à",
      label: "Revues à faire",
      icon: "review",
      run: go({ id: "reviews" }),
    },
    {
      id: "go-settings",
      group: "Aller à",
      label: "Réglages",
      icon: "settings",
      run: go({ id: "settings" }),
    },
  ];
  if (import.meta.env.DEV) {
    commands.push(
      {
        id: "dev-gallery",
        group: "Développement",
        label: "Galerie de composants",
        icon: "layers",
        run: go({ id: "design-system" }),
      },
      {
        id: "dev-onboarding",
        group: "Développement",
        label: "Revoir l'onboarding",
        icon: "github",
        run: go({ id: "onboarding-github" }),
      },
    );
  }
  return commands;
}

/** Palette ⌘K / Ctrl+K : navigation et actions, au clavier. */
export function CommandPalette() {
  const open = useUiStore((s) => s.paletteOpen);
  const setOpen = useUiStore((s) => s.setPaletteOpen);
  const openDialog = useUiStore((s) => s.openDialog);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (hasModKey(e) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useUiStore.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  const setPickerOpen = useUiStore((s) => s.setProjectPickerOpen);
  // Laisse la palette se fermer avant d'ouvrir le sélecteur (focus).
  const commands = buildCommands(openDialog, () => setTimeout(() => setPickerOpen(true), 0));
  const groups = [...new Set(commands.map((c) => c.group))];

  const run = (command: PaletteCommand) => {
    setOpen(false);
    command.run();
  };

  return (
    <DialogPrimitive.Root open={open} onOpenChange={setOpen}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-backdrop/70" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed top-[60px] left-1/2 z-50 w-[600px] max-w-[calc(100vw-32px)] -translate-x-1/2 overflow-hidden rounded-modal border border-line2 bg-panel shadow-modal outline-none"
        >
          <DialogPrimitive.Title className="sr-only">Palette de commandes</DialogPrimitive.Title>
          <Command loop label="Palette de commandes">
            <div className="flex h-[50px] items-center gap-2.5 border-b border-line px-[15px]">
              <span className="flex text-tx3">
                <Icon name="search" size={17} />
              </span>
              <Command.Input
                autoFocus
                placeholder="Rechercher une action ou un écran…"
                className="grow bg-transparent text-[14.5px] text-tx outline-none placeholder:text-tx3"
              />
              <Kbd boxed>esc</Kbd>
            </div>
            <Command.List className="max-h-[360px] overflow-y-auto py-2">
              <Command.Empty className="px-3.5 py-6 text-center text-body-sm text-tx3">
                Aucune commande ne correspond.
              </Command.Empty>
              {groups.map((group, i) => (
                <Command.Group
                  key={group}
                  heading={group}
                  className={cn(
                    "[&_[cmdk-group-heading]]:px-3.5 [&_[cmdk-group-heading]]:pt-1.5 [&_[cmdk-group-heading]]:pb-1",
                    "[&_[cmdk-group-heading]]:text-micro [&_[cmdk-group-heading]]:font-semibold [&_[cmdk-group-heading]]:tracking-[0.09em] [&_[cmdk-group-heading]]:text-tx3 [&_[cmdk-group-heading]]:uppercase",
                    i > 0 && "mt-1.5 border-t border-line pt-1",
                  )}
                >
                  {commands
                    .filter((c) => c.group === group)
                    .map((c) => (
                      <Command.Item
                        key={c.id}
                        value={`${c.group} ${c.label}`}
                        onSelect={() => run(c)}
                        className="group flex h-[38px] cursor-pointer items-center gap-[11px] px-3.5 text-body text-tx2 data-[selected=true]:bg-p3 data-[selected=true]:text-tx"
                      >
                        <span className="flex text-tx3 group-data-[selected=true]:text-clay">
                          <Icon name={c.icon} size={16} />
                        </span>
                        <span className="grow">{c.label}</span>
                        {c.shortcut && <Kbd>{c.shortcut}</Kbd>}
                      </Command.Item>
                    ))}
                </Command.Group>
              ))}
            </Command.List>
          </Command>
          <div className="flex h-[34px] items-center gap-4 border-t border-line bg-p2 px-[15px]">
            <span className="mono text-micro text-tx3">↑↓ naviguer</span>
            <span className="mono text-micro text-tx3">↵ exécuter</span>
            <span className="mono text-micro text-tx3">esc fermer</span>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
