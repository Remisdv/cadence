# FAQ et dépannage

## Agents

### « Claude Code non connecté » / la session de test échoue

Claude Code est installé mais n'a pas de session ouverte (ou elle a expiré). Ouvrez un terminal :

```bash
claude
```

puis tapez `/login` et suivez les instructions. Revenez dans Cadence et cliquez sur **Tester**
(onboarding, ou Réglages › Machine locale). Le message d'erreur renvoyé par la CLI est affiché
tel quel (par exemple « Invalid API key · Please run /login »).

### Claude Code est introuvable

Cadence cherche le binaire dans le `PATH`, puis dans les emplacements usuels (`~/.local/bin`,
`~/.npm-global/bin`, `/opt/homebrew/bin`, `%APPDATA%\npm`). Sur macOS, une app lancée depuis le
Finder n'hérite pas du `PATH` de votre shell. Vérifiez avec `which claude` (macOS, Linux) ou
`where claude` (Windows), puis indiquez ce chemin avec **Localiser…**.

### Un agent s'arrête au bout d'un moment

La session a atteint `max_session_minutes` (30 min par défaut). Augmentez la valeur pour cet
agent dans Réglages › Agents (jusqu'à 240), ou découpez l'issue.

### L'agent développeur n'a pas poussé

C'est voulu tant que vous n'avez pas validé l'étape 5 (autonomie `apply-after-review`), et
toujours en autonomie `propose`. Cadence refuse aussi de pousser sur la branche par défaut. Si le
push est refusé par GitHub pour un fichier `.github/workflows/`, ajoutez la portée `workflow` au
jeton.

### Les agents sont « suspendus »

Le quota GitHub est presque épuisé (moins de 200 requêtes) : les agents reprennent seuls à la
réinitialisation, en général dans l'heure. Voir [Hors ligne et quota](hors-ligne-et-quota.md).

## Connexion GitHub

### « GitHub refuse ce jeton »

Le jeton a expiré, a été révoqué, ou a été mal copié. Créez-en un nouveau (portée `repo` au
minimum) ou utilisez une autre méthode de connexion.

### Les dépôts de mon organisation n'apparaissent pas

Il manque la portée `read:org`, ou l'organisation restreint l'accès des applications OAuth.
Dans ce cas, un administrateur doit approuver l'application (github.com › Settings ›
Applications), ou utilisez un jeton personnel autorisé pour l'organisation (SSO).

### Linux : « trousseau indisponible »

Cadence range le jeton via Secret Service. Installez et démarrez GNOME Keyring (`gnome-keyring`)
ou KWallet, puis relancez Cadence.

### macOS redemande l'accès au trousseau à chaque lancement

Choisissez « Toujours autoriser » dans la fenêtre du trousseau. Après une mise à jour, macOS peut
le redemander une fois.

## Projet

### « .cadence/config.toml illisible » ou « valeur invalide »

Le fichier du dépôt contient une erreur de syntaxe TOML ou une valeur hors bornes. Le message
indique la clé en cause ; corrigez-la et poussez.

### L'enregistrement des réglages est refusé

Le fichier a été modifié sur GitHub depuis son ouverture dans Cadence. Rouvrez les réglages pour
recharger la dernière version, puis recommencez. Si la branche par défaut est protégée contre
les push directs, éditez le fichier par une pull request.

### Une modification faite sur github.com n'apparaît pas

La synchronisation passe au plus une fois par minute (toutes les 5 minutes en régime économe).
Patientez, ou changez d'écran pour relancer une lecture.

## Mises à jour

### « Vérification impossible » dans Réglages › À propos

Cadence n'a pas pu joindre les releases GitHub (hors ligne, proxy, pare-feu). Réessayez avec
**Rechercher une mise à jour**. Vous pouvez toujours télécharger le dernier installeur sur la
page des releases.

### « Installation impossible : signature invalide »

Le fichier téléchargé ne correspond pas à la signature publiée : il a été altéré, ou le
téléchargement est incomplet. Rien n'a été installé. Réessayez ; si l'erreur persiste,
téléchargez l'installeur depuis la page des releases et signalez le problème.

### Linux : la mise à jour automatique ne fait rien

Seule l'AppImage se met à jour seule. Pour un paquet `.deb` ou `.rpm`, installez le nouveau
paquet.

## Journaux

Lancez Cadence depuis un terminal avec `RUST_LOG=cadence_lib=debug` pour obtenir des journaux
détaillés (aucun jeton n'y figure). Joignez-les à un signalement de bug.
