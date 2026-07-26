//! Miroir strict de `src/lib/shape.ts`.
//!
//! Ce fichier et son homologue TypeScript doivent produire *exactement* le même
//! contour pour la même entrée : l'aperçu à l'écran et le masque écrit dans le
//! DDS en dépendent tous les deux. `tests/parity.rs` vérifie cette égalité
//! contre des contours de référence exportés depuis le TypeScript.

use serde::{Deserialize, Serialize};

/// Superellipse d'exposant 2 = cercle parfait.
pub const SMOOTHING_MIN_EXPONENT: f64 = 2.0;
/// ~ le squircle des icônes iOS.
pub const SMOOTHING_MAX_EXPONENT: f64 = 5.0;
pub const POLYGON_MIN_SIDES: u32 = 3;
pub const POLYGON_MAX_SIDES: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Vanilla,
    Rounded,
    Circle,
    Squircle,
    Polygon,
}

/// `Follow` = l'élément épouse la nouvelle forme au lieu de rester en position vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HudMode {
    Vanilla,
    Follow,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Corners {
    pub tl: f64,
    pub tr: f64,
    pub br: f64,
    pub bl: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Inset {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Polygon {
    pub sides: u32,
    pub rotation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Border {
    pub visible: bool,
    pub width: f64,
    pub color: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Hud {
    pub health: HudMode,
    pub armour: HudMode,
    pub compass: HudMode,
}

/// Toutes les grandeurs géométriques sont des **fractions**, jamais des pixels :
/// la même définition alimente `radarmasksm` et `radarmasklg`, qui n'ont pas
/// les mêmes dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapShape {
    pub preset: Preset,
    pub inset: Inset,
    pub corners: Corners,
    pub smoothing: f64,
    pub feather: f64,
    pub polygon: Polygon,
    pub border: Border,
    pub hud: Hud,
}

impl Default for MinimapShape {
    fn default() -> Self {
        Self {
            preset: Preset::Rounded,
            inset: Inset { top: 0.02, right: 0.02, bottom: 0.02, left: 0.02 },
            corners: Corners { tl: 0.22, tr: 0.22, br: 0.22, bl: 0.22 },
            smoothing: 0.6,
            feather: 0.004,
            polygon: Polygon { sides: 6, rotation: 0.0 },
            border: Border { visible: true, width: 2.0, color: "#000000".into() },
            hud: Hud {
                health: HudMode::Follow,
                armour: HudMode::Follow,
                compass: HudMode::Vanilla,
            },
        }
    }
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn clamp01(v: f64) -> f64 {
    clamp(v, 0.0, 1.0)
}

struct Box2 {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn box_of(shape: &MinimapShape) -> Box2 {
    // Des marges opposées qui se croisent donneraient une boîte inversée.
    let x0 = clamp01(shape.inset.left);
    let x1 = clamp(1.0 - shape.inset.right, x0, 1.0);
    let y0 = clamp01(shape.inset.top);
    let y1 = clamp(1.0 - shape.inset.bottom, y0, 1.0);
    Box2 { x0, y0, x1, y1 }
}

/// Rayons en unités absolues, avec la mise à l'échelle de CSS `border-radius` :
/// si deux rayons voisins dépassent la longueur de leur arête commune, tous les
/// rayons sont réduits du même facteur. Sans ça, les coins se chevauchent et le
/// contour se replie sur lui-même.
fn resolve_radii(shape: &MinimapShape, b: &Box2) -> Corners {
    let w = b.x1 - b.x0;
    let h = b.y1 - b.y0;
    let max_r = w.min(h) / 2.0;

    let mut tl = clamp01(shape.corners.tl) * max_r;
    let mut tr = clamp01(shape.corners.tr) * max_r;
    let mut br = clamp01(shape.corners.br) * max_r;
    let mut bl = clamp01(shape.corners.bl) * max_r;

    let ratio = |span: f64, sum: f64| if sum > 0.0 { span / sum } else { f64::INFINITY };
    let f = 1.0f64
        .min(ratio(w, tl + tr))
        .min(ratio(w, br + bl))
        .min(ratio(h, tl + bl))
        .min(ratio(h, tr + br));

    if f < 1.0 {
        tl *= f;
        tr *= f;
        br *= f;
        bl *= f;
    }
    Corners { tl, tr, br, bl }
}

fn exponent(smoothing: f64) -> f64 {
    SMOOTHING_MIN_EXPONENT
        + clamp01(smoothing) * (SMOOTHING_MAX_EXPONENT - SMOOTHING_MIN_EXPONENT)
}

/// Échantillonne un quadrant de superellipse |x/r|^n + |y/r|^n = 1.
///
/// n = 2 redonne exactement le cercle, donc `smoothing = 0` produit un coin
/// arrondi classique et il n'y a pas deux chemins de code à maintenir.
fn superellipse_quadrant(n: f64, steps: u32) -> Vec<Vec2> {
    let p = 2.0 / n;
    (0..=steps)
        .map(|i| {
            let t = (i as f64 / steps as f64) * std::f64::consts::FRAC_PI_2;
            Vec2 { x: t.cos().powf(p), y: t.sin().powf(p) }
        })
        .collect()
}

/// Contour de la forme, en coordonnées normalisées [0,1]² (y vers le bas),
/// dans le sens horaire.
pub fn outline(shape: &MinimapShape, samples_per_corner: u32) -> Vec<Vec2> {
    if shape.preset == Preset::Polygon {
        return polygon_outline(shape, samples_per_corner);
    }

    let b = box_of(shape);
    let r = resolve_radii(shape, &b);
    let q = superellipse_quadrant(exponent(shape.smoothing), samples_per_corner);
    let mut pts = Vec::with_capacity(q.len() * 4);

    // Coin haut-gauche : arête gauche -> arête haute.
    let (cx, cy) = (b.x0 + r.tl, b.y0 + r.tl);
    if r.tl == 0.0 {
        pts.push(Vec2 { x: b.x0, y: b.y0 });
    } else {
        pts.extend(q.iter().map(|s| Vec2 { x: cx - r.tl * s.x, y: cy - r.tl * s.y }));
    }

    // Coin haut-droit : arête haute -> arête droite.
    let (cx, cy) = (b.x1 - r.tr, b.y0 + r.tr);
    if r.tr == 0.0 {
        pts.push(Vec2 { x: b.x1, y: b.y0 });
    } else {
        pts.extend(q.iter().map(|s| Vec2 { x: cx + r.tr * s.y, y: cy - r.tr * s.x }));
    }

    // Coin bas-droit : arête droite -> arête basse.
    let (cx, cy) = (b.x1 - r.br, b.y1 - r.br);
    if r.br == 0.0 {
        pts.push(Vec2 { x: b.x1, y: b.y1 });
    } else {
        pts.extend(q.iter().map(|s| Vec2 { x: cx + r.br * s.x, y: cy + r.br * s.y }));
    }

    // Coin bas-gauche : arête basse -> arête gauche.
    let (cx, cy) = (b.x0 + r.bl, b.y1 - r.bl);
    if r.bl == 0.0 {
        pts.push(Vec2 { x: b.x0, y: b.y1 });
    } else {
        pts.extend(q.iter().map(|s| Vec2 { x: cx - r.bl * s.y, y: cy + r.bl * s.x }));
    }

    pts
}

/// N-gone inscrit dans la boîte, coins adoucis par un arc de cercle.
///
/// Le lissage superellipse ne s'applique pas ici : il est défini pour un coin
/// à 90°, pas pour un angle quelconque.
fn polygon_outline(shape: &MinimapShape, samples_per_corner: u32) -> Vec<Vec2> {
    let b = box_of(shape);
    let cx = (b.x0 + b.x1) / 2.0;
    let cy = (b.y0 + b.y1) / 2.0;
    let rx = (b.x1 - b.x0) / 2.0;
    let ry = (b.y1 - b.y0) / 2.0;

    let sides = shape.polygon.sides.clamp(POLYGON_MIN_SIDES, POLYGON_MAX_SIDES);
    let n = sides as usize;

    let verts: Vec<Vec2> = (0..n)
        .map(|i| {
            // -PI/2 place un sommet en haut, ce qui correspond à l'attente visuelle.
            let a = shape.polygon.rotation + (i as f64 / sides as f64) * std::f64::consts::TAU
                - std::f64::consts::FRAC_PI_2;
            Vec2 { x: cx + rx * a.cos(), y: cy + ry * a.sin() }
        })
        .collect();

    let radius_frac =
        clamp01((shape.corners.tl + shape.corners.tr + shape.corners.br + shape.corners.bl) / 4.0);
    if radius_frac <= 0.0 {
        return verts;
    }

    let mut out = Vec::new();
    for i in 0..n {
        let v = verts[i];
        let prev = verts[(i + n - 1) % n];
        let next = verts[(i + 1) % n];

        let da = norm(sub(prev, v));
        let db = norm(sub(next, v));
        let theta = clamp(da.x * db.x + da.y * db.y, -1.0, 1.0).acos();

        // Sommets colinéaires : pas de coin à arrondir.
        if theta < 1e-6 || std::f64::consts::PI - theta < 1e-6 {
            out.push(v);
            continue;
        }

        let max_tan = (len(sub(prev, v)) / 2.0).min(len(sub(next, v)) / 2.0);
        let tan = radius_frac * max_tan;
        // Relation tangente/rayon dans un coin d'angle intérieur theta.
        let r = tan * (theta / 2.0).tan();

        let p1 = Vec2 { x: v.x + da.x * tan, y: v.y + da.y * tan };
        let p2 = Vec2 { x: v.x + db.x * tan, y: v.y + db.y * tan };
        let bis = norm(Vec2 { x: da.x + db.x, y: da.y + db.y });
        let dist = r / (theta / 2.0).sin();
        let c = Vec2 { x: v.x + bis.x * dist, y: v.y + bis.y * dist };

        let a1 = (p1.y - c.y).atan2(p1.x - c.x);
        let a2 = (p2.y - c.y).atan2(p2.x - c.x);
        let mut sweep = a2 - a1;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }

        for s in 0..=samples_per_corner {
            let a = a1 + sweep * (s as f64 / samples_per_corner as f64);
            out.push(Vec2 { x: c.x + r * a.cos(), y: c.y + r * a.sin() });
        }
    }
    out
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 { x: a.x - b.x, y: a.y - b.y }
}

fn len(a: Vec2) -> f64 {
    a.x.hypot(a.y)
}

fn norm(a: Vec2) -> Vec2 {
    let l = len(a);
    if l == 0.0 {
        Vec2 { x: 0.0, y: 0.0 }
    } else {
        Vec2 { x: a.x / l, y: a.y / l }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape_with(preset: Preset, corners: f64, smoothing: f64) -> MinimapShape {
        MinimapShape {
            preset,
            corners: Corners { tl: corners, tr: corners, br: corners, bl: corners },
            smoothing,
            inset: Inset { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 },
            ..Default::default()
        }
    }

    #[test]
    fn contour_ferme_reste_dans_la_boite() {
        for preset in [Preset::Vanilla, Preset::Rounded, Preset::Circle, Preset::Squircle] {
            let s = shape_with(preset, 0.5, 0.5);
            let pts = outline(&s, 32);
            assert!(pts.len() >= 4, "{preset:?} : contour vide");
            for p in &pts {
                assert!(
                    (-1e-9..=1.0 + 1e-9).contains(&p.x) && (-1e-9..=1.0 + 1e-9).contains(&p.y),
                    "{preset:?} : point hors boîte {p:?}"
                );
            }
        }
    }

    /// Le point clé de la conception : `smoothing = 0` doit redonner un cercle
    /// exact, sinon les deux familles de coins divergent silencieusement.
    #[test]
    fn lissage_nul_donne_un_arc_de_cercle_exact() {
        let s = shape_with(Preset::Circle, 1.0, 0.0);
        let pts = outline(&s, 128);
        // Boîte carrée, rayon plein : le contour est le cercle inscrit.
        for p in &pts {
            let d = ((p.x - 0.5).powi(2) + (p.y - 0.5).powi(2)).sqrt();
            assert!((d - 0.5).abs() < 1e-9, "point à {d} du centre au lieu de 0.5");
        }
    }

    /// Un lissage croissant doit gonfler le coin vers l'extérieur : c'est ce qui
    /// distingue visuellement un squircle d'un simple arrondi.
    #[test]
    fn le_lissage_gonfle_le_coin() {
        let aire = |sm: f64| {
            let pts = outline(&shape_with(Preset::Circle, 1.0, sm), 128);
            let mut a = 0.0;
            for i in 0..pts.len() {
                let p = pts[i];
                let q = pts[(i + 1) % pts.len()];
                a += p.x * q.y - q.x * p.y;
            }
            (a / 2.0).abs()
        };
        assert!(aire(1.0) > aire(0.5), "squircle pas plus large qu'un arrondi moyen");
        assert!(aire(0.5) > aire(0.0), "arrondi moyen pas plus large qu'un cercle");
    }

    /// Sans la mise à l'échelle façon CSS, deux rayons voisins trop grands
    /// replieraient le contour sur lui-même.
    #[test]
    fn les_rayons_voisins_ne_se_chevauchent_pas() {
        let mut s = shape_with(Preset::Rounded, 1.0, 0.0);
        s.inset = Inset { top: 0.0, right: 0.0, bottom: 0.4, left: 0.0 };
        let r = resolve_radii(&s, &box_of(&s));
        let b = box_of(&s);
        assert!(r.tl + r.tr <= (b.x1 - b.x0) + 1e-9);
        assert!(r.tl + r.bl <= (b.y1 - b.y0) + 1e-9);
    }

    #[test]
    fn le_polygone_produit_le_bon_nombre_de_sommets() {
        let mut s = shape_with(Preset::Polygon, 0.0, 0.0);
        s.polygon = Polygon { sides: 6, rotation: 0.0 };
        assert_eq!(outline(&s, 16).len(), 6);
    }
}
