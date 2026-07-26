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
/// `graphics_ytd` porte les masques réécrits ; `minimap_gfx` le Scaleform
/// patché. Les deux sont optionnels, mais n'en fournir qu'un donne une minimap
/// incohérente — masque sans bordure assortie, ou bordure sans changement de
/// forme.
pub fn build_resource(
    name: &str,
    target: Target,
    graphics_ytd: Option<Vec<u8>>,
    minimap_gfx: Option<Vec<u8>>,
    client_lua: Option<String>,
) -> Vec<ResourceFile> {
    let mut files = vec![ResourceFile {
        path: "fxmanifest.lua".into(),
        data: fxmanifest(name, target, client_lua.is_some()).into_bytes(),
    }];

    if let Some(data) = graphics_ytd {
        files.push(ResourceFile { path: "stream/graphics.ytd".into(), data });
    }
    if let Some(data) = minimap_gfx {
        files.push(ResourceFile { path: "stream/minimap.gfx".into(), data });
    }
    if let Some(lua) = client_lua {
        files.push(ResourceFile { path: "client.lua".into(), data: lua.into_bytes() });
    }
    files
}

/// Décrit ce qui manque pour que la resource soit cohérente en jeu.
pub fn warnings(files: &[ResourceFile]) -> Vec<String> {
    let has = |p: &str| files.iter().any(|f| f.path == p);
    let mut out = Vec::new();

    if !has("stream/graphics.ytd") {
        out.push(
            "Sans graphics.ytd, la forme du radar ne changera pas : c'est le \
             masque alpha qui la définit, pas le .gfx."
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
        let files = build_resource("x", Target::Enhanced, Some(vec![1]), Some(vec![2]), None);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["fxmanifest.lua", "stream/graphics.ytd", "stream/minimap.gfx"]);
    }

    #[test]
    fn une_resource_complete_ne_leve_aucun_avertissement() {
        let files = build_resource("x", Target::Legacy, Some(vec![1]), Some(vec![2]), None);
        assert!(warnings(&files).is_empty());
    }

    /// Le piège que l'outil existe pour éviter : ne livrer que le .gfx.
    #[test]
    fn le_gfx_seul_avertit_que_la_forme_ne_changera_pas() {
        let files = build_resource("x", Target::Legacy, None, Some(vec![2]), None);
        let w = warnings(&files);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("masque alpha"));
    }

    /// Le cas qui rend l'outil utilisable sans aucun asset Rockstar : masque +
    /// script client, sans .gfx. L'avertissement sur le .gfx serait ici un
    /// faux positif, puisque le Lua couvre le HUD.
    #[test]
    fn le_script_client_dispense_du_gfx() {
        let files = build_resource(
            "x",
            Target::Legacy,
            Some(vec![1]),
            None,
            Some("-- lua".into()),
        );
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["fxmanifest.lua", "stream/graphics.ytd", "client.lua"]);
        assert!(warnings(&files).is_empty());
    }
}
