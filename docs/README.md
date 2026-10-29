# Documentation de Cadence

- **[Guide d'utilisation](utilisation.md)** (Cadence 2, 0.3) : installer, premier lancement, chat,
  plugins, connexion GitHub, réglages.
- **[Documentation technique](technique.md)** : construire depuis les sources, mode mock,
  architecture, recettes, état d'avancement.
- **[Processus de release](release.md)** : secrets, clé de mise à jour, tag, signature.

## Pages de Cadence 0.2

> Les pages ci-dessous décrivent encore la 0.2 (tag `v0.2-final`), avant que Cadence devienne un
> hôte d'IA et de plugins ; elles seront réécrites avec la documentation des plugins.

Cadence 0.2 était un cockpit de projet logiciel adossé à GitHub : un sprint est une version, une
version est un milestone.

1. [Installation](installation.md) — télécharger, installer, mettre à jour, par système.
2. [Premiers pas](premiers-pas.md) — connexion à GitHub, agents de code, choix du dépôt.
3. [Issues, versions et roadmap](projet.md) — comment Cadence lit et écrit votre dépôt.
4. [Hors ligne et quota GitHub](hors-ligne-et-quota.md) — ce qui marche sans réseau, et comment
   Cadence ménage le quota de l'API.
5. [FAQ et dépannage](depannage.md).

## Maintenir Cadence

- [Processus de release](release.md) — secrets, clé de mise à jour, tag, signature, notarisation.
- Développement : voir la [documentation technique](technique.md) (démarrer, mode mock,
  architecture, recettes).

## Principes à retenir

- **GitHub est la source de vérité.** Cadence ne stocke aucun état métier de son côté : versions
  = milestones, types = labels `type/*`, traces d'agent = labels `agent/*` et commentaires signés.
  Le cache local n'est qu'une copie.
- **Le jeton GitHub reste dans le trousseau du système.** Il n'est jamais écrit sur disque, ni
  montré dans l'interface.
