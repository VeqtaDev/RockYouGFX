//! Rasterisation du contour en masque alpha.
//!
//! Le masque est ce qui découpe réellement la minimap dans GTA V : c'est son
//! canal alpha, une fois écrit dans `radarmasksm.dds` / `radarmasklg.dds`, qui
//! donne sa forme au radar.

use crate::shape::{outline, MinimapShape, Vec2};

/// Assez de segments pour qu'un coin reste lisse à 1024 px, sans faire exploser
/// le coût du champ de distance (qui est en O(pixels × segments)).
const SAMPLES_PER_CORNER: u32 = 32;

#[derive(Debug, Clone)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    /// Un octet d'alpha par pixel, en ligne par ligne depuis le haut.
    pub alpha: Vec<u8>,
}

impl Mask {
    pub fn at(&self, x: u32, y: u32) -> u8 {
        self.alpha[(y * self.width + x) as usize]
    }
}

/// Rasterise `shape` dans un masque `width` × `height`.
///
/// Les dimensions viennent de la texture vanilla qu'on remplace : elles ne sont
/// jamais devinées ici.
pub fn rasterize(shape: &MinimapShape, width: u32, height: u32) -> Mask {
    assert!(width > 0 && height > 0, "dimensions de masque nulles");

    // Le contour est en fractions ; on le passe en pixels avant toute mesure de
    // distance. Mesurer en fractions rendrait l'adoucissement anisotrope dès que
    // la texture n'est pas carrée.
    let pts: Vec<Vec2> = outline(shape, SAMPLES_PER_CORNER)
        .into_iter()
        .map(|p| Vec2 { x: p.x * width as f64, y: p.y * height as f64 })
        .collect();

    // Une bande d'au moins 1 px fait office d'anticrénelage : sans elle, un
    // masque à adoucissement nul sortirait en escalier.
    let band = (shape.feather * width.min(height) as f64).max(1.0);

    let mut alpha = vec![0u8; (width as usize) * (height as usize)];
    if pts.len() < 3 {
        return Mask { width, height, alpha };
    }

    for y in 0..height {
        for x in 0..width {
            let p = Vec2 { x: x as f64 + 0.5, y: y as f64 + 0.5 };
            let d = signed_distance(p, &pts);
            let a = (0.5 - d / band).clamp(0.0, 1.0);
            alpha[(y * width + x) as usize] = (a * 255.0).round() as u8;
        }
    }

    Mask { width, height, alpha }
}

/// Distance signée au polygone : négative à l'intérieur, positive à l'extérieur.
fn signed_distance(p: Vec2, pts: &[Vec2]) -> f64 {
    let mut min_sq = f64::INFINITY;
    let mut inside = false;

    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];

        let sq = point_segment_dist_sq(p, a, b);
        if sq < min_sq {
            min_sq = sq;
        }

        // Test de parité par lancer de rayon horizontal.
        if (a.y > p.y) != (b.y > p.y) {
            let t = (p.y - a.y) / (b.y - a.y);
            if p.x < a.x + t * (b.x - a.x) {
                inside = !inside;
            }
        }
    }

    let d = min_sq.sqrt();
    if inside {
        -d
    } else {
        d
    }
}

fn point_segment_dist_sq(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let wx = p.x - a.x;
    let wy = p.y - a.y;
    let len_sq = vx * vx + vy * vy;
    // Segment dégénéré : la distance se réduit à celle du point A.
    let t = if len_sq > 0.0 { ((wx * vx + wy * vy) / len_sq).clamp(0.0, 1.0) } else { 0.0 };
    let dx = wx - t * vx;
    let dy = wy - t * vy;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{Corners, Inset, Preset};

    fn full_box(preset: Preset, corners: f64, feather: f64) -> MinimapShape {
        MinimapShape {
            preset,
            corners: Corners { tl: corners, tr: corners, br: corners, bl: corners },
            inset: Inset { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
            smoothing: 0.0,
            feather,
            ..Default::default()
        }
    }

    #[test]
    fn le_rectangle_plein_est_opaque_au_centre_et_aux_coins() {
        let m = rasterize(&full_box(Preset::Vanilla, 0.0, 0.0), 64, 64);
        assert_eq!(m.at(32, 32), 255);
        assert_eq!(m.at(1, 1), 255, "un coin carré doit rester opaque");
    }

    /// Le test qui prouve que la forme est bien découpée : sur un disque, les
    /// coins de la texture doivent être totalement transparents.
    #[test]
    fn le_cercle_evide_les_coins() {
        let m = rasterize(&full_box(Preset::Circle, 1.0, 0.0), 128, 128);
        assert_eq!(m.at(64, 64), 255, "centre du disque non opaque");
        for (x, y) in [(1, 1), (126, 1), (1, 126), (126, 126)] {
            assert_eq!(m.at(x, y), 0, "coin ({x},{y}) non évidé");
        }
    }

    /// L'aire opaque d'un disque inscrit vaut PI/4 de celle de la texture.
    #[test]
    fn l_aire_du_disque_est_coherente() {
        let m = rasterize(&full_box(Preset::Circle, 1.0, 0.0), 256, 256);
        let sum: f64 = m.alpha.iter().map(|&a| a as f64 / 255.0).sum();
        let ratio = sum / (256.0 * 256.0);
        let expected = std::f64::consts::FRAC_PI_4;
        assert!((ratio - expected).abs() < 0.01, "aire {ratio} au lieu de {expected}");
    }

    /// Sans bande minimale d'un pixel, un adoucissement nul donnerait un bord
    /// en escalier : on vérifie qu'il existe bien des valeurs intermédiaires.
    #[test]
    fn le_bord_est_anticrenele_meme_sans_adoucissement() {
        let m = rasterize(&full_box(Preset::Circle, 1.0, 0.0), 128, 128);
        assert!(
            m.alpha.iter().any(|&a| a > 10 && a < 245),
            "aucun pixel de transition : le bord est en escalier"
        );
    }

    #[test]
    fn l_adoucissement_elargit_la_zone_de_transition() {
        let compte = |f: f64| {
            rasterize(&full_box(Preset::Circle, 1.0, f), 256, 256)
                .alpha
                .iter()
                .filter(|&&a| a > 10 && a < 245)
                .count()
        };
        assert!(compte(0.02) > compte(0.0) * 2, "l'adoucissement n'élargit pas le bord");
    }
}
