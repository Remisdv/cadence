/**
 * Contexte d'une conversation, dans le composeur du chat (comme le choix du projet et des
 * dossiers dans Claude Code) :
 *
 * - `ChatPluginMenu` : le plugin de la conversation (« Global » ou un plugin activé). Elle est
 *   rangée dans sa catégorie de la colonne des conversations ; fixé une fois la conversation
 *   ouverte.
 * - `ChatReposMenu` : les dépôts GitHub suivis ouverts à l'IA (plusieurs possibles : un POC et
 *   le dépôt de production, par exemple). Chacun reçoit un worktree propre à la conversation.
 */
import { Icon } from "@/components/icons/Icon";
import { NeutralTile, PluginTile } from "@/components/domain/PluginTile";
import {
  Menu,
  MenuCheckboxItem,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/Menu";
import { reposLabel } from "@/lib/domain/chat";
import type { InstalledPluginView } from "@/lib/types";

const chipClass =
  "inline-flex h-[30px] max-w-[260px] shrink-0 items-center gap-1.5 rounded-control border border-line pr-[7px] pl-1.5 text-body-sm text-tx2 hover:border-line2 hover:text-tx disabled:opacity-50 disabled:hover:border-line";

export interface ChatPluginMenuProps {
  /** Plugin choisi ; `null` = Global. */
  value: string | null;
  plugins: InstalledPluginView[];
  onChange: (plugin: string | null) => void;
  /** Conversation déjà ouverte : le plugin est fixé (puce sans menu). */
  locked?: boolean;
}

export function ChatPluginMenu({ value, plugins, onChange, locked }: ChatPluginMenuProps) {
  const current = plugins.find((p) => p.id === value);
  const chip = (
    <>
      {current ? (
        <PluginTile color={current.color} icon={current.railIcon} size={18} />
      ) : (
        <NeutralTile icon="chat" size={18} />
      )}
      <span className="truncate">{current?.name ?? (value ? value : "Global")}</span>
    </>
  );
  if (locked || plugins.length === 0) {
    return (
      <span
        className={chipClass.replace(/hover:\S+/g, "")}
        title="Catégorie de la conversation, fixée à sa création"
      >
        {chip}
      </span>
    );
  }
  return (
    <Menu>
      <MenuTrigger asChild>
        <button
          type="button"
          aria-label={`Plugin de la conversation : ${current?.name ?? "Global"}`}
          title="Plugin de la conversation : elle est rangée dans sa catégorie"
          className={chipClass}
        >
          {chip}
          <Icon name="chevronDown" size={13} className="shrink-0 text-tx3" />
        </button>
      </MenuTrigger>
      <MenuContent className="w-[280px]">
        <MenuLabel>Ranger la conversation dans</MenuLabel>
        <MenuItem onSelect={() => onChange(null)}>
          <NeutralTile icon="chat" size={18} />
          <span className="min-w-0 grow truncate">Global · tous les plugins</span>
          {value === null && <Icon name="check" size={13} className="text-clay" />}
        </MenuItem>
        <MenuSeparator />
        {plugins.map((plugin) => (
          <MenuItem key={plugin.id} onSelect={() => onChange(plugin.id)}>
            <PluginTile color={plugin.color} icon={plugin.railIcon} size={18} />
            <span className="min-w-0 grow truncate">{plugin.name}</span>
            {plugin.id === value && <Icon name="check" size={13} className="text-clay" />}
          </MenuItem>
        ))}
      </MenuContent>
    </Menu>
  );
}

export interface ChatReposMenuProps {
  /** Dépôts choisis (`owner/nom`). */
  value: string[];
  /** Dépôts suivis par le plugin de la conversation, puis par les autres plugins. */
  own: string[];
  others: string[];
  /** Nom du plugin, pour titrer ses dépôts. */
  pluginName?: string;
  onChange: (repos: string[]) => void;
  /** « Gérer les dépôts… » : Réglages › Dépôts GitHub. */
  onManage: () => void;
  disabled?: boolean;
}

export function ChatReposMenu({
  value,
  own,
  others,
  pluginName,
  onChange,
  onManage,
  disabled,
}: ChatReposMenuProps) {
  const chosen = new Set(value.map((r) => r.toLowerCase()));
  const toggle = (repo: string, on: boolean) =>
    onChange(
      on ? [...value, repo] : value.filter((r) => r.toLowerCase() !== repo.toLowerCase()),
    );
  // Un dépôt choisi qui n'est plus suivi reste affiché, pour pouvoir le retirer.
  const known = new Set([...own, ...others].map((r) => r.toLowerCase()));
  const orphans = value.filter((r) => !known.has(r.toLowerCase()));
  const item = (repo: string) => (
    <MenuCheckboxItem
      key={repo}
      checked={chosen.has(repo.toLowerCase())}
      onCheckedChange={(on) => toggle(repo, on)}
    >
      <span className="min-w-0 grow truncate mono text-small">{repo}</span>
    </MenuCheckboxItem>
  );
  return (
    <Menu>
      <MenuTrigger asChild disabled={disabled}>
        <button
          type="button"
          aria-label={`Dépôts de la conversation : ${reposLabel(value)}`}
          title={
            value.length > 0
              ? `Ouverts à l'IA (un worktree par dépôt) : ${value.join(", ")}`
              : "Choisir les dépôts GitHub ouverts à l'IA"
          }
          className={chipClass}
        >
          <Icon
            name="branch"
            size={14}
            className={value.length > 0 ? "shrink-0 text-green" : "shrink-0 text-tx3"}
          />
          <span className={value.length > 0 ? "truncate mono text-small" : "truncate"}>
            {reposLabel(value)}
          </span>
          <Icon name="chevronDown" size={13} className="shrink-0 text-tx3" />
        </button>
      </MenuTrigger>
      <MenuContent className="max-h-[380px] w-[320px] overflow-y-auto">
        {own.length > 0 && (
          <>
            <MenuLabel>{pluginName ? `Dépôts de ${pluginName}` : "Dépôts du plugin"}</MenuLabel>
            {own.map(item)}
          </>
        )}
        {others.length > 0 && (
          <>
            {own.length > 0 && <MenuSeparator />}
            <MenuLabel>Autres dépôts suivis</MenuLabel>
            {others.map(item)}
          </>
        )}
        {orphans.length > 0 && (
          <>
            <MenuSeparator />
            <MenuLabel>Plus suivis</MenuLabel>
            {orphans.map(item)}
          </>
        )}
        {own.length + others.length + orphans.length === 0 && (
          <p className="px-2.5 py-2 text-caption leading-[1.55] text-tx3">
            Aucun dépôt suivi pour l'instant.
          </p>
        )}
        <MenuSeparator />
        <p className="px-2.5 pt-1 pb-1.5 text-caption leading-[1.5] text-tx3">
          Chaque dépôt coché reçoit un worktree propre à la conversation, ouvert à l'IA au prochain
          message.
        </p>
        <MenuItem onSelect={onManage}>
          <Icon name="settings" size={14} />
          Gérer les dépôts suivis…
        </MenuItem>
      </MenuContent>
    </Menu>
  );
}
