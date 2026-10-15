# Référence de `.cadence/config.toml`

Les réglages d'un projet vivent **dans le dépôt**, dans `.cadence/config.toml` : ils sont
versionnés avec le code et partagés par toute l'équipe. Le fichier ne contient **aucun secret**.

- **Édition** : Réglages › *Projet et mapping* et *Agents*. Enregistrer crée un commit sur la
  branche par défaut via l'API GitHub ; l'écriture est refusée si le fichier a changé entre-temps
  (rechargez, puis recommencez). Vous pouvez aussi éditer le fichier à la main et le pousser.
- **Fichier absent** : les valeurs par défaut s'appliquent, et les moteurs d'agents reprennent
  l'affectation choisie à l'onboarding sur votre machine. Le premier enregistrement crée le
  fichier.
- **Fichier partiel** : les clés absentes prennent leur valeur par défaut.
- **Clé inconnue** : ignorée (fichier écrit par une version plus récente, faute de frappe).
- **Valeur invalide** (type incorrect, hors bornes) : Cadence affiche l'erreur et n'utilise pas
  le fichier tant qu'il n'est pas corrigé.
- Les clés sont en `snake_case`.

## Exemple complet (valeurs par défaut)

```toml
# Configuration Cadence — https://github.com/cadence-app/desktop
# Versionnée avec le dépôt ; aucun secret ici.

[project]
repo = "owner/nom"
tag_prefix = "v"

[versioning]
scheme = "semver"
confirm_major_bump = true
sprint_days = 14

[issues]
require_milestone = true
send_to_cto_on_create = true
template = "contexte-attendu-criteres"

[agents]
worktree_root = "~/.cadence/wt"

[agents.cto]
engine = "claude-code"
model = "opus"
autonomy = "apply-after-review"
max_session_minutes = 30

[agents.ceo]
engine = "claude-code"
model = "sonnet"
autonomy = "propose"
max_session_minutes = 30

[agents.dev]
engine = "claude-code"
model = "opus"
autonomy = "apply-after-review"
max_session_minutes = 30

[agents.deploy]
engine = "claude-code"
model = "sonnet"
autonomy = "apply-after-review"
max_session_minutes = 30
```

## `[project]`

| Clé | Type | Défaut | Description |
| --- | --- | --- | --- |
| `repo` | texte `owner/nom` | le dépôt ouvert | informatif : **le dépôt ouvert fait toujours foi**, une autre valeur est ignorée |
| `tag_prefix` | texte | `"v"` | préfixe des tags posés à la clôture (`v1.0.0`) ; `""` pour aucun. 10 caractères au plus, sans espace ni `~ ^ : ? * [ \` |

## `[versioning]`

| Clé | Type | Défaut | Description |
| --- | --- | --- | --- |
| `scheme` | texte | `"semver"` | schéma de numérotation ; seul `semver` est pris en charge |
| `confirm_major_bump` | booléen | `true` | demander confirmation avant un palier (`0.2.0`, `1.0.0`) |
| `sprint_days` | entier, 1 à 60 | `14` | durée d'un sprint : échéance du milestone créé à la clôture du précédent |

## `[issues]`

| Clé | Type | Défaut | Description |
| --- | --- | --- | --- |
| `require_milestone` | booléen | `true` | une issue est toujours rattachée à une version ; verrouillé dans l'interface |
| `send_to_cto_on_create` | booléen | `true` | cocher par défaut « Envoyer à l'agent CTO » à la création d'une issue |
| `template` | texte | `"contexte-attendu-criteres"` | structure des descriptions écrites par l'agent CTO : `contexte-attendu-criteres` (sections *Contexte*, *Attendu*, *Critères d'acceptation*) ou `libre` |

## `[agents]`

| Clé | Type | Défaut | Description |
| --- | --- | --- | --- |
| `worktree_root` | chemin | `"~/.cadence/wt"` | racine des worktrees de l'agent développeur (`~` est développé) ; chaque tâche vit dans `<worktree_root>/<owner>/<nom>/<issue>` |

### `[agents.cto]`, `[agents.ceo]`, `[agents.dev]`, `[agents.deploy]`

Même structure pour les quatre agents.

| Clé | Type | Défaut | Description |
| --- | --- | --- | --- |
| `engine` | `"claude-code"` ou `"codex"` | choix de l'onboarding, sinon `"claude-code"` | CLI qui exécute l'agent |
| `model` | texte, facultatif | `opus` (CTO, développeur), `sonnet` (CEO, déploiement) | modèle demandé au moteur (`opus`, `sonnet`…) ; clé absente = modèle par défaut du moteur |
| `autonomy` | `"propose"`, `"apply-after-review"` ou `"auto"` | `apply-after-review` ; `propose` pour le CEO | voir [Autonomie](agents.md#autonomie) ; le CEO doit rester en `propose` |
| `max_session_minutes` | entier, 1 à 240 | `30` | durée maximale d'une session de l'agent |

## Préférences de la machine (`settings.json`)

Ce qui est propre à **votre** machine n'est pas dans le dépôt mais dans `settings.json`, dans le
dossier de configuration de l'app (`~/Library/Application Support/app.cadence.desktop` sur
macOS, `%APPDATA%\app.cadence.desktop` sous Windows, `~/.config/app.cadence.desktop` sous
Linux). Il est géré par l'app ; on ne l'édite normalement pas à la main.

| Clé | Rôle |
| --- | --- |
| `claudeCodePath`, `codexPath` | binaire choisi à la main (« Localiser… ») |
| `disabledTools` | agents de code écartés (`claude-code`, `codex`) |
| `agentEngines` | affectation des rôles faite à l'onboarding (moteur et modèle par agent) |
| `lastProject` | dernier dépôt ouvert, rouvert au lancement |

Le jeton GitHub n'est ni dans `settings.json` ni dans `config.toml` : il est dans le trousseau du
système.

## Variables d'environnement

| Variable | Rôle |
| --- | --- |
| `CADENCE_GITHUB_CLIENT_ID`, `CADENCE_GITHUB_CLIENT_SECRET` | OAuth App de Cadence (connexion par navigateur directe et code d'appairage). Lues au lancement, ou figées dans la build officielle |
| `RUST_LOG` | niveau des journaux du cœur Rust (par défaut `cadence_lib=info`), utile au dépannage |
