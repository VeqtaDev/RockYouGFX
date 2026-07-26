// L'exécutable portable est lancé au double-clic : sans ça, Windows ouvrirait
// une console noire derrière la fenêtre.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod updater;

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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequest {
    shape: MinimapShape,
    name: String,
    out_dir: String,
    enhanced: bool,
    /// Dimensions de `radarmasksm`, reprises de la texture vanilla.
    mask_sm: (u32, u32),
    /// Dimensions de `radarmasklg`.
    mask_lg: (u32, u32),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReport {
    root: String,
    files: Vec<String>,
    warnings: Vec<String>,
    limitations: Vec<String>,
    /// Nom réellement utilisé, une fois assaini.
    resource_name: String,
}

/// Un nom de resource FiveM ne peut pas contenir d'espace ni d'accent : le
/// `ensure` du server.cfg ne le retrouverait pas. On assainit plutôt que de
/// laisser l'utilisateur produire un dossier qui ne se chargera jamais.
fn sanitize_resource_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for c in raw.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "minimap".into()
    } else {
        trimmed
    }
}

/// Écrit la resource FiveM complète : masques, script client, manifeste, notice.
///
/// Le script client est dérivé de la forme, pas fourni par l'appelant : il
/// remplace le patch du `.gfx` pour tout ce que les natives savent faire, ce
/// qui permet de produire une resource sans aucun asset Rockstar.
///
/// Les masques atterrissent dans `masks/` et non dans `stream/` : ce sont des
/// `.dds` bruts, que GTA V ne sait pas lire isolément. Ils doivent d'abord
/// être injectés dans `graphics.ytd`. Les placer dans `stream/` laisserait
/// croire que la resource est complète.
#[tauri::command]
fn export_resource(req: ExportRequest) -> CmdResult<ExportReport> {
    for (label, (w, h)) in [("radarmasksm", req.mask_sm), ("radarmasklg", req.mask_lg)] {
        if w == 0 || h == 0 {
            return Err(format!("dimensions nulles pour {label}"));
        }
        // Au-delà, la rasterisation par champ de distance devient très lente
        // pour un masque qui n'a aucune raison d'être si grand.
        if w > 4096 || h > 4096 {
            return Err(format!("{label} : {w}×{h} dépasse la limite de 4096"));
        }
    }

    let name = sanitize_resource_name(&req.name);
    let target =
        if req.enhanced { emitter::Target::Enhanced } else { emitter::Target::Legacy };

    let client_lua = lua::client_script(&req.shape);
    let mut files = emitter::build_resource(&name, target, None, None, client_lua);
    files.push(emitter::ResourceFile {
        path: "LISEZ-MOI.md".into(),
        data: emitter::readme(&name).into_bytes(),
    });
    for (tex, (w, h)) in [("radarmasksm", req.mask_sm), ("radarmasklg", req.mask_lg)] {
        files.push(emitter::ResourceFile {
            path: format!("masks/{tex}.dds"),
            data: dds::write_mask(&mask::rasterize(&req.shape, w, h)),
        });
    }

    let root = PathBuf::from(&req.out_dir).join(&name);
    for f in &files {
        let dest = root.join(&f.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err("création du dossier", e))?;
        }
        std::fs::write(&dest, &f.data).map_err(|e| io_err("écriture du fichier", e))?;
    }

    Ok(ExportReport {
        root: root.to_string_lossy().into_owned(),
        files: files.iter().map(|f| f.path.clone()).collect(),
        // Une resource incomplète produit une minimap incohérente : on le dit
        // plutôt que de laisser l'utilisateur le découvrir en jeu.
        warnings: emitter::warnings(&files),
        // Ce que les natives Lua ne savent pas faire et qui exigerait un .gfx.
        limitations: lua::limitations(&req.shape),
        resource_name: name,
    })
}

/// L'application peut-elle se remplacer elle-même ?
///
/// Faux pour une installation NSIS, dont le désinstalleur tient le registre de
/// ce qui est installé, et dont le dossier n'est de toute façon pas accessible
/// en écriture sans élévation.
#[tauri::command]
fn can_self_update() -> bool {
    std::env::current_exe()
        .map(|exe| !updater::is_installed(&exe))
        .unwrap_or(false)
}

/// Remplace le binaire portable puis relance l'application.
///
/// Le téléchargement est fait par le frontend : `fetch` sait déjà suivre les
/// redirections de GitHub, et éviter un client HTTP côté Rust garde
/// l'exécutable portable léger.
#[tauri::command]
fn apply_portable_update(
    app: tauri::AppHandle,
    payload_b64: String,
    sha256: String,
) -> CmdResult<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload_b64.as_bytes())
        .map_err(|e| io_err("décodage du binaire téléchargé", e))?;

    let exe = updater::swap_binary(&bytes, &sha256).map_err(|e| e.to_string())?;

    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| io_err("relance de l'application", e))?;

    // Laisser Tauri fermer proprement : un exit brutal laisserait la WebView
    // derrière lui, et deux fenêtres se chevaucheraient le temps qu'elle meure.
    app.exit(0);
    Ok(())
}

fn main() {
    // Le binaire évincé par une mise à jour précédente n'était pas supprimable
    // tant qu'il tournait : c'est ici, au démarrage suivant, qu'il part.
    updater::cleanup_previous();

    tauri::Builder::default()
        // dialog : sélection du dossier de sortie et des DDS vanilla.
        // opener : ouvrir le dossier produit dans l'explorateur.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            apply_portable_update,
            can_self_update,
            dds_info,
            export_masks,
            export_resource,
            gfx_summary
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de RockYouGFX");
}

#[cfg(test)]
mod tests {
    use super::sanitize_resource_name;

    #[test]
    fn le_nom_de_resource_est_assaini() {
        assert_eq!(sanitize_resource_name("Ma Minimap"), "ma-minimap");
        assert_eq!(sanitize_resource_name("  Minimap Ronde  "), "minimap-ronde");
        // Les accents ne passent pas dans un nom de resource FiveM.
        assert_eq!(sanitize_resource_name("carré"), "carr");
        assert_eq!(sanitize_resource_name("a___b"), "a___b");
        assert_eq!(sanitize_resource_name("!!!"), "minimap");
        assert_eq!(sanitize_resource_name(""), "minimap");
    }

    /// Un tiret final produirait `ensure ma-minimap-`, qui ne résout pas.
    #[test]
    fn aucun_tiret_en_fin_de_nom() {
        assert_eq!(sanitize_resource_name("minimap !"), "minimap");
        assert_eq!(sanitize_resource_name("minimap - "), "minimap");
    }
}
