//! Géométrie et format des masques de radar vanilla.
//!
//! **Ces valeurs sont mesurées, pas devinées.** Elles proviennent du décodage
//! d'un `graphics.ytd` réel (GTA V build 2026-07) et corrigent plusieurs
//! hypothèses de conception initiales qui se sont révélées fausses :
//!
//! - le masque est en **DXT1**, pas DXT5 ;
//! - la forme vit dans la **luminance**, pas dans le canal alpha, qui est
//!   uniformément opaque ;
//! - l'intérieur n'est **pas blanc** mais gris (~181–189) ;
//! - la forme **ne remplit pas la texture** : elle occupe une sous-région
//!   entourée d'une large marge, et celle de `radarmasksm` n'est même pas
//!   centrée horizontalement ;
//! - le bord est adouci sur plusieurs dizaines de pixels, pas sur un ou deux.
//!
//! Aucun octet du jeu n'est reproduit ici : seulement des mesures.

/// Description d'une des deux textures de masque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskTexture {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// Bornes du plateau opaque, en pixels, inclusives.
    pub plateau: (u32, u32, u32, u32),
    /// Luminance du plateau. Le jeu module par-dessus : saturer à 255
    /// éclaircirait le radar par rapport au vanilla.
    pub peak_luminance: u8,
    /// Largeur approximative du dégradé de bord, en pixels.
    pub feather_px: u32,
}

impl MaskTexture {
    /// Marges du plateau en fractions `(gauche, droite, haut, bas)`.
    ///
    /// C'est la forme attendue par [`crate::shape::Inset`], ce qui permet de
    /// partir de la géométrie vanilla plutôt que d'une valeur arbitraire.
    pub fn inset_fractions(&self) -> (f64, f64, f64, f64) {
        let (x0, y0, x1, y1) = self.plateau;
        let w = self.width as f64;
        let h = self.height as f64;
        (
            x0 as f64 / w,
            1.0 - (x1 + 1) as f64 / w,
            y0 as f64 / h,
            1.0 - (y1 + 1) as f64 / h,
        )
    }

    /// Adoucissement en fraction de la plus petite dimension, unité utilisée
    /// par [`crate::shape::MinimapShape::feather`].
    pub fn feather_fraction(&self) -> f64 {
        self.feather_px as f64 / self.width.min(self.height) as f64
    }

    /// Taille des données DXT1 : blocs de 4×4 pixels sur 8 octets.
    pub fn dxt1_len(&self) -> usize {
        (self.width as usize / 4) * (self.height as usize / 4) * 8
    }
}

/// Minimap courante. Sa forme n'est pas centrée : elle est décalée vers la
/// gauche de la texture, la marge droite valant près du triple de la gauche.
pub const RADAR_MASK_SM: MaskTexture = MaskTexture {
    name: "radarmasksm",
    width: 512,
    height: 256,
    plateau: (67, 51, 317, 203),
    peak_luminance: 189,
    feather_px: 33,
};

/// Carte agrandie. Celle-ci est centrée.
pub const RADAR_MASK_LG: MaskTexture = MaskTexture {
    name: "radarmasklg",
    width: 512,
    height: 512,
    plateau: (63, 55, 448, 456),
    peak_luminance: 181,
    feather_px: 54,
};

pub const RADAR_MASKS: [MaskTexture; 2] = [RADAR_MASK_SM, RADAR_MASK_LG];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_tailles_dxt1_correspondent_aux_mesures() {
        assert_eq!(RADAR_MASK_SM.dxt1_len(), 65536);
        assert_eq!(RADAR_MASK_LG.dxt1_len(), 131072);
    }

    /// La forme de `radarmasksm` est décalée vers la gauche : une marge
    /// symétrique produirait un radar mal placé.
    #[test]
    fn le_masque_courant_n_est_pas_centre() {
        let (l, r, _, _) = RADAR_MASK_SM.inset_fractions();
        assert!(r > l * 2.0, "marge droite {r} pas nettement supérieure à la gauche {l}");
    }

    #[test]
    fn le_masque_agrandi_est_centre() {
        let (l, r, t, b) = RADAR_MASK_LG.inset_fractions();
        assert!((l - r).abs() < 1e-6, "marges horizontales asymétriques");
        assert!((t - b).abs() < 1e-6, "marges verticales asymétriques");
    }

    /// L'adoucissement vanilla est large : la valeur par défaut de l'éditeur
    /// était deux ordres de grandeur en dessous.
    #[test]
    fn l_adoucissement_vanilla_est_large() {
        assert!(RADAR_MASK_SM.feather_fraction() > 0.1);
        assert!(RADAR_MASK_LG.feather_fraction() > 0.1);
    }

    #[test]
    fn le_plateau_tient_dans_la_texture() {
        for m in RADAR_MASKS {
            let (x0, y0, x1, y1) = m.plateau;
            assert!(x1 < m.width && y1 < m.height, "{} déborde", m.name);
            assert!(x0 < x1 && y0 < y1, "{} : plateau vide", m.name);
        }
    }
}
