/**
 * Sélecteur du mode d'une conversation, dans le composeur du chat (comme les modes de Claude
 * Code) : Demander, Édition auto, Auto. Choisi pendant un tour, il vaut tout de suite pour les
 * demandes d'accord (« Auto » accepte celles en attente et les suivantes) ; la CLI le reçoit au
 * tour suivant.
 */
import { Icon } from "@/components/icons/Icon";
import {
  Menu,
  MenuContent,
  MenuLabel,
  MenuRadioGroup,
  MenuRadioItem,
  MenuTrigger,
} from "@/components/ui/Menu";
import { cn } from "@/lib/cn";
import { chatModeOrder, chatModes } from "@/lib/domain/chatModes";
import type { ChatPermissions } from "@/lib/types";

export interface ChatModeMenuProps {
  value: ChatPermissions;
  onChange: (mode: ChatPermissions) => void;
  /** Un tour est en cours : le nouveau mode vaut pour ses demandes d'accord (dit dans le menu). */
  running?: boolean;
  disabled?: boolean;
}

export function ChatModeMenu({ value, onChange, running, disabled }: ChatModeMenuProps) {
  const current = chatModes[value];
  return (
    <Menu>
      <MenuTrigger asChild disabled={disabled}>
        <button
          type="button"
          aria-label={`Mode de la conversation : ${current.label}`}
          title={current.description}
          className="inline-flex h-[30px] shrink-0 items-center gap-1.5 rounded-control border border-line pr-[7px] pl-2 text-body-sm text-tx2 hover:border-line2 hover:text-tx disabled:opacity-50"
        >
          <span className={cn("flex", current.tone)}>
            <Icon name={current.icon} size={13} />
          </span>
          {current.label}
          <Icon name="chevronDown" size={13} className="text-tx3" />
        </button>
      </MenuTrigger>
      <MenuContent className="w-[300px]">
        <MenuLabel>
          {running ? "Mode · vaut aussi pour les demandes en cours" : "Mode de la conversation"}
        </MenuLabel>
        <MenuRadioGroup value={value} onValueChange={(mode) => onChange(mode as ChatPermissions)}>
          {chatModeOrder.map((mode) => {
            const info = chatModes[mode];
            return (
              <MenuRadioItem key={mode} value={mode} className="h-auto items-start py-1.5">
                <span className="min-w-0">
                  <span className="flex items-center gap-1.5 text-body-sm text-tx">
                    <span className={cn("flex", info.tone)}>
                      <Icon name={info.icon} size={13} />
                    </span>
                    {info.label}
                  </span>
                  <span className="mt-0.5 block text-caption leading-[1.45] text-tx3">
                    {info.description}
                  </span>
                </span>
              </MenuRadioItem>
            );
          })}
        </MenuRadioGroup>
      </MenuContent>
    </Menu>
  );
}
