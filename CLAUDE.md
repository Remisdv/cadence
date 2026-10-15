# Cadence — conventions

App de bureau Tauri 2 (Rust) + React 19 / TS / Tailwind v4. Tout le texte d'interface est en français.
Voir README.md pour l'arborescence et les recettes.

## Sources de vérité

- Maquettes : canvas claude.ai « Cadence — Écrans de l'app » (artifact 4fX3USQ4tZEKxFxwaWoDpj). Chaque
  vue de `src/views/registry.tsx` indique le fichier `.dc.html` correspondant. Reproduire les valeurs
  exactes (px, couleurs) des styles inline des maquettes.
- Plan technique : doc claude.ai « Cadence — Plan technique d'implémentation » (artifact 66VRzBWtiCnCeikwZhb44c).

## Règles

- Couleurs, tailles de texte, rayons : uniquement les tokens de `src/styles/globals.css`
  (`bg-panel`, `text-tx2`, `text-caption`, `rounded-card`…). Pas de hex en dur dans les composants.
- Icônes : `<Icon name="…" />` (`src/components/icons/Icon.tsx`), jamais une autre bibliothèque.
- Avant d'écrire un composant, chercher dans `components/ui` et `components/domain` ; la galerie
  (`?view=design-system`) les montre tous. Un nouveau composant réutilisable y est ajouté.
- Vocabulaire métier → libellé/couleur : `src/lib/domain/*` (agents, types d'issue, états, versions).
- Données : hooks de `src/lib/query/queries.ts`, jamais `call()` dans une vue.
- Types partagés : importer depuis `@/lib/types`. `src/lib/bindings/` est généré (`pnpm bindings`).
- Toute nouvelle commande Rust : contrat TS (`lib/ipc/commands.ts`) + handler mock + hook.
- Le webview ne voit jamais de jeton ; aucun appel réseau direct depuis le front.
- Pas d'entier 64 bits dans les types IPC (ts-rs → `number`, numéros en `u32`).

## Vérifier

`pnpm typecheck && pnpm test && pnpm test:rust`, puis `pnpm dev` pour l'UI (données mockées).
