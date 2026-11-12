import type { Repo } from "@/lib/types";

/**
 * Service Dépôts d'un plugin : un seul dépôt actif à la fois (tout ce que le plugin gère vit
 * dans ce dépôt). La liste « suivie » ne sert plus qu'à retenir les dépôts récemment ouverts,
 * que l'IA peut encore toucher (tâches en fond lancées avant un changement de dépôt).
 */

/** Dépôts récents retenus par plugin, actif compris. */
export const RECENT_REPOS = 8;

/** Liste suivie après activation de `repo` : en tête, sans doublon, bornée à `RECENT_REPOS`. */
export function activateRepo(tracked: string[], repo: string): string[] {
  const rest = tracked.filter((r) => r.toLowerCase() !== repo.toLowerCase());
  return [repo, ...rest].slice(0, RECENT_REPOS);
}

/** Dépôt actif : la sélection enregistrée, sinon le premier dépôt suivi. */
export function activeRepo(tracked: string[], selection: string | null | undefined): string | null {
  if (selection && tracked.some((r) => r.toLowerCase() === selection.toLowerCase())) return selection;
  return tracked[0] ?? null;
}

/** Ligne du sélecteur de dépôt : un récent du plugin ou un dépôt du compte. */
export interface RepoRow {
  fullName: string;
  description: string | null;
  private: boolean;
  recent: boolean;
}

/** Récents (filtrés par la recherche) puis les autres dépôts du compte, sans doublon. */
export function mergeRepoRows(recents: string[], account: Repo[], query: string): RepoRow[] {
  const q = query.trim().toLowerCase();
  const byName = new Map(account.map((r) => [r.fullName.toLowerCase(), r]));
  const recentRows: RepoRow[] = recents
    .filter((name) => !q || name.toLowerCase().includes(q) || byName.has(name.toLowerCase()))
    .map((name) => {
      const repo = byName.get(name.toLowerCase());
      return {
        fullName: name,
        description: repo?.description ?? null,
        private: repo?.private ?? false,
        recent: true,
      };
    });
  const seen = new Set(recents.map((r) => r.toLowerCase()));
  const others: RepoRow[] = account
    .filter((r) => !seen.has(r.fullName.toLowerCase()))
    .map((r) => ({ fullName: r.fullName, description: r.description, private: r.private, recent: false }));
  return [...recentRows, ...others];
}
