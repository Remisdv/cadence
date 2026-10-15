# Cadence

Cockpit de projet logiciel adossé à GitHub : un sprint est une version, une version est un milestone,
et les agents proposent pendant que vous disposez. Application de bureau Tauri 2.

- Maquettes : canvas « Cadence — Écrans de l'app » (19 écrans)
- Plan technique : doc « Cadence — Plan technique d'implémentation »

## Démarrer

Prérequis : Node 20+, pnpm, Rust stable (MSVC sous Windows), WebView2.

```bash
pnpm install
pnpm dev          # UI seule dans le navigateur, données mockées (http://localhost:1420)
pnpm tauri dev    # app de bureau, back Rust réel
pnpm tauri:mock   # app de bureau, back mocké (tant que les commandes Rust ne sont pas branchées)
```

| Commande | Rôle |
| --- | --- |
| `pnpm typecheck` | TypeScript strict |
| `pnpm test` | tests front (Vitest) |
| `pnpm test:rust` | tests Rust + régénération des types TS |
| `pnpm bindings` | régénère seulement `src/lib/bindings` depuis Rust (ts-rs) |
| `pnpm format` | Prettier + tri des classes Tailwind |
| `pnpm build` | build de prod du front |

### Connexion GitHub

Le device flow a besoin d'une **OAuth App** GitHub (seul son client ID, public, est embarqué) :

1. github.com › Settings › Developer settings › OAuth Apps › New OAuth App
   (URL de callback quelconque, elle n'est pas utilisée) ;
2. cocher **Enable Device Flow** ;
3. exporter le client ID avant `pnpm tauri dev` (lu à l'exécution, ou figé au build) :

```bash
export CADENCE_GITHUB_CLIENT_ID=Ov23li...
```

Sans client ID, l'écran propose le repli par jeton personnel (portées `repo`, `read:org`, `workflow`).
Le jeton est rangé dans le trousseau de l'OS (Keychain, Credential Manager, Secret Service).

### Mode mock

En mode navigateur, l'URL pilote le scénario mocké :

- `?scenario=onboarding` : compte GitHub non connecté ; `?scenario=no-project` : aucun projet choisi ;
- `&flow=denied|expired|config` : issue du device flow simulé (par défaut, autorisé après 6 s) ;
- `&cli=fail` : session de test de Claude Code en échec ;
- `?view=<id>` force un écran (`?view=design-system`, `?view=issue&number=156`, `?view=agent&kind=dev`).

La **galerie de composants** (`?view=design-system`, ou `Ctrl+K` → « Galerie de composants ») montre
toutes les briques du design system avec les données des maquettes.

## Architecture

```
cadence/
├─ src/                        # front React 19 + TypeScript
│  ├─ app/                     # racine : providers, démarrage, cadre des vues, modales globales
│  ├─ views/                   # un écran du canvas = une vue ; registry.tsx les déclare toutes
│  ├─ components/
│  │  ├─ icons/                # les 34 icônes des maquettes (Icon name="…")
│  │  ├─ ui/                   # primitives du design system (Button, Badge, Field, Dialog…)
│  │  ├─ domain/               # briques métier (IssueCard, AgentBadge, StepTimeline, DiffViewer…)
│  │  └─ layout/               # Sidebar, Page/TopBar, Rail, OnboardingLayout, CommandPalette
│  ├─ stores/                  # Zustand : navigation (vue courante), état d'interface
│  ├─ lib/
│  │  ├─ bindings/             # GÉNÉRÉ par ts-rs — ne pas éditer
│  │  ├─ types.ts              # point d'import unique des types partagés
│  │  ├─ ipc/                  # contrat des commandes, transport Tauri/mock, événements
│  │  ├─ query/                # TanStack Query : client, clés, hooks, pont événements → cache
│  │  └─ domain/               # vocabulaire métier → libellés/couleurs, formats FR, semver
│  └─ styles/globals.css       # tokens du design system (Tailwind v4 @theme)
└─ src-tauri/                  # cœur Rust
   ├─ src/
   │  ├─ commands/             # surface IPC, une commande par intention métier
   │  ├─ domain/               # types du contrat (exportés en TS)
   │  ├─ api/github/           # client GitHub, régimes de quota
   │  ├─ auth/                 # device flow, trousseau, détection des CLI d'agents
   │  ├─ agents/               # runner, cycle de vie, garde-fous
   │  ├─ store/  sync/  vcs/   # cache SQLite, moteur de sync, git/worktrees
   │  ├─ events.rs             # événements back → front
   │  └─ error.rs              # AppError → { kind, message }
   └─ migrations/              # schéma SQLite du cache
```

Principes (du plan technique) :

- Le webview n'a ni jeton ni accès à l'API GitHub : tout passe par une commande Rust.
- Une commande par intention métier ; le front ne compose jamais plusieurs appels pour une vue.
- Les vues lisent le cache via TanStack Query et se rafraîchissent sur `sync://updated`.
- Navigation par état (`useNavigation`), sans routeur d'URL.
- Les types du contrat sont écrits une fois en Rust et générés en TS : un changement casse à la compilation.

## Recettes

**Implémenter un écran** — créer `src/views/<zone>/<Nom>View.tsx` (export par défaut, prop `view`
typée), puis remplacer son `placeholder(...)` dans `src/views/registry.tsx` par
`lazy(() => import("./<zone>/<Nom>View"))`. Composer avec `Page` (en-tête + zone principale),
`components/domain` et `components/ui` ; lire les données avec les hooks de `lib/query/queries.ts`.

**Ajouter une commande** — 1. l'écrire dans `src-tauri/src/commands/<domaine>.rs` avec ses types dans
`domain/` (`#[ts(export)]`) ; 2. l'enregistrer dans `lib.rs` ; 3. `pnpm bindings` ; 4. la déclarer dans
`src/lib/ipc/commands.ts` et lui donner un handler dans `src/lib/ipc/mock/handlers.ts` (le
typage refuse un oubli) ; 5. exposer un hook dans `lib/query/queries.ts`.

**Ajouter une couleur ou une taille** — dans `src/styles/globals.css` (`@theme`) ; une nouvelle taille de
texte ou un nouveau rayon se déclare aussi dans `src/lib/cn.ts` pour que `cn()` fusionne correctement.

## État

- **Fondations** : design system complet, architecture front/back, contrat IPC typé (29 commandes,
  toutes mockées), navigation et palette de commandes.
- **Onboarding 1 — Connexion GitHub** : branché de bout en bout (device flow, polling, trousseau,
  repli par jeton personnel, changement de compte).
- **Onboarding 2 — Agents de code** : détection de Claude Code et Codex CLI (réglage manuel, `PATH`,
  emplacements usuels), session de test headless de Claude Code, localisation manuelle du binaire,
  désactivation, affectation des rôles. Les préférences de la machine sont dans
  `<config de l'app>/settings.json`.
- **Onboarding 3 — Projet et mapping** : liste des dépôts accessibles (une requête GraphQL, filtrée
  localement), aperçu des milestones importés comme versions et du sprint en cours, ouverture du
  projet. Le choix est mémorisé et le projet rouvert au lancement suivant. `versions_list` est branché.
- **À venir** : les écrans de l'app (Sprint, Issues…), aujourd'hui des placeholders qui pointent
  vers leur maquette, et le cache SQLite.
