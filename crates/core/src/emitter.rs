//! Assemblage de la resource FiveM.

/// Un fichier à écrire, chemin relatif à la racine de la resource.
#[derive(Debug, Clone)]
pub struct ResourceFile {
    pub path: String,
    pub data: Vec<u8>,
}

/// Cible du serveur. Depuis le build b95, l'override d'un `minimap.gfx` ou
/// `minimap.ytd` streamé fonctionne aussi sur Enhanced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Legacy,
    Enhanced,
}

/// Génère le `fxmanifest.lua`.
///
/// Le dossier `stream/` n'y est volontairement pas déclaré : FiveM le détecte
/// tout seul, et l'énumérer est une source classique d'erreurs de chargement.
/// Le script client, lui, doit l'être explicitement.
pub fn fxmanifest(name: &str, target: Target, has_client_script: bool) -> String {
    let cible = match target {
        Target::Legacy => "GTA V Legacy (gen8)",
        Target::Enhanced => "GTA V Enhanced (gen9)",
    };
    let client = if has_client_script { "\nclient_script 'client.lua'\n" } else { "" };
    format!(
        "-- Généré par RockYouGFX\n\
         -- Cible : {cible}\n\
         --\n\
         -- Le dossier stream/ est détecté automatiquement par FiveM :\n\
         -- il n'a pas à être déclaré ici.\n\
         \n\
         fx_version 'cerulean'\n\
         game 'gta5'\n\
         \n\
         name '{name}'\n\
         description 'Masque et Scaleform de minimap personnalisés'\n\
         version '1.0.0'\n{client}"
    )
}

/// Assemble la resource complète.
///
/// `mask_ytd` est le dictionnaire de masques : son nom (sans extension) et son
/// contenu. Il est livré sous un nom **propre à la resource**, jamais sous
/// `graphics.ytd` : un dictionnaire de base du jeu n'est pas surchargeable par
/// `stream/`, alors qu'un dictionnaire nouveau se streame normalement. C'est le
/// `client.lua` qui demande ensuite au jeu d'y piocher les masques.
pub fn build_resource(
    name: &str,
    target: Target,
    mask_ytd: Option<(String, Vec<u8>)>,
    minimap_gfx: Option<Vec<u8>>,
    client_lua: Option<String>,
) -> Vec<ResourceFile> {
    let mut files = vec![ResourceFile {
        path: "fxmanifest.lua".into(),
        data: fxmanifest(name, target, client_lua.is_some()).into_bytes(),
    }];

    if let Some((dict, data)) = mask_ytd {
        files.push(ResourceFile { path: format!("stream/{dict}.ytd"), data });
    }
    if let Some(data) = minimap_gfx {
        files.push(ResourceFile { path: "stream/minimap.gfx".into(), data });
    }
    if let Some(lua) = client_lua {
        files.push(ResourceFile { path: "client.lua".into(), data: lua.into_bytes() });
    }
    files
}

/// Notice accompagnant l'export.
///
/// Les masques ne peuvent pas être streamés tels quels : GTA V ne lit pas de
/// `.dds` isolé, il lit un dictionnaire de textures. Il reste donc une étape
/// manuelle d'injection dans `graphics.ytd`, et cette notice existe pour que
/// l'utilisateur ne se retrouve pas devant un dossier dont il ne sait que
/// faire.
pub fn readme(name: &str) -> String {
    format!(
        r#"# {name}

Généré par RockYouGFX.

## Installation

1. Copier ce dossier dans le `resources/` du serveur.
2. Ajouter dans le `server.cfg` :

```
ensure {name}
```

3. Vider le cache FiveM (`%localappdata%\FiveM\FiveM.app\data\cache`) : un
   asset déjà en cache masquerait le changement.

## Comment ça marche

La forme du radar vient de la **luminance** de deux textures, `radarmasksm`
(minimap courante) et `radarmasklg` (carte agrandie). Elles vivent normalement
dans `graphics.ytd`.

Mais `graphics.ytd` est un dictionnaire **de base** du jeu : en déposer une
version modifiée dans `stream/` n'a aucun effet, FiveM ne la relit pas. C'est
le piège de cette personnalisation.

Cette resource contourne le problème : elle livre son **propre** dictionnaire,
qui se streame normalement, et le `client.lua` demande au jeu d'y piocher les
deux masques via `AddReplaceTexture`. Aucun fichier du jeu n'est remplacé.

Le dossier `masks/` contient les mêmes masques en `.dds` : ils ne servent qu'au
contrôle visuel, la resource n'en a pas besoin.

## Ce qui n'est pas modifié

Le `minimap.gfx` reste celui du jeu. La bordure du radar et la boussole gardent
donc la forme vanilla, même si le radar change de forme.
"#
    )
}

/// Décrit ce qui manque pour que la resource soit cohérente en jeu.
pub fn warnings(files: &[ResourceFile]) -> Vec<String> {
    let has = |p: &str| files.iter().any(|f| f.path == p);
    let mut out = Vec::new();

    let has_mask = files.iter().any(|f| f.path.starts_with("stream/") && f.path.ends_with(".ytd"));
    if !has_mask {
        out.push(
            "Aucun dictionnaire de masques n'est livré : la forme du radar ne \
             changera pas. C'est la luminance de radarmasksm/radarmasklg qui la \
             définit, pas le .gfx."
                .into(),
        );
    }

    // L'absence de .gfx n'est un problème que si rien d'autre ne prend en
    // charge le HUD. Le script client couvre le masquage des barres, donc
    // avertir malgré sa présence serait un faux positif.
    if !has("stream/minimap.gfx") && !has("client.lua") {
        out.push(
            "Sans minimap.gfx ni script client, la bordure et les barres de \
             vie resteront calées sur la forme vanilla."
                .into(),
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_manifeste_ne_declare_pas_le_dossier_stream() {
        let m = fxmanifest("ma-minimap", Target::Legacy, false);
        assert!(m.contains("fx_version 'cerulean'"));
        assert!(m.contains("game 'gta5'"));
        assert!(m.contains("name 'ma-minimap'"));
        assert!(!m.contains("files {"), "stream/ ne doit pas être énuméré");
        assert!(!m.contains("client_script"), "aucun script client à déclarer ici");
    }

    /// Le script client, lui, ne serait pas chargé sans déclaration.
    #[test]
    fn le_manifeste_declare_le_script_client_quand_il_existe() {
        let m = fxmanifest("x", Target::Legacy, true);
        assert!(m.contains("client_script 'client.lua'"));
    }

    #[test]
    fn la_resource_place_les_assets_dans_stream() {
        let files = build_resource("x", Target::Enhanced, Some(("m".into(), vec![1])), Some(vec![2]), None);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["fxmanifest.lua", "stream/m.ytd", "stream/minimap.gfx"]);
    }

    #[test]
    fn une_resource_complete_ne_leve_aucun_avertissement() {
        let files = build_resource("x", Target::Legacy, Some(("m".into(), vec![1])), Some(vec![2]), None);
        assert!(warnings(&files).is_empty());
    }

    /// Le piège que l'outil existe pour éviter : ne livrer que le .gfx.
    #[test]
    fn le_gfx_seul_avertit_que_la_forme_ne_changera_pas() {
        let files = build_resource("x", Target::Legacy, None, Some(vec![2]), None);
        let w = warnings(&files);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("dictionnaire de masques"));
    }

    /// Le cas qui rend l'outil utilisable sans aucun asset Rockstar : masque +
    /// script client, sans .gfx. L'avertissement sur le .gfx serait ici un
    /// faux positif, puisque le Lua couvre le HUD.
    #[test]
    fn le_script_client_dispense_du_gfx() {
        let files = build_resource(
            "x",
            Target::Legacy,
            Some(("m".into(), vec![1])),
            None,
            Some("-- lua".into()),
        );
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["fxmanifest.lua", "stream/m.ytd", "client.lua"]);
        assert!(warnings(&files).is_empty());
    }
}
