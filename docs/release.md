# Processus de release (mainteneurs)

Deux workflows GitHub Actions :

- **`ci.yml`** — sur chaque push vers `main` et chaque pull request : `pnpm typecheck`,
  `pnpm test`, `pnpm build` ; `cargo clippy --all-targets -- -D warnings` et `cargo test` sur
  Linux seulement (minutes GitHub) ; vérifie que les types TS générés (`src/lib/bindings`) sont à jour.
  Aucun secret.
- **`release.yml`** — **lancé à la main** (Actions › Release › Run workflow, en choisissant le
  tag `vX.Y.Z` dans « Use workflow from ») pour économiser les minutes : vérifie les versions et les clés, crée un **brouillon**
  de release, construit macOS (binaire universel Apple Silicon + Intel), Windows x64 et Linux x64
  avec `tauri-apps/tauri-action`, signe, notarise, et dépose dans le brouillon les installeurs,
  leurs signatures de mise à jour et `latest.json`. **Rien n'est publié automatiquement.**

## Mise en place (une fois)

### 1. Clé de signature des mises à jour (obligatoire)

L'updater refuse tout paquet qui n'est pas signé par la clé privée de Cadence. Générez la paire
sur une machine de confiance :

```bash
pnpm tauri signer generate -w ~/.tauri/cadence-updater.key
```

- La **clé publique** (contenu de `~/.tauri/cadence-updater.key.pub`) remplace le placeholder
  `REMPLACER-PAR-LA-CLE-PUBLIQUE-DES-MISES-A-JOUR…` de `plugins.updater.pubkey` dans
  `src-tauri/tauri.conf.json`. Commitez-la : elle est publique. Le workflow de release échoue
  tant que le placeholder est en place.
- La **clé privée** (contenu de `~/.tauri/cadence-updater.key`) va dans le secret
  `TAURI_SIGNING_PRIVATE_KEY`, son mot de passe dans `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

> **Ne perdez pas cette clé.** Les apps installées n'acceptent que des mises à jour signées par
> elle : la perdre oblige tous les utilisateurs à réinstaller à la main une version portant une
> nouvelle clé publique. Gardez-en une copie chiffrée hors de GitHub.

### 2. Point de terminaison de l'updater

`plugins.updater.endpoints` vaut
`https://github.com/RemiSDV/cadence/releases/latest/download/latest.json`. Il doit désigner
le dépôt **public** qui héberge les releases (l'app interroge cette URL sans authentification).
Si les releases sont publiées ailleurs, adaptez l'URL (et le lien `homepage` du bundle).

### 3. Secrets et variables du dépôt

À créer dans Settings › Secrets and variables › Actions. Les signatures macOS et Windows sont
facultatives pour un essai (le workflow avertit et construit des binaires non signés), mais
**indispensables** pour une release publique.

| Nom | Type | Rôle |
| --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | secret | clé privée de l'updater (obligatoire) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | secret | son mot de passe |
| `APPLE_CERTIFICATE` | secret | certificat **Developer ID Application** exporté en `.p12`, encodé en base64 |
| `APPLE_CERTIFICATE_PASSWORD` | secret | mot de passe du `.p12` |
| `APPLE_SIGNING_IDENTITY` | secret | identité, ex. `Developer ID Application: Rémi Lhuillier (ABCDE12345)` |
| `APPLE_ID` | secret | identifiant Apple du compte développeur (notarisation) |
| `APPLE_PASSWORD` | secret | **mot de passe d'app** créé sur appleid.apple.com (pas le mot de passe du compte) |
| `APPLE_TEAM_ID` | secret | Team ID du compte développeur (10 caractères) |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | secrets | Windows, voie **Azure Trusted Signing** : principal de service autorisé sur le profil de certificat |
| `AZURE_TRUSTED_SIGNING_ENDPOINT` | variable | ex. `https://weu.codesigning.azure.net` |
| `AZURE_TRUSTED_SIGNING_ACCOUNT` | variable | nom du compte Trusted Signing |
| `AZURE_TRUSTED_SIGNING_PROFILE` | variable | nom du profil de certificat |
| `WINDOWS_CERTIFICATE` | secret | Windows, voie **certificat** : `.pfx` de signature de code encodé en base64 |
| `WINDOWS_CERTIFICATE_PASSWORD` | secret | mot de passe du `.pfx` |
| `CADENCE_GITHUB_CLIENT_ID` | variable | facultatif : OAuth App de Cadence, figée dans le binaire |
| `CADENCE_GITHUB_CLIENT_SECRET` | secret | facultatif : son client secret (voir [technique.md](technique.md#optionnel--oauth-app-propre-à-cadence), il ne peut pas rester confidentiel dans une app native) |

Encodage base64 d'un certificat :

```bash
base64 -i certificat.p12 | pbcopy                       # macOS
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificat.pfx")) | Set-Clipboard   # PowerShell
```

**Windows** : si les secrets Azure sont présents, ils sont prioritaires (le workflow installe
`trusted-signing-cli` et règle `bundle.windows.signCommand`) ; sinon le `.pfx` est importé dans le
magasin de l'utilisateur et désigné par son empreinte (`certificateThumbprint`, SHA-256,
horodatage DigiCert). La configuration générée est passée à `tauri build --config`.

**macOS** : Tauri importe le certificat dans un trousseau temporaire, signe avec le *hardened
runtime*, envoie l'app au service de notarisation d'Apple (`notarytool`) puis agrafe le ticket.
Comptez quelques minutes de plus. Une clé d'API App Store Connect (`APPLE_API_ISSUER`,
`APPLE_API_KEY`, `APPLE_API_KEY_PATH`) peut remplacer Apple ID et mot de passe d'app ; il faut
alors adapter l'étape « Signature et notarisation (macOS) » du workflow.

## Publier une version

1. **Aligner les versions** sur la version à publier (ex. `1.0.0`) dans
   `src-tauri/tauri.conf.json` (`version`), `package.json` et `src-tauri/Cargo.toml`, puis lancer
   `cargo check` dans `src-tauri` pour mettre à jour `Cargo.lock`. Le workflow refuse un tag qui
   ne correspond pas aux trois fichiers.
2. Commiter sur `main` et attendre que la CI soit verte.
3. **Poser le tag et le pousser** :
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
   Une version avec un suffixe (`v1.1.0-beta.1`) crée une pré-release.
4. Suivre le workflow **Release** dans l'onglet Actions : `Vérifications avant build`, `Brouillon
   de release`, puis les trois builds en parallèle.
5. **Vérifier le brouillon** sur la page des releases :
   - installeurs : `.dmg` et `.app.tar.gz` (macOS), `-setup.exe` et `.msi` (Windows),
     `.AppImage`, `.deb`, `.rpm` (Linux), chacun avec son `.sig` pour l'updater ;
   - `latest.json` qui liste `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64` et
     `linux-x86_64` ;
   - signatures : `spctl -a -vv /Applications/Cadence.app` et
     `xcrun stapler validate /Applications/Cadence.app` (macOS),
     `Get-AuthenticodeSignature .\Cadence_1.0.0_x64-setup.exe` (Windows).
6. Rédiger les notes (les notes générées par GitHub servent de base ; ce texte est affiché
   dans Réglages › À propos), puis **publier** la release.

Tant qu'elle est en brouillon, la release est invisible pour l'updater : l'URL
`releases/latest/download/latest.json` désigne la **dernière release publiée** (hors
pré-releases). Dès la publication, les apps installées la proposent au lancement suivant.

### Relancer un build

Relancer un job en échec depuis l'onglet Actions reprend le même brouillon (le workflow le
retrouve par son tag). Pour repartir de zéro, supprimez le brouillon et le tag
(`git push --delete origin v1.0.0`), corrigez, puis reposez le tag.

### Retirer une version défectueuse

L'updater ne revient jamais à une version antérieure. Publiez rapidement une version corrective
(`1.0.1`) ; repasser la release défectueuse en brouillon la retire de `latest/download`, mais ne
désinstalle rien.

## Builds locaux

`createUpdaterArtifacts` étant actif, `pnpm tauri build` exige la clé privée de l'updater :

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/cadence-updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="…"
pnpm tauri build
```

Sans la clé, construisez sans paquets de mise à jour :

```bash
pnpm tauri:build:unsigned
```
