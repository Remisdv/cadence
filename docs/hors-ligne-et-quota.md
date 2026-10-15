# Hors ligne et quota GitHub

## Cache local

Chaque dépôt ouvert a son **cache SQLite** dans le dossier de données locales de l'app
(`cache/<owner>/<nom>.sqlite3`). Issues, milestones et pull requests y sont rangés à chaque
synchronisation. Au lancement, Cadence affiche d'abord ce cache, puis le complète.

La déconnexion du compte GitHub supprime toutes les bases du cache.

## Hors ligne

Sans réseau, Cadence reste utilisable en **lecture** : les écrans servent le cache local (issues,
versions, roadmap, pull requests déjà vues). Les écritures sur GitHub (création d'issue,
clôture de version, application d'une proposition d'agent, push) demandent le réseau ; les
agents eux-mêmes ont besoin de Claude Code, qui est un service en ligne.

Au retour du réseau, la synchronisation reprend d'elle-même. Le détail du mode hors ligne
(écritures mises en attente, indicateurs) dépend de votre version de Cadence : voyez les notes
de version.

## Quota de l'API GitHub

Un jeton GitHub dispose de **5 000 requêtes par heure**. Cadence les ménage :

- au plus **une passe par minute** et par ressource ;
- lectures **incrémentales** (seules les issues modifiées depuis la dernière passe) ;
- requêtes **conditionnelles** (ETag) : un dépôt calme répond `304 Not Modified`, ce qui ne
  consomme pas de quota ;
- une seule requête GraphQL pour la liste des pull requests et leur CI.

Selon le quota restant, Cadence adapte son rythme :

| Requêtes restantes | Régime | Effet |
| --- | --- | --- |
| plus de 1 000 | normal | synchronisation toutes les 60 s |
| de 200 à 1 000 | économe | synchronisation toutes les 5 min |
| moins de 200 | gel | lecture du cache seule ; les agents sont **suspendus** (pas en échec) et reprennent à la réinitialisation du quota |

Le quota est partagé avec les autres outils qui utilisent le même jeton (CLI `gh`, scripts,
CI locale) : si vous atteignez souvent le régime économe, utilisez un jeton dédié à Cadence.
