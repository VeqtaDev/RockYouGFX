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
| 3 · Rasterizer + DDS (Rust) | à faire |
| 4 · Sidecar YTD | à faire |
| 5 · Patcher GFX | à faire |
| 6 · Emitter resource + Alchemist | à faire |
| 7 · Packaging NSIS + portable | à faire |

## Développement

```sh
pnpm install
pnpm dev        # éditeur seul, dans le navigateur
pnpm build      # typecheck + build de production
```

L'éditeur et l'aperçu fonctionnent sans aucun fichier du jeu.

## Assets

Aucun fichier Rockstar n'est distribué avec ce dépôt — ni `graphics.ytd`, ni
`minimap.gfx`, ni tuiles de map. L'app lit ceux de votre propre installation.
Le fond de l'aperçu est un faux plan généré procéduralement.
