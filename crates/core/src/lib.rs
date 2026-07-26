//! Cœur métier de RockYouGFX.
//!
//! Volontairement sans dépendance à Tauri : `src-tauri` n'en est qu'un mince
//! wrapper. Le cœur se compile et se teste sans toolchain graphique, ce qui
//! permet de le couvrir en CI comme en conteneur headless.
//!
//! Le fil conducteur : une [`shape::MinimapShape`] unique alimente à la fois le
//! masque alpha (`mask` → `dds` → `.ytd`) et le Scaleform (`gfx`). Générer les
//! deux depuis la même définition est ce qui les garde cohérents — un masque
//! rond avec une bordure rectangulaire est le défaut le plus courant des
//! minimaps personnalisées.

pub mod dds;
pub mod dxt1;
pub mod emitter;
pub mod gfx;
pub mod lua;
pub mod mask;
pub mod shape;
pub mod vanilla;
pub mod ytd;

pub use shape::MinimapShape;

/// Les deux masques que GTA V utilise pour le radar, dans `graphics.ytd`.
///
/// `sm` est la minimap courante, `lg` la carte agrandie. Les deux dérivent de
/// la même forme mais n'ont pas les mêmes dimensions, d'où une définition
/// exprimée en fractions.
pub const MASK_TEXTURES: [&str; 2] = ["radarmasksm", "radarmasklg"];

/// Rasterise la forme aux dimensions données et sérialise le DDS.
///
/// Format d'échange, pratique pour inspecter le résultat dans un éditeur
/// d'images. Ce n'est **pas** ce qui entre dans le `.ytd` : voir [`mask_dxt1`].
pub fn mask_dds(shape: &MinimapShape, width: u32, height: u32) -> Vec<u8> {
    dds::write_mask(&mask::rasterize(shape, width, height))
}

/// Produit les octets DXT1 remplaçant une texture de masque vanilla.
///
/// La couverture rasterisée est ramenée à la luminance de crête mesurée sur le
/// vanilla, et non saturée à 255 : le jeu module la minimap par-dessus, si
/// bien qu'un masque plus clair éclaircirait le radar par rapport à l'origine.
pub fn mask_dxt1(shape: &MinimapShape, tex: &vanilla::MaskTexture) -> Vec<u8> {
    let m = mask::rasterize(&frame_into_vanilla(shape, tex), tex.width, tex.height);
    let peak = tex.peak_luminance as u32;
    let lum: Vec<u8> = m
        .alpha
        .iter()
        .map(|&a| ((a as u32 * peak + 127) / 255) as u8)
        .collect();
    dxt1::encode_gray(&lum, tex.width, tex.height)
}

/// Recadre une forme dans la zone qu'occupe le masque vanilla.
///
/// La texture est plus grande que le radar affiché : la forme vanilla n'en
/// occupe qu'une sous-région, et celle de `radarmasksm` n'est même pas
/// centrée. Étaler la forme sur toute la texture agrandirait et déplacerait le
/// radar en jeu.
///
/// L'utilisateur pilote donc la **forme**, le **cadrage** reste celui du jeu.
/// C'est aussi ce qui garde l'aperçu honnête : il montre ce que le joueur voit,
/// c'est-à-dire le contenu de cette sous-région, pas la texture entière.
fn frame_into_vanilla(shape: &MinimapShape, tex: &vanilla::MaskTexture) -> MinimapShape {
    let (vl, vr, vt, vb) = tex.inset_fractions();
    let span_x = 1.0 - vl - vr;
    let span_y = 1.0 - vt - vb;
    let i = shape.inset;

    MinimapShape {
        inset: shape::Inset {
            left: vl + i.left * span_x,
            right: vr + i.right * span_x,
            top: vt + i.top * span_y,
            bottom: vb + i.bottom * span_y,
        },
        // L'adoucissement n'est délibérément pas remis à l'échelle du cadre.
        // Il est déjà exprimé en fraction de la plus petite dimension de la
        // *texture* ; le remettre à l'échelle éloignait des proportions
        // vanilla au lieu de s'en rapprocher — mesuré à 33 px sur `sm` contre
        // 54 px sur `lg`, un rapport que la mise à l'échelle triplait.
        ..shape.clone()
    }
}

/// Applique une forme aux deux masques d'un `graphics.ytd`.
///
/// Le dictionnaire est modifié sur place : mêmes dimensions, même format,
/// donc même longueur, et aucune pagination à reconstruire.
pub fn patch_graphics_ytd(
    ytd: &mut ytd::Ytd,
    shape: &MinimapShape,
) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    for spec in vanilla::RADAR_MASKS {
        let tex = ytd
            .find_texture(spec.name)
            .ok_or_else(|| format!("texture {} introuvable dans le .ytd", spec.name))?;

        // Les dimensions du fichier font foi : celles de `vanilla` ne sont que
        // des mesures, susceptibles de bouger d'une version du jeu à l'autre.
        let actual = vanilla::MaskTexture {
            width: tex.width,
            height: tex.height,
            ..spec
        };
        if !tex.is_dxt1() {
            return Err(format!(
                "{} est en {}, or seul DXT1 est pris en charge",
                spec.name,
                tex.format_name()
            ));
        }

        let bytes = mask_dxt1(shape, &actual);
        ytd.replace_texture_data(&tex, &bytes).map_err(|e| e.to_string())?;
        done.push(format!("{} {}×{}", tex.name, tex.width, tex.height));
    }
    Ok(done)
}
