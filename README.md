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

**Aucune configuration n'est nécessaire** sur une machine où la CLI GitHub ou Git sont installés.
Méthodes, dans l'ordre proposé à l'écran :

| Méthode | Comment | Besoin |
| --- | --- | --- |
| Compte de la CLI GitHub | reprend la session de `gh` en un clic (`gh auth token`) | `gh` connecté |
| Navigateur | Git Credential Manager ouvre GitHub dans le navigateur et rend la main tout seul (il a sa propre application OAuth) | Git pour Windows / GCM |
| Navigateur, OAuth App de Cadence | flux web direct : redirection vers `127.0.0.1`, PKCE | client ID + client secret |
| Code d'appairage | device flow : un code à saisir sur github.com/login/device | client ID, device flow activé |
| Jeton personnel | jeton classique collé dans l'app | rien |

Dans tous les cas, le jeton est validé auprès de GitHub puis copié dans le trousseau de l'OS sous
l'entrée de Cadence ; il n'est jamais affiché ni renvoyé au webview. Seule la portée `repo` est
bloquante : l'écran signale les autres portées manquantes.

#### Optionnel : OAuth App propre à Cadence

Utile pour une distribution publique (l'écran d'autorisation GitHub affiche alors « Cadence »).
Créer l'**OAuth App** : github.com › Settings › Developer settings › OAuth Apps › New OAuth App.

1. **Authorization callback URL** : `http://127.0.0.1/callback` (GitHub accepte tout port sur cette
   adresse loopback ; Cadence en choisit un libre à chaque connexion) ;
2. cocher **Enable Device Flow** (pour le code d'appairage) ;
3. générer un client secret ;
4. exporter les deux valeurs avant `pnpm tauri dev` (lues à l'exécution, ou figées au build) :

```powershell
$env:CADENCE_GITHUB_CLIENT_ID="Ov23li..."
$env:CADENCE_GITHUB_CLIENT_SECRET="..."
```

Le client secret est compilé dans l'app : dans une app native il ne peut pas rester confidentiel
(RFC 8252, même choix que la CLI `gh`). PKCE et le paramètre `state` empêchent qu'un code
d'autorisation intercepté soit utilisable. Le jeton obtenu est rangé dans le trousseau de l'OS
(Keychain, Credential Manager, Secret Service), jamais dans le webview ni sur disque.

### Mode mock

En mode navigateur, l'URL pilote le scénario mocké :

- `?scenario=onboarding` : compte GitHub non connecté ; `?scenario=no-project` : aucun dépôt choisi (sélecteur ouvert) ;
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

- **Fondations** : design system complet, architecture front/back, contrat IPC typé (32 commandes,
  toutes mockées), navigation et palette de commandes.
- **Onboarding 1 — Connexion GitHub** : branché de bout en bout (compte de la CLI GitHub, navigateur via Git Credential Manager ou OAuth App, code d'appairage, trousseau,
  jeton personnel, changement de compte).
- **Onboarding 2 — Agents de code** : détection de Claude Code et Codex CLI (réglage manuel, `PATH`,
  emplacements usuels), session de test headless de Claude Code, localisation manuelle du binaire,
  désactivation, affectation des rôles. Les préférences de la machine sont dans
  `<config de l'app>/settings.json`.
- **Sélecteur de dépôt** (haut de la barre latérale, ou `Ctrl+K` → « Changer de dépôt ») : Cadence
  fonctionne sur n'importe quel dépôt accessible au compte (une requête GraphQL, filtrée localement),
  et permet d'en créer un (compte personnel ou organisation, privé ou public, README initial).
  Ouvrir un dépôt importe ses milestones comme versions ; le choix est mémorisé et rouvert au
  lancement suivant. Sans dépôt mémorisé, le sélecteur s'ouvre de lui-même. `versions_list` est branché.
- **Issues** : toutes les issues du dépôt ouvert (pull requests exclues), groupées par version avec
  leur avancement ; filtres texte / version / état / assigné, sélection multiple. Lecture GitHub
  paginée avec ETag, cache mémoire rafraîchi au plus toutes les 60 s (un `304` ne consomme pas de
  quota), servi hors ligne. Les actions groupées et l'agent CTO arrivent avec l'écriture (0.1.2+).
- **À venir** : le cache SQLite (remplacera le cache mémoire), puis les autres écrans (Sprint,
  détail d'issue…), aujourd'hui des placeholders qui pointent vers leur maquette.
