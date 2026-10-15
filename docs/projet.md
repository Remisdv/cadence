# Issues, versions et roadmap

Cadence ne crée aucun objet qui lui soit propre : chaque notion de l'app correspond à un objet
GitHub existant. Vous pouvez donc continuer à travailler depuis github.com, la CLI `gh` ou un
autre outil, Cadence le verra à la synchronisation suivante.

| Dans Cadence | Sur GitHub |
| --- | --- |
| Version / sprint | un **milestone** ; son échéance est la fin du sprint |
| Version livrée | un **tag Git** (préfixe `v` par défaut) et un brouillon de **release** |
| Type d'issue (fonctionnalité, bug…) | un label **`type/*`** |
| Trace d'un agent | un label **`agent/*`** et un commentaire signé |
| Estimation de l'agent CTO | un label **`estimate/*`** |
| Thème de palier (roadmap) | un élément de liste dans la description du milestone |

## Versions et numérotation

Les versions suivent **semver** (`MAJEUR.MINEUR.CORRECTIF`) :

- chaque **sprint** incrémente le dernier segment (`0.1.2` → `0.1.3`) ;
- un **palier** se décide à la main (`0.2.0`, `1.0.0`), avec une confirmation si
  `confirm_major_bump` est actif ;
- un sprint dure `sprint_days` jours (14 par défaut) et commence à l'échéance du précédent.

La **première ligne** de la description d'un milestone nomme la version (`# Cockpit Tauri`) ; ses
éléments de liste (`- …`) sont les thèmes d'un palier, affichés dans la roadmap.

### Clôturer une version

Depuis la roadmap ou `Ctrl+K` › « Clôturer la version » :

1. choisissez la version suivante (sprint, palier, majeure ou saisie libre) ;
2. décidez du sort des issues encore ouvertes : **reporter** sur la version suivante, **retirer**
   du milestone, ou **fermer comme non retenues** ;
3. relisez l'**aperçu exact des écritures**, puis confirmez.

Tout est vérifié avant la première écriture (version semver plus grande, tag libre, branche
lisible). Ensuite, dans l'ordre : le milestone suivant est créé ou daté, les issues sont
traitées, le milestone est fermé, un tag léger est posé sur la branche par défaut, et un
**brouillon** de release est créé avec des notes générées. La release n'est jamais publiée
automatiquement. Si une étape échoue, le message dit exactement ce qui a déjà été écrit.

## Issues

L'écran **Issues** montre toutes les issues du dépôt ouvert (les pull requests sont exclues),
groupées par version avec leur avancement. Filtres : texte, version, état, assigné ; sélection
multiple.

**Nouvelle issue** (bouton des écrans ou `Ctrl+K`) : le titre et la version (un milestone ouvert)
sont obligatoires ; le type est posé en label `type/*` ; labels libres et assignation à soi en
option. Cocher **Envoyer à l'agent CTO** l'ajoute à la file de l'agent (voir [les agents](agents.md)).

Un clic sur une issue ouvre son **détail** : description rendue, état de l'agent CTO (en file, en
cours, proposition à relire, questions, validée), propriétés (version, labels, assigné, branche
et PR liée, estimation).

## Pull requests et revues

- **Pull requests** : PR ouvertes et 30 dernières fusionnées ou fermées, triées par activité, avec
  la CI du dernier commit, les approbations et les changements demandés. Filtres version / état /
  auteur / origine (une PR est « d'agent » si elle porte un label `agent/*`).
- **Revues à faire** : PR dont vous êtes relecteur, et vos PR en attente de revue ; diff fichier
  par fichier, description, impact sur la version.

## Roadmap

Une carte par version : livrées et en cours en haut, sprints planifiés et paliers en bas, avec
l'avancement et les premières issues (ouvertes d'abord).

## Synchronisation

Cadence lit GitHub au plus une fois par minute et par ressource, de façon incrémentale et
conditionnelle : un dépôt calme ne consomme presque pas de quota. Les écrans se rafraîchissent
seuls. Voir [Hors ligne et quota](hors-ligne-et-quota.md).
