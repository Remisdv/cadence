# Installation

Les installeurs sont publiés sur la page des
[releases GitHub de Cadence](https://github.com/cadence-app/desktop/releases). Choisissez le
fichier correspondant à votre système.

| Système | Fichier | Remarques |
| --- | --- | --- |
| macOS 10.15 ou plus récent (Apple Silicon et Intel) | `Cadence_<version>_universal.dmg` | binaire universel, signé et notarisé |
| Windows 10 / 11 (x64) | `Cadence_<version>_x64-setup.exe` (recommandé) ou `Cadence_<version>_x64_en-US.msi` | installeur signé |
| Linux x64 | `cadence_<version>_amd64.AppImage`, `.deb` ou `.rpm` | AppImage : mise à jour automatique |

## Avant d'installer

Cadence s'appuie sur des outils déjà présents sur votre machine :

- **Git** — pour les copies locales des dépôts et les worktrees de l'agent développeur.
  macOS : `xcode-select --install` ; Windows : [Git pour Windows](https://git-scm.com/download/win)
  (fournit aussi Git Credential Manager, utile à la connexion GitHub) ; Linux : paquet `git`.
- **Claude Code** — le moteur des agents. Installez-le puis connectez-le une fois dans un
  terminal : `claude`, puis `/login`. Codex CLI est pris en charge comme moteur alternatif.
- **Facultatif : la CLI GitHub (`gh`)** connectée (`gh auth login`) : Cadence peut reprendre sa
  session en un clic.

## macOS

1. Ouvrez le `.dmg` et glissez **Cadence** dans **Applications**.
2. Lancez Cadence depuis le Launchpad ou le Finder. L'app est notarisée par Apple : Gatekeeper
   l'ouvre sans avertissement.
3. Au premier accès au trousseau, macOS demande d'autoriser Cadence : choisissez « Toujours
   autoriser » pour ne pas être sollicité à chaque lancement.

Une app graphique macOS n'hérite pas du `PATH` de votre shell : si Claude Code n'est pas trouvé,
Cadence cherche aussi dans `~/.local/bin`, `~/.npm-global/bin` et `/opt/homebrew/bin`, et vous
pouvez indiquer le binaire à la main (onboarding, ou Réglages › Machine locale).

## Windows

1. Lancez `Cadence_<version>_x64-setup.exe`. L'installation se fait pour l'utilisateur courant,
   sans droits administrateur.
2. Cadence utilise **WebView2**, déjà présent sur Windows 10 à jour et Windows 11 ; s'il manque,
   l'installeur le télécharge.
3. Le jeton GitHub est rangé dans le **Gestionnaire d'identifiants Windows**.

Le `.msi` convient aux déploiements gérés (GPO, Intune). Les deux installeurs sont signés : si
SmartScreen avertit malgré tout, vérifiez que l'éditeur affiché est bien celui de Cadence.

## Linux

- **AppImage** (recommandé, mis à jour automatiquement) :
  ```bash
  chmod +x cadence_<version>_amd64.AppImage
  ./cadence_<version>_amd64.AppImage
  ```
- **Debian / Ubuntu** : `sudo apt install ./cadence_<version>_amd64.deb`
- **Fedora / RHEL** : `sudo dnf install ./cadence-<version>-1.x86_64.rpm`

Cadence a besoin de WebKitGTK 4.1 (Ubuntu 22.04+, Debian 12+, Fedora 36+) et d'un service de
trousseau **Secret Service** (GNOME Keyring ou KWallet) pour ranger le jeton GitHub. Les paquets
`.deb` et `.rpm` se mettent à jour par réinstallation du nouveau paquet.

## Mises à jour

Cadence vérifie discrètement au lancement si une nouvelle version est publiée. Le résultat
apparaît dans **Réglages › À propos** (et en pied de la navigation des réglages) :

1. **Rechercher une mise à jour** relance la vérification à la demande ;
2. **Télécharger et installer** récupère l'installeur, dont la signature est vérifiée sur votre
   machine avant toute installation (une mise à jour non signée par Cadence est refusée) ;
3. **Redémarrer maintenant** relance l'app sur la nouvelle version. Sous Windows, l'installeur
   ferme et relance Cadence lui-même.

Les agents en cours reprennent leur file au lancement suivant. Aucune donnée n'est perdue : tout
l'état métier est sur GitHub, le cache local est conservé.

## Désinstaller

- macOS : supprimez `Cadence.app` ; données dans `~/Library/Application Support/app.cadence.desktop`.
- Windows : Paramètres › Applications › Cadence › Désinstaller ; données dans
  `%APPDATA%\app.cadence.desktop` et `%LOCALAPPDATA%\app.cadence.desktop`.
- Linux : supprimez l'AppImage ou le paquet ; données dans `~/.config/app.cadence.desktop` et
  `~/.local/share/app.cadence.desktop`.

Pour retirer aussi le jeton GitHub, déconnectez-vous avant de désinstaller (Réglages › Comptes et
jetons › Se déconnecter), ou supprimez l'entrée `app.cadence.desktop` (compte `github`) du trousseau.
