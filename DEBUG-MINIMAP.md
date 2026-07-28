# Reprise du débogage — la minimap ne change pas en jeu

Note de passage de relais. Cette session a été menée depuis un conteneur Linux
distant, **sans accès à la machine de l'utilisateur ni à GTA V**. Rien n'a donc
jamais pu être vérifié en jeu, et trois correctifs successifs ont échoué.

Si vous lisez ceci depuis une session locale : vous avez l'accès disque qui
manquait. C'est ce qui va trancher.

## État

L'éditeur, la génération des masques et l'export fonctionnent et sont testés
(49 tests côté Rust, 9 côté frontend). Le problème est **uniquement** que le
jeu n'affiche pas la nouvelle forme.

## Ce qui est établi, avec certitude

- Les masques vivent dans `radarmasksm` (512×256) et `radarmasklg` (512×512),
  **DXT1**, dans `graphics.ytd`. La forme est portée par la **luminance** ;
  l'alpha est uniformément opaque. Mesures dans `crates/core/src/vanilla.rs`.
- Le patch du `.ytd` fonctionne : textures retrouvées aux offsets attendus,
  remplacées, fichier réécrit et relu avec une taille décompressée identique.
  Vérifiable par `cargo run -p rockyougfx-core --example patch_ytd`.
- Le patcher `.gfx` est neutre au round-trip sur un vrai `minimap.gfx` de
  224 161 octets et 3030 tags — identité octet pour octet.
- Le Lua généré est **identique** en natives et en ordre d'arguments à
  `fh4map`, une resource minimap FiveM en état de marche.

## Les trois hypothèses déjà éliminées

1. **Absence du `.ytd` dans la resource** — l'utilisateur a confirmé sa
   présence.
2. **Format Enhanced manquant (Alchemist)** — l'utilisateur est en Legacy
   (gen8), donc hors de cause.
3. **Surcharge de `graphics.ytd` via `stream/`** — sans effet, c'est un
   dictionnaire de base. Remplacé par `AddReplaceTexture`, sans succès non plus.

## L'hypothèse restante, la plus probable

La resource livre une **copie de `graphics.ytd` renommée** (~1,8 Mo). Les
resources qui fonctionnent, comme `fh4map`, embarquent un dictionnaire **dédié
et minimal** (`circlemap.ytd`), ne contenant que les deux masques.

Si le moteur identifie un dictionnaire par un nom interne plutôt que par le nom
de fichier, celui produit ici reste `graphics`. `RequestStreamedTextureDict`
n'aboutit alors jamais, et l'échec est **silencieux**.

La v0.1.6 borne cette attente à 10 secondes et journalise chaque étape,
justement pour lever ce doute.

## Par où commencer, en local

1. **Lire la console F8** après avoir démarré la resource. Les lignes
   `[RockYouGFX]` désignent directement le coupable :

   | Sortie | Conclusion |
   | --- | --- |
   | `ERREUR : le dictionnaire ... ne se charge pas` | Hypothèse confirmée — il faut un vrai `.ytd` dédié |
   | `chargé en N ms` puis `remplacé`, sans effet visuel | Le dictionnaire passe ; c'est `AddReplaceTexture` qui n'accroche pas |
   | Aucune ligne | La resource ne démarre pas — `server.cfg` ou manifeste |

2. **Comparer à une resource qui marche.** Récupérer un `circlemap.ytd` d'une
   resource minimap fonctionnelle et l'ouvrir dans CodeWalker à côté du `.ytd`
   produit ici. La différence de structure devrait sauter aux yeux.

3. **Si le dictionnaire est bien le problème** : il faudra construire un `.ytd`
   depuis zéro plutôt que d'en patcher un. Le conteneur RSC7 est déjà compris
   et implémenté dans `crates/core/src/ytd.rs` — en lecture et en écriture.
   Ce qui manque, c'est la construction de la pagination et de la table du
   dictionnaire, ce qui n'a pas été tenté ici.

## Un point à trancher au passage

La resource embarque aujourd'hui l'intégralité des textures de `graphics.ytd`,
soit ~1,8 Mo d'assets Rockstar, alors que deux suffiraient. C'est une raison de
plus de fabriquer un dictionnaire minimal.

## Fichiers utiles

| Chemin | Rôle |
| --- | --- |
| `crates/core/src/ytd.rs` | Conteneur RSC7 : lecture, patch, écriture |
| `crates/core/src/vanilla.rs` | Mesures du vanilla, avec leur justification |
| `crates/core/src/lua.rs` | Script client généré |
| `crates/core/examples/patch_ytd.rs` | Patch et vérifie un `.ytd` réel |
| `crates/core/examples/inspect.rs` | Inventaire des tags d'un `.gfx` |

Les fichiers du jeu ne sont pas versionnés. Déposer `graphics.ytd` et
`minimap.gfx` dans `fixtures/game/`, déjà couvert par le `.gitignore`.
