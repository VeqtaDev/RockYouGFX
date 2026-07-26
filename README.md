# RockYouGFX

Éditeur visuel de minimap GTA V pour FiveM. On dessine la forme voulue — coins
arrondis, cercle, squircle, polygone — et l'outil sort une resource FiveM prête
à déposer dans `resources/`.

## Ce qui définit vraiment la forme de la minimap

C'est le contresens le plus répandu du modding FiveM, et il conditionne toute
l'architecture du projet.

La forme du radar **ne vient pas du `.gfx`**. Elle vient du canal alpha de deux
textures DDS, `radarmasksm.dds` (minimap normale) et `radarmasklg.dds` (carte
agrandie), stockées dans `graphics.ytd` sous `x64/textures`. C'est ainsi que
fonctionnent tous les mods « Circle Minimap ».

Le `minimap.gfx`, lui, porte le Scaleform posé par-dessus : la bordure du radar,
les barres de vie et d'armure, la boussole, et l'ActionScript
`com.rockstar.gtav.minimap > MINIMAP`.

Modifier un seul des deux donne un résultat cassé :

| Ce qu'on modifie | Résultat |
| --- | --- |
| Le `.gfx` seul | La forme ne change pas du tout |
| Le masque DDS seul | Map ronde, bordure rectangulaire vanilla, barres de vie dans le vide |
| Les deux | Ce que l'outil produit |

Les deux sont donc générés depuis une **définition de forme unique**
(`src/lib/shape.ts`), ce qui garantit qu'ils restent cohérents.

## Faut-il fournir un minimap.gfx ?

Pour l'essentiel, non — et c'est délibéré.

Le `minimap.gfx` n'est pas un fichier passif : le jeu *appelle dedans*.
`SETUP_HEALTH_ARMOUR` est une méthode Scaleform invoquée sur ce movie. Générer
un `.gfx` de zéro obligerait donc à réimplémenter tout le contrat ActionScript
de Rockstar, et la moindre méthode manquante casse le HUD.

L'outil prend le chemin inverse : il émet un `client.lua` qui appelle ces
mêmes méthodes sur le Scaleform vanilla. Masquer les barres de vie et d'armure
ne demande alors **aucun asset du jeu**.

| Besoin | Moyen | Asset requis |
| --- | --- | --- |
| Forme du radar | masque alpha dans `graphics.ytd` | `graphics.ytd` |
| Masquer vie/armure | `client.lua` généré | aucun |
| Repositionner le HUD, déplacer la boussole | patch du `minimap.gfx` | `minimap.gfx` |

`crates/core/src/lua.rs` expose `limitations()`, qui énumère ce que les natives
ne savent pas faire, pour que l'interface le dise au lieu de laisser
l'utilisateur le découvrir en jeu.

## Mise à jour automatique

L'application interroge les releases GitHub au démarrage et affiche un bandeau
lorsqu'une version plus récente existe.

La vérification liste les releases au lieu d'interroger `/releases/latest` :
cet endpoint **exclut les préversions** et renvoie 404 tant qu'aucune version
stable n'est publiée.

L'installation silencieuse n'est pas encore branchée. Elle demande soit le
plugin updater de Tauri, qui exige une paire de clés de signature dont la
privée doit vivre dans les secrets du dépôt, soit — pour le portable — un
mécanisme de renommage puis relance, l'exécutable en cours ne pouvant pas être
écrasé sous Windows.

## Architecture

```
MinimapShape (JSON, source de vérité unique)
 ├─ rasterizer  → masques alpha → radarmasksm.dds + radarmasklg.dds → patch graphics.ytd
 ├─ gfx patcher → minimap.gfx (bordure, HUD, boussole)
 └─ emitter     → fxmanifest.lua + stream/
                   └─ Alchemist CLI → variante Enhanced
```

Deux invariants portent le projet :

- **Tout est exprimé en fractions, jamais en pixels.** La même définition
  alimente deux textures de dimensions différentes ; un rayon en pixels
  donnerait deux formes distinctes.
- **Le `.gfx` est édité par splice d'octets, jamais ré-encodé.** Les tags
  propriétaires Scaleform (plage 1000+) ne survivraient pas forcément à un
  cycle parse → ré-encode.

## État

| Lot | État |
| --- | --- |
| 1 · Socle Tauri + React + design system iOS | ✅ |
| 2 · Modèle de forme, éditeur, aperçu live | ✅ |
| 3 · Rasterizer + DDS (Rust) | ✅ |
| 5 · Patcher GFX par splice d'octets | ✅ |
| 6 · Emitter resource + script client Lua | ✅ (conversion Alchemist à brancher) |
| 7 · Packaging NSIS + portable | ✅ via GitHub Actions |
| 4 · Sidecar YTD | à faire — demande le SDK .NET |

Le cœur Rust est couvert par 28 tests. Deux d'entre eux portent le projet :

- **`le_round_trip_neutre_est_identique_octet_pour_octet`** — relire puis
  réécrire un `.gfx` sans le modifier rend un fichier identique. C'est la
  garantie que rien n'est perdu, tags Scaleform propriétaires compris.
- **`le_contour_rust_correspond_au_contour_typescript`** — le contour calculé
  par Rust est comparé à celui exporté depuis TypeScript sur six familles de
  formes. Sans lui, l'aperçu et le DDS pourraient diverger en silence.

## Développement

```sh
pnpm install
pnpm dev        # éditeur seul, dans le navigateur
pnpm build      # typecheck + build de production
pnpm test       # tests du frontend
pnpm outlines   # régénère les contours de référence pour le test de conformité

cargo test      # cœur métier Rust
```

L'éditeur, l'aperçu et l'intégralité des tests fonctionnent sans aucun fichier
du jeu.

### Limite de vérification

Le patcher `.gfx` est testé sur des fichiers synthétisés, pas sur un vrai
`minimap.gfx` — aucun asset Rockstar n'étant redistribuable. La mécanique de
splice est donc validée, mais pas la sémantique du fichier réel : identifier
la bordure et les barres de vie dans le Scaleform d'origine reste à faire sur
un fichier fourni par l'utilisateur.

## Assets

Aucun fichier Rockstar n'est distribué avec ce dépôt — ni `graphics.ytd`, ni
`minimap.gfx`, ni tuiles de map. L'app lit ceux de votre propre installation.
Le fond de l'aperçu est un faux plan généré procéduralement.
