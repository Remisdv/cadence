# Documentation de Cadence

Cadence est un cockpit de projet logiciel adossé à GitHub : un sprint est une version, une
version est un milestone, et des agents Claude Code proposent pendant que vous disposez. C'est
une application de bureau (macOS, Windows, Linux).

## Utiliser Cadence

1. [Installation](installation.md) — télécharger, installer, mettre à jour, par système.
2. [Premiers pas](premiers-pas.md) — connexion à GitHub, agents de code, choix du dépôt.
3. [Issues, versions et roadmap](projet.md) — comment Cadence lit et écrit votre dépôt.
4. [Les agents](agents.md) — CTO, CEO, développeur, déploiement, et leurs garde-fous.
5. [Référence de `.cadence/config.toml`](configuration.md) — toutes les clés, valeurs et défauts.
6. [Hors ligne et quota GitHub](hors-ligne-et-quota.md) — ce qui marche sans réseau, et comment
   Cadence ménage le quota de l'API.
7. [FAQ et dépannage](depannage.md).

## Maintenir Cadence

- [Processus de release](release.md) — secrets, clé de mise à jour, tag, signature, notarisation.
- Développement : voir le [README du dépôt](../README.md) (démarrer, mode mock, architecture,
  recettes).

## Principes à retenir

- **GitHub est la source de vérité.** Cadence ne stocke aucun état métier de son côté : versions
  = milestones, types = labels `type/*`, traces d'agent = labels `agent/*` et commentaires signés.
  Le cache local n'est qu'une copie.
- **Le jeton GitHub reste dans le trousseau du système.** Il n'est jamais écrit sur disque, ni
  montré dans l'interface.
- **Les agents proposent, vous disposez.** Par défaut, rien n'est écrit sur GitHub sans votre
  validation explicite ; les garde-fous restent actifs quelle que soit l'autonomie choisie.
