---
name: tester-cadence
description: Teste Cadence en conditions réelles comme un utilisateur (vraie app, vrais plugins, vrais boutons, vrai projet bac à sable) et fait remonter les incohérences d'UX/UI et les bugs dans un rapport avec captures annotées. À utiliser pour « tester l'app », « faire une recette », « dogfooder un plugin », « vérifier un parcours », ou après une série de changements avant une release.
---

# Tester Cadence en conditions réelles

But : trouver ce qu'un utilisateur verrait et qu'un test unitaire ne voit pas. Exemple qui a
motivé ce skill : la revue de code n'était possible qu'en la demandant dans le chat, alors qu'un
bouton dans le kanban était attendu. Tu joues **l'utilisateur**, pas le développeur.

Arguments possibles : un parcours (`gestion-projet`, `documents`, `chat`, `onboarding`,
`catalogue`, `complet`), un dépôt bac à sable (`owner/name`), `--plugins dev|catalogue`.
Sans argument : demande le parcours (QCM court) puis lance-le.

## Règles

1. **Uniquement par l'interface.** Clique, tape, glisse comme un humain avec `scripts/cdp.mjs`
   (`voir`, `clic`, `saisir`, `touche`). N'appelle jamais une commande IPC, un outil MCP ni l'API
   GitHub pour faire avancer le parcours : si tu en as besoin, c'est un constat (« action
   impossible depuis l'interface »). `eval`, `gh` en lecture et les journaux servent seulement à
   **vérifier** un résultat ou diagnostiquer.
2. **Instance isolée, toujours.** `scripts/lancer.ps1` démarre « Cadence QA » (identifiant
   `app.cadence.desktop.qa`, trousseau séparé, Vite sur 1439, débogage WebView2 sur 9339,
   build dans `src-tauri/target-agents`). Ne touche jamais au port 1420, aux données
   `app.cadence.desktop` ni à l'app installée de l'utilisateur.
3. **Vrai projet, mais bac à sable.** Les écritures GitHub ne se font que sur un dépôt de test
   (défaut : `Remisdv/linear-test` ; un autre seulement si l'utilisateur le donne). Jamais sur un
   dépôt de travail de l'utilisateur. Annonce le dépôt utilisé au début du rapport.
4. **Vraies IA, budget limité.** Les parcours lancent de vraies sessions (Claude Code, Codex,
   OpenCode installées) avec l'abonnement de l'utilisateur. Annonce-le avant de commencer, garde
   les demandes petites (issues minuscules, sprint d'un jour, un ou deux tours de chat), arrête
   une exécution qui part en boucle.
5. **Ne corrige rien pendant le test.** Tu observes et tu notes. Les corrections viennent après,
   sur accord de l'utilisateur, à partir du rapport.
6. **Ne jamais saisir de secret.** Si un écran demande un jeton ou un mot de passe, utilise la
   connexion existante proposée (ex. reprise de `gh`) ou arrête-toi et demande à l'utilisateur.

## Déroulé

1. **Préparer** : `git status` des deux dépôts (`cadence`, `../cadence-plugins`), branche testée,
   version de l'app et des plugins. Choisir la source des plugins : `catalogue` (versions
   publiées, comme un utilisateur) ou `dev` (dossiers de `../cadence-plugins`, installés depuis
   Réglages › Développeur — par l'interface). Dossier du rapport :
   `qa/rapports/AAAA-MM-JJ-<parcours>/` (captures dans `captures/`).
2. **Lancer** : `powershell -NoProfile -ExecutionPolicy Bypass -File .claude/skills/tester-cadence/scripts/lancer.ps1 -Reinitialiser`
   (en arrière-plan ; la première compilation prend plusieurs minutes). Attendre « PRÊT ».
3. **Parcourir** pas à pas. À chaque étape :
   - `node .claude/skills/tester-cadence/scripts/cdp.mjs voir` puis agir (`clic "#12"`, `saisir`…) ;
   - `capture captures/NN-etape.png` avant/après les étapes clés ;
   - noter ce que l'utilisateur **attendrait** avant de cliquer, puis comparer à ce qui arrive ;
   - après une action longue (IA, sync GitHub) : `attendre "<texte>" 120`, puis vérifier le résultat
     réel (`gh issue view`, `gh pr view` en lecture) ;
   - `console` de temps en temps pour les erreurs du front.
4. **Constater** : pour chaque problème, une capture **annotée** :
   `capture captures/P03.png --cadre "#12" --note "Aucun bouton pour relire depuis le kanban"`
   (`--cadre x,y,l,h` pour une zone sans élément ; plusieurs `--cadre` possibles).
5. **Arrêter** : `scripts/arreter.ps1` (garder les données pour reproduire ; `-Effacer` sinon).
6. **Rapport** puis bilan à l'utilisateur (voir plus bas). Proposer les corrections, par lots.

## Parcours

Adapter à ce qui existe dans la version testée ; un parcours = un récit d'utilisateur complet.

- **onboarding** : premier lancement (données réinitialisées) → IA de code → plugins → outils
  nécessaires → configuration (service Dépôts : reprise de `gh`, dépôts suivis) → arrivée dans l'app.
- **gestion-projet** (vrai mini-projet sur le bac à sable) : décrire un petit produit (3–5 issues
  minuscules, ex. un script « bonjour » en plusieurs étapes) → jalons dans la Roadmap → démarrer un
  sprint d'un jour → « Développer l'issue » / « Développer le sprint » → suivre l'exécution de fond
  → PR créées → tâches « En revue » : relire depuis le kanban (une, puis sélection multiple) →
  valider (approuver + fusionner) → issue fermée, tâche terminée → conversations du sprint visibles
  dans la vue du sprint et **absentes** du chat global → fin de sprint, sprint suivant.
- **documents** : importer un PDF, un DOCX, un PPTX et un MD dans Matière → Chapitre → poser une
  question (citations cliquables vers la bonne page) → prendre une note (éditeur, `/`, sauvegarde,
  modification hors de Cadence) → QCM, questions ouvertes (correction), flashcards.
- **chat** : chat global et contextuel, modes Demander / Édition auto / Auto, interrupteur
  « Commandes », longues commandes, questions à choix de l'IA, interruption, reprise après
  redémarrage de l'app.
- **catalogue** : installer, mettre à jour, désactiver, désinstaller un plugin ; état des outils
  (Réglages › Outils) ; plugin sans binaire pour la plateforme.
- **complet** : tous les parcours à la suite (long ; demander confirmation).

## Grille de lecture (à appliquer à chaque écran)

- **Découvrabilité** : toute action que le chat ou un skill sait faire a-t-elle un bouton là où on
  l'attend (kanban, détail, barre du plugin) ? Une action doit se trouver là où l'utilisateur
  regarde l'objet concerné.
- **Retour** : chaque clic donne-t-il un signe (chargement, succès, erreur lisible en français) ?
  Une exécution de fond est-elle visible et suivable ?
- **Cohérence** : mêmes mots pour les mêmes choses ; compteurs justes ; états identiques entre
  écrans (kanban, détail, GitHub) ; pas de doublons (ex. 60 fois la même conversation).
- **Maquettes** : écart visible avec la page « Cadence 2 » du canvas (artifact
  4fX3USQ4tZEKxFxwaWoDpj) : mise en page, tailles, couleurs (tokens), icônes.
- **Robustesse visuelle** : textes longs, chemins, commandes ; rien ne déborde ni n'est coupé ;
  fenêtre étroite (`taille 1100 760`) ; états vides, chargement, erreur, hors ligne.
- **Sécurité perçue** : action irréversible sans confirmation (fusion, suppression) ; écriture
  GitHub sans que l'utilisateur le sache.
- **Langue** : tout en français, pas de clé technique ni d'anglais brut affiché.
- **Performance ressentie** : attente > 2 s sans indicateur, écran figé.

## Rapport

`qa/rapports/AAAA-MM-JJ-<parcours>/rapport.md` :

```markdown
# Recette <parcours> — <date>

Version : Cadence <x.y.z> (<branche>@<commit>), plugins <source + versions>, IA <laquelle>,
dépôt bac à sable <owner/name>. Durée, nombre d'exécutions d'IA lancées.

## Résumé
<3 phrases : ce qui marche, les 3 problèmes les plus gênants.>

| # | Gravité | Écran | Constat | Capture |
| --- | --- | --- | --- | --- |
| P01 | bloquant | Sprint › En revue | Aucun bouton pour relire une tâche | captures/P01.png |

## Constats
### P01 — <titre court> (bloquant | majeur | mineur | idée)
- **Étapes** : 1. … 2. …
- **Attendu** : …
- **Observé** : … (+ erreur console / journal s'il y en a)
- **Capture** : ![P01](captures/P01.png)
- **Piste** : fichier ou composant probable (`plugins/gestion-projet/ui/src/views/SprintView.tsx`…),
  correction proposée.

## Ce qui fonctionne
<liste courte, pour ne pas le casser.>
```

Gravités : **bloquant** (parcours impossible), **majeur** (contournable mais pénible, ou donnée
fausse), **mineur** (cosmétique), **idée** (amélioration). Ne mets pas un constat sans capture ni
étapes de reproduction. Termine en proposant à l'utilisateur les corrections par lots (une
session ou un sous-agent par zone de fichiers), sans les lancer d'office.
