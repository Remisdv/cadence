# Les agents

Cadence orchestre quatre agents. Chacun pilote une session **Claude Code** headless sur votre
machine (`claude -p --output-format stream-json`), avec la session que vous avez ouverte dans la
CLI : Cadence n'a pas de clé d'API et ne parle à aucun modèle directement. Codex CLI peut être
choisi comme moteur dans [la configuration](configuration.md).

| Agent | Rôle | Ce qu'il peut produire |
| --- | --- | --- |
| **CTO** | rend les issues exécutables : description, estimation, labels, questions | réécrire une description, poser une question |
| **CEO** | dérive des issues candidates depuis la roadmap | proposer une issue |
| **Développeur** | implémente une issue : branche, PR, code, tests | créer une branche, ouvrir une PR en brouillon, pousser des commits |
| **Déploiement** | prépare la release : pré-vol, build, tests de vérification, documentation, tag | publier une release, rouvrir une issue |

Toute autre sortie d'un agent est **rejetée** : un agent ne peut produire que les actions de son
rôle. L'agent CEO et l'agent déploiement arrivent dans les versions 0.2.x ; leurs réglages sont
déjà lus dans `.cadence/config.toml`.

## Autonomie

Chaque agent a un niveau d'autonomie, réglé par dépôt (`autonomy` dans `config.toml`, ou
Réglages › Agents) :

| Valeur | Libellé | Effet |
| --- | --- | --- |
| `propose` | Propose seulement | rien n'est écrit sur GitHub ; vous lisez les propositions |
| `apply-after-review` | Applique après validation (défaut) | chaque écriture attend votre clic « Appliquer » / « Pousser » |
| `auto` | Automatique | l'agent applique seul ce qui ne demande pas de décision ; les garde-fous restent actifs |

L'agent **CEO** reste toujours en `propose` : Cadence refuse un `config.toml` qui dit autre chose.

## Agent CTO

Barre latérale › **CTO**, ou « Agent CTO » dans l'écran Issues. Il parcourt les issues ouvertes de
la version qui ne sont pas encore passées par lui (ou celles que vous lui envoyez depuis le
détail d'une issue, ou à la création).

- Une session par issue, dans une **copie superficielle du dépôt** en lecture seule
  (`<données de l'app>/repos/<owner>/<nom>`), outils limités à `Read`, `Grep`, `Glob`.
- Il rend une description au format du projet (`template` : *Contexte / Attendu / Critères
  d'acceptation*, ou libre), une estimation, des labels, un niveau de risque, des issues liées —
  ou jusqu'à trois **questions** quand le titre ne suffit pas. Vos réponses reprennent la même
  session.
- Vous voyez un **diff avant/après**, pouvez éditer, « Passer » ou « Appliquer ». Seule
  l'application écrit sur GitHub : description, labels `agent/cto` et `estimate/*`, commentaire
  signé.

## Agent développeur

Barre latérale › **Développeur**. Cochez des issues de la version : elles passent l'une après
l'autre en six étapes.

1. **Analyse** de l'issue et du dépôt dans un **worktree isolé**
   (`<worktree_root>/<owner>/<nom>/<issue>`, branche `<type>/<numéro>-<slug>`), en lecture seule ;
2. **Branche et PR en brouillon** : Cadence pousse un commit d'ouverture vide et ouvre la PR
   (label `agent/dev`) ;
3. **Implémentation** et 4. **tests**, dans la même session, avec les outils d'écriture et les
   seules commandes de build et de test (`cargo`, `pnpm`, `npm`, `python`, `go`, `make`…) ;
   Cadence commite ensuite le travail sous votre identité GitHub *noreply* ;
5. **Push** : attend votre validation (sauf autonomie `auto`) ;
6. **Sortie du brouillon** et commentaire signé sur la PR.

En autonomie `propose`, rien ne quitte la machine : branche locale, aucun push, aucune PR.
Vous pouvez **mettre en pause** (après l'étape en cours), **arrêter**, ou **intervenir depuis
un terminal** dans le worktree : l'agent reprend à l'étape suivante.

## Garde-fous

Ils s'appliquent à tous les agents, quelle que soit l'autonomie :

- **Portée limitée au dépôt** : chaque session tourne dans une copie ou un worktree du dépôt
  ouvert, jamais ailleurs.
- **Outils restreints** : lecture seule pour le CTO ; pour le développeur, écriture dans le
  worktree et commandes de build/test seulement. Sont interdits explicitement : `git push`,
  `git remote`, `git config`, `git commit`, `gh`, `curl`, `wget`, l'accès web et les
  sous-agents.
- **Aucun identifiant** : le jeton GitHub ne quitte pas le trousseau ; l'environnement des
  sessions n'a aucun assistant d'identifiants Git, et l'URL de push de la copie est invalide.
  Seul Cadence pousse, après vos validations.
- **Jamais sur la branche par défaut** : Cadence refuse de pousser sur la branche par défaut du
  dépôt (ni sur `main` / `master`), même en `auto`.
- **Durée plafonnée** : une session est arrêtée au-delà de `max_session_minutes`.
- **Quota GitHub** : quand le quota est presque épuisé, les agents sont suspendus (pas en
  échec) et reprennent à la réinitialisation.
- **Traçabilité** : toute écriture d'agent porte un label `agent/*` et un commentaire signé ; le
  journal complet de chaque exécution est gardé localement et survit à un redémarrage.
