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
   (`voir`, `clic`, `saisir`, `cocher`, `defiler`, `touche` ; voir « Pilote » plus bas). N'appelle jamais une commande IPC, un outil MCP ni l'API
   GitHub pour faire avancer le parcours : si tu en as besoin, c'est un constat (« action
   impossible depuis l'interface »). `eval`, `gh` en lecture et les journaux servent seulement à
   **vérifier** un résultat ou diagnostiquer.
2. **Instance isolée, toujours.** `scripts/lancer.ps1` démarre « Cadence QA » (identifiant
   `app.cadence.desktop.qa`, trousseau séparé, Vite sur 1439, débogage WebView2 sur 9339,
   build dans `src-tauri/target-agents`). Ne touche jamais au port 1420, aux données
   `app.cadence.desktop` ni à l'app installée de l'utilisateur.
3. **Vrai projet, mais bac à sable.** Les écritures GitHub ne se font que sur un dépôt de test
   (défaut : `Remisdv/linear-test` ; un autre seulement si l'utilisateur le donne). Jamais sur un
   dépôt de travail de l'utilisateur. Annonce le dépôt utilisé au début du rapport. Le bac à
   sable a souvent déjà servi : fais l'inventaire au début (voir Déroulé, étape 1).
4. **Vraies IA, budget limité.** Les parcours lancent de vraies sessions (Claude Code, Codex,
   OpenCode installées) avec l'abonnement de l'utilisateur. Annonce-le avant de commencer, garde
   les demandes petites (issues minuscules, sprint d'un jour, un ou deux tours de chat), arrête
   une exécution qui part en boucle. **Compter les exécutions** : un tour d'IA = une exécution
   (un message envoyé dans un chat et sa réponse complète ; un bouton qui lance un skill ; chaque
   issue d'un « Développer le sprint » ; une relance automatique). Une conversation de trois
   échanges compte trois. Note-les au fil de l'eau (heure, écran, IA, ce qui l'a lancée) et
   reporte le total par IA dans l'en-tête du rapport.
5. **Ne corrige rien pendant le test.** Tu observes et tu notes. Les corrections viennent après,
   sur accord de l'utilisateur, à partir du rapport.
6. **Ne jamais saisir de secret.** Si un écran demande un jeton ou un mot de passe, utilise la
   connexion existante proposée (ex. reprise de `gh`) ou arrête-toi et demande à l'utilisateur.
7. **Jamais de clavier ni de souris globaux.** Interdits : `SendKeys`, `keybd_event`,
   `SendInput`, AutoHotkey, déplacement du curseur système, ou toute frappe qui part vers « la
   fenêtre au premier plan » : elle peut être l'app Claude ou une autre fenêtre de l'utilisateur.
   Tout passe par `cdp.mjs` (WebView2 de Cadence QA). Une boîte de dialogue native (sélecteur de
   fichier…) que le pilote ne voit pas est un **constat** ; seul contournement permis : des
   messages Win32 adressés au handle de fenêtre de cette boîte, après avoir vérifié qu'elle
   appartient au processus de Cadence QA. Sinon, arrête-toi et demande à l'utilisateur.

## Déroulé

1. **Préparer** : `git status` des deux dépôts (`cadence`, `../cadence-plugins`), branche testée,
   version de l'app et des plugins. Choisir la source des plugins : `catalogue` (versions
   publiées, comme un utilisateur) ou `dev` (dossiers de `../cadence-plugins`, installés depuis
   Réglages › Développeur — par l'interface). Dossier du rapport :
   `qa/rapports/AAAA-MM-JJ-<parcours>/` (captures dans `captures/`).
   **Inventaire du bac à sable** (lecture seule, `gh`) : sprint en cours (issue d'état du plugin,
   milestone ouvert à échéance proche), milestones, issues et PR ouvertes, branches `release/*`.
   Note-le en tête du rapport. Puis, au choix de l'utilisateur (QCM court) :
   - **travailler à côté** (défaut) : tes issues, ton milestone et ta branche sont nouveaux et
     préfixés (ex. « QA 21/10 — … ») ; ne ferme, ne déplace, ne renomme rien d'existant ; si un
     sprint automatique tourne déjà, ne le termine pas, crée le tien à côté ou teste dans le sien
     en le signalant ;
   - **repartir d'un dépôt neuf** : l'utilisateur crée lui-même un dépôt vide (`owner/linear-test-2`…)
     et te le donne ; tu ne crées jamais de dépôt.
   Tout ce que tu as écrit sur GitHub va dans la section « Écritures » du rapport.
2. **Lancer** : `powershell -NoProfile -ExecutionPolicy Bypass -File .claude/skills/tester-cadence/scripts/lancer.ps1 -Reinitialiser`
   (en arrière-plan ; compilation : ~30 s si rien n'a changé, 5 min ou plus sinon ; un point
   d'avancement par minute). Attendre « PRÊT ». En cas d'« ÉCHEC », le script affiche les erreurs
   de compilation : c'est un constat bloquant, pas à corriger pendant la recette.
   - L'instance ne se recompile **pas** quand `src-tauri` change (`--no-watch`) : un autre agent
     qui modifie le Rust ne la redémarre pas en pleine recette. `-Surveiller` rétablit le
     rechargement. Le front (Vite) se recharge toujours à chaud.
   - Journal horodaté (compilation, Vite, traces `cadence_lib` en debug : plugins, serveurs de
     plugin, sessions d'IA, passerelle) : `%TEMP%\cadence-qa.log` ; `-RustLog "<filtre>"` pour
     l'ajuster.
   - En arrière-plan dès le début : `cdp.mjs console --ecouter` (tampon des erreurs du front).
3. **Parcourir** pas à pas. À chaque étape :
   - `node .claude/skills/tester-cadence/scripts/cdp.mjs voir` puis agir (`clic "#12"`, `saisir`…) ;
   - `capture captures/NN-etape.png` avant/après les étapes clés ;
   - noter ce que l'utilisateur **attendrait** avant de cliquer, puis comparer à ce qui arrive ;
   - après une action longue (IA, sync GitHub) : `attendre "<texte>" 120` (ou `--absent` pour un
     « En cours… » qui doit partir), puis vérifier le résultat réel (`gh issue view`, `gh pr view`
     en lecture) ;
   - **suivre une exécution de fond** comme un utilisateur : revenir à la vue qui la montre (vue du
     sprint, conversation du sprint), `attendre` sa fin, puis contrôler le résultat ;
   - `console` de temps en temps pour les erreurs du front (app et plugins).
4. **Constater** : pour chaque problème, une capture **annotée** :
   `capture captures/P03.png --cadre "#12" --note "Aucun bouton pour relire depuis le kanban"`
   (`--cadre x,y,l,h` pour une zone sans élément, `--cadre "<texte>"` pour un élément par son
   libellé ; plusieurs `--cadre` possibles ; l'étiquette se place d'elle-même sans masquer le cadre).
5. **Arrêter** : `scripts/arreter.ps1` (garder les données pour reproduire ; `-Effacer` sinon).
   Il arrête tout l'arbre lancé par `lancer.ps1`, jamais l'app de l'utilisateur ni le port 1420.
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
  fenêtre étroite (`taille 1100 760`, puis `taille max`) ; états vides, chargement, erreur, hors ligne.
- **Sécurité perçue** : action irréversible sans confirmation (fusion, suppression) ; écriture
  GitHub sans que l'utilisateur le sache.
- **Langue** : tout en français, pas de clé technique ni d'anglais brut affiché.
- **Performance ressentie** : attente > 2 s sans indicateur, écran figé.

## Pilote (`scripts/cdp.mjs`)

`node .claude/skills/tester-cadence/scripts/cdp.mjs <commande>` (détail en tête du fichier). Il
voit la page de Cadence (« app ») **et les iframes des plugins** (« plugin <id> », hors
processus) ; les coordonnées sont celles de la fenêtre, pour les deux.

| Commande | Effet |
| --- | --- |
| `voir [--tout] [--cadre app\|plugin\|<id>]` | éléments cliquables **à l'écran**, numérotés ; hors écran et recouverts (modale) seulement comptés, `--tout` les liste |
| `clic <#n\|texte> [rang] [--cadre …]` | amène l'élément dans la vue puis vrai clic souris ; texte : exact > début > inclusion, plugin affiché avant l'app ; signale les autres correspondances |
| `saisir <#n\|texte> <valeur>` | remplace le contenu d'un champ ; sur un `<select>`, choisit l'option par libellé |
| `cocher <#n\|texte> [oui\|non]` | case, radio, interrupteur (ne clique que si l'état doit changer) |
| `defiler <#n\|texte\|haut\|bas\|+px\|-px> [--cadre …]` | amène un élément au centre, ou fait défiler la zone principale du cadre actif (le plugin s'il est affiché) |
| `taper <texte>`, `touche <Enter\|Escape\|ctrl+a…> [n]` | clavier, là où est le focus (y compris dans un plugin) |
| `attendre <texte> [s] [--absent] [--cadre …]` | attend qu'un texte apparaisse ou parte (code de sortie 1 sinon) |
| `texte [max] [--cadre …] [--delai s]` | texte visible par cadre ; délai par cadre (5 s), un cadre figé est signalé « illisible » |
| `capture <f.png> [--cadre #n\|texte\|x,y,l,h]… [--note …]` | capture annotée ; l'étiquette va dessous, sinon dessus, sinon à côté, sans couvrir le cadre |
| `console [--effacer]` / `console --ecouter [s]` | erreurs depuis le chargement de chaque cadre (relecture CDP + collecteur injecté) ; écoute continue vers `%TEMP%\cadence-qa-pilote\console.log` |
| `taille [<l> <h>\|max]` | redimensionne la vraie fenêtre (Win32) pour une zone de rendu de l×h px CSS ; durable ; `taille max` pour revenir |
| `eval <expr> [--cadre …]` | diagnostic seulement, jamais pour faire avancer le parcours |

Pièges : relancer `voir` après chaque changement d'écran (un `#n` périmé est retrouvé par son
libellé s'il est unique, sinon erreur) ; juste après une transition, `voir` peut être vide : le
relancer. Si un autre agent modifie le front pendant la recette, Vite recharge la page (et les
iframes) : écran vide quelques secondes, retour à l'écran d'accueil, état perdu. Ce n'est pas un
constat ; attendre, relancer `voir`, reprendre l'étape. Une liste déroulante maison (`role=combobox`) se pilote par `clic` puis `clic` sur
l'option. L'état du pilote (numérotation, tampon console) vit dans `%TEMP%\cadence-qa-pilote`,
jamais dans le dépôt.

## Rapport

`qa/rapports/AAAA-MM-JJ-<parcours>/rapport.md` :

```markdown
# Recette <parcours> — <date>

Version : Cadence <x.y.z> (<branche>@<commit>), plugins <source + versions>, IA <laquelle>,
dépôt bac à sable <owner/name> (état de départ : sprint en cours, milestones, issues ouvertes).
Durée, nombre d'exécutions d'IA (tours) par IA.

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

## Écritures sur <owner/name>
<issues, PR, milestones, branches créés ou modifiés, et leur état final.>

## Outil de recette
<ce qui a gêné dans le pilote ou ce skill, pour l'améliorer.>
```

Gravités : **bloquant** (parcours impossible), **majeur** (contournable mais pénible, ou donnée
fausse), **mineur** (cosmétique), **idée** (amélioration). Ne mets pas un constat sans capture ni
étapes de reproduction. Termine en proposant à l'utilisateur les corrections par lots (une
session ou un sous-agent par zone de fichiers), sans les lancer d'office.
