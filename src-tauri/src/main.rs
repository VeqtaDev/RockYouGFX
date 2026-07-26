// L'exécutable portable est lancé au double-clic : sans ça, Windows ouvrirait
// une console noire derrière la fenêtre.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rockyougfx_core::{dds, emitter, gfx, lua, mask, shape::MinimapShape};
use serde::Serialize;
use std::path::PathBuf;

/// Les commandes renvoient un message lisible plutôt qu'une erreur brute :
/// il est destiné à être affiché tel quel dans l'interface.
type CmdResult<T> = Result<T, String>;

fn io_err(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context} : {e}")
}

#[derive(Serialize)]
struct MaskExport {
    path: String,
    width: u32,
    height: u32,
}

/// Dimensions et format d'un DDS.
///
/// Sert à reprendre celles des masques vanilla : les coder en dur casserait au
/// moindre changement de version du jeu.
#[tauri::command]
fn dds_info(path: String) -> CmdResult<serde_json::Value> {
    let file = std::fs::File::open(&path).map_err(|e| io_err("ouverture du DDS", e))?;
    let info = dds::read_info(file).map_err(|e| io_err("lecture de l'en-tête DDS", e))?;
    Ok(serde_json::json!({
        "width": info.width,
        "height": info.height,
        "fourCC": info.four_cc.map(|c| String::from_utf8_lossy(&c).to_string()),
        "bitCount": info.bit_count,
    }))
}

/// Rasterise la forme aux deux résolutions et écrit les DDS.
///
/// Les dimensions sont fournies par l'appelant, qui les lit sur les textures
/// vanilla via [`dds_info`].
#[tauri::command]
fn export_masks(
    shape: MinimapShape,
    out_dir: String,
    sm: (u32, u32),
    lg: (u32, u32),
) -> CmdResult<Vec<MaskExport>> {
    let dir = PathBuf::from(&out_dir);
    std::fs::create_dir_all(&dir).map_err(|e| io_err("création du dossier de sortie", e))?;

    let mut out = Vec::new();
    for (name, (w, h)) in [("radarmasksm", sm), ("radarmasklg", lg)] {
        if w == 0 || h == 0 {
            return Err(format!("dimensions nulles pour {name}"));
        }
        let path = dir.join(format!("{name}.dds"));
        let bytes = dds::write_mask(&mask::rasterize(&shape, w, h));
        std::fs::write(&path, &bytes).map_err(|e| io_err("écriture du DDS", e))?;
        out.push(MaskExport {
            path: path.to_string_lossy().into_owned(),
            width: w,
            height: h,
        });
    }
    Ok(out)
}

/// Inventaire d'un `.gfx` : signature, version et flux de tags.
///
/// Première étape pour brancher le patch de bordure sur un vrai `minimap.gfx`,
/// dont la structure interne ne peut pas être devinée sans l'inspecter.
#[tauri::command]
fn gfx_summary(path: String) -> CmdResult<serde_json::Value> {
    let data = std::fs::read(&path).map_err(|e| io_err("lecture du .gfx", e))?;
    let file = gfx::GfxFile::parse(&data).map_err(|e| io_err("analyse du .gfx", e))?;
    let tags = file.tags().map_err(|e| io_err("parcours des tags", e))?;

    let listing: Vec<serde_json::Value> = tags
        .iter()
        .map(|t| {
            serde_json::json!({
                "code": t.code,
                "offset": t.offset,
                "length": t.body_len,
                "scaleform": t.code >= gfx::tag::EXPORTER_INFO,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "signature": format!("{:?}", file.signature),
        "compressed": file.signature.is_compressed(),
        "scaleform": file.signature.is_scaleform(),
        "version": file.version,
        "bodyLength": file.body.len(),
        "tagCount": tags.len(),
        "tags": listing,
    }))
}

/// Écrit l'arborescence de la resource FiveM.
///
/// Le script client est dérivé de la forme, pas fourni par l'appelant : il
/// remplace le patch du `.gfx` pour tout ce que les natives savent faire, ce
/// qui permet de produire une resource sans aucun asset Rockstar.
#[tauri::command]
fn write_resource(
    shape: MinimapShape,
    name: String,
    out_dir: String,
    enhanced: bool,
    graphics_ytd: Option<Vec<u8>>,
    minimap_gfx: Option<Vec<u8>>,
) -> CmdResult<serde_json::Value> {
    let target = if enhanced { emitter::Target::Enhanced } else { emitter::Target::Legacy };
    let client_lua = lua::client_script(&shape);
    let files = emitter::build_resource(&name, target, graphics_ytd, minimap_gfx, client_lua);
    let root = PathBuf::from(&out_dir).join(&name);

    for f in &files {
        let dest = root.join(&f.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err("création du dossier", e))?;
        }
        std::fs::write(&dest, &f.data).map_err(|e| io_err("écriture du fichier", e))?;
    }

    Ok(serde_json::json!({
        "root": root.to_string_lossy(),
        "files": files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
        // Une resource incomplète produit une minimap incohérente : on le dit
        // plutôt que de laisser l'utilisateur le découvrir en jeu.
        "warnings": emitter::warnings(&files),
        // Ce que les natives Lua ne savent pas faire et qui exigerait un .gfx.
        "limitations": lua::limitations(&shape),
    }))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dds_info,
            export_masks,
            gfx_summary,
            write_resource
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de RockYouGFX");
}
