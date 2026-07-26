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
pub mod emitter;
pub mod gfx;
pub mod mask;
pub mod shape;

pub use shape::MinimapShape;

/// Les deux masques que GTA V utilise pour le radar, dans `graphics.ytd`.
///
/// `sm` est la minimap courante, `lg` la carte agrandie. Les deux dérivent de
/// la même forme mais n'ont pas les mêmes dimensions, d'où une définition
/// exprimée en fractions.
pub const MASK_TEXTURES: [&str; 2] = ["radarmasksm", "radarmasklg"];

/// Rasterise la forme aux dimensions données et sérialise le DDS.
pub fn mask_dds(shape: &MinimapShape, width: u32, height: u32) -> Vec<u8> {
    dds::write_mask(&mask::rasterize(shape, width, height))
}
