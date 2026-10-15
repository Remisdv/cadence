# Premiers pas

Au premier lancement, Cadence vous guide en trois temps : connecter GitHub, détecter les agents
de code de la machine, choisir un dépôt.

## 1. Connexion à GitHub

**Aucune configuration n'est nécessaire** sur une machine où la CLI GitHub ou Git sont installés.
L'écran propose les méthodes dans cet ordre :

| Méthode | Comment ça marche | Ce qu'il faut |
| --- | --- | --- |
| **Compte de la CLI GitHub** | reprend la session de `gh` en un clic (`gh auth token`) | `gh` installé et connecté (`gh auth login`) |
| **Navigateur** | Git Credential Manager ouvre GitHub dans votre navigateur et rend la main tout seul | Git pour Windows, ou GCM installé |
| **Navigateur, OAuth App de Cadence** | flux web direct avec retour sur `127.0.0.1` (PKCE) | une build de Cadence fournie avec son OAuth App |
| **Code d'appairage** | un code à saisir sur [github.com/login/device](https://github.com/login/device) | une build fournie avec son OAuth App (device flow) |
| **Jeton personnel** | vous collez un jeton classique dans l'app | rien |

Quelle que soit la méthode, le jeton est **validé auprès de GitHub**, puis rangé dans le trousseau
du système (Keychain sur macOS, Gestionnaire d'identifiants sous Windows, Secret Service sous
Linux). Il n'est jamais affiché, jamais écrit dans un fichier et jamais transmis à l'interface :
toutes les requêtes GitHub partent du cœur Rust de l'app.

### Portées du jeton

Seule la portée **`repo`** est indispensable (lire et écrire issues, milestones, tags, pull
requests). L'écran signale les autres portées manquantes sans bloquer :

- `read:org` — lister les dépôts de vos organisations et y créer un dépôt ;
- `workflow` — permettre à l'agent développeur de pousser une modification de
  `.github/workflows/` ;
- `read:user` (facultative) — nom affiché et avatar.

Pour un **jeton personnel**, créez un jeton classique sur github.com › Settings › Developer
settings › Personal access tokens › Tokens (classic), avec au moins `repo`. Les jetons
« fine-grained » fonctionnent aussi s'ils donnent accès en lecture/écriture aux *Issues*,
*Pull requests*, *Contents* et *Metadata* des dépôts concernés.

### Changer de compte

Réglages › **Comptes et jetons** › **Se déconnecter** retire le jeton du trousseau et efface le
cache local de tous les dépôts. L'écran de connexion réapparaît.

## 2. Agents de code

Cadence ne fournit pas de modèle : ses agents pilotent **Claude Code** (ou **Codex CLI**)
installé sur votre machine, avec la session que vous y avez déjà ouverte.

1. Cadence cherche le binaire : réglage manuel, puis `PATH`, puis les emplacements usuels.
   **Localiser…** permet de l'indiquer à la main.
2. **Tester** lance une courte session headless (`claude -p`) pour vérifier que la CLI répond et
   qu'elle est connectée. En cas d'échec, ouvrez un terminal, lancez `claude` puis `/login`.
3. **Affectation des rôles** : choisissez le moteur et le modèle de chaque agent (par défaut,
   Claude Code `opus` pour le CTO et le développeur, `sonnet` pour le CEO et le déploiement).
   Ce choix est une préférence de la machine ; il sert de valeur par défaut aux projets qui n'ont
   pas encore de `.cadence/config.toml`.

Vous pouvez désactiver un outil que vous n'utilisez pas. Tout se retrouve ensuite dans
Réglages › **Machine locale**.

## 3. Choix du dépôt

Le **sélecteur de dépôt** (en haut de la barre latérale, ou `Ctrl+K` / `⌘K` › « Changer de
dépôt ») liste tous les dépôts accessibles à votre compte, personnels et d'organisation, avec une
recherche locale. Vous pouvez aussi **créer un dépôt** (compte personnel ou organisation, privé ou
public, avec un README initial).

Ouvrir un dépôt :

- importe ses **milestones** comme versions et ses issues dans le cache local ;
- lit son `.cadence/config.toml` s'il existe (sinon, les valeurs par défaut s'appliquent, voir
  [la référence](configuration.md)) ;
- est mémorisé : Cadence rouvre ce dépôt au lancement suivant.

Un dépôt sans milestone s'ouvre normalement ; créez la première version depuis GitHub (un
milestone nommé `0.1.0`, par exemple) ou par la clôture de version de Cadence. Voir
[Issues, versions et roadmap](projet.md).

## Palette de commandes

`Ctrl+K` (`⌘K` sur macOS) ouvre la palette : changer de dépôt, créer une issue, aller à un
écran, clore la version… C'est le moyen le plus rapide de naviguer dans Cadence.
