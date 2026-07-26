//! Conformité entre l'implémentation TypeScript et l'implémentation Rust.
//!
//! `src/lib/shape.ts` pilote l'aperçu à l'écran, `crates/core/src/shape.rs`
//! pilote le masque écrit dans le DDS. Si les deux divergent, l'utilisateur
//! voit une forme et le jeu en reçoit une autre — un écart silencieux, et le
//! genre de bug qu'on ne découvre qu'une fois en jeu.
//!
//! Les contours de référence sont régénérés par :
//!
//! ```sh
//! node scripts/export-reference-outlines.ts
//! ```

use rockyougfx_core::shape::{outline, MinimapShape};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    shape: MinimapShape,
    samples: u32,
    points: Vec<[f64; 2]>,
}

/// Les deux implémentations font les mêmes opérations dans le même ordre, mais
/// `powf` et les fonctions trigonométriques ne sont pas garanties au bit près
/// entre V8 et Rust. Cette tolérance reste très inférieure à un pixel, même
/// sur une texture de 4096.
const TOLERANCE: f64 = 1e-9;

#[test]
fn le_contour_rust_correspond_au_contour_typescript() {
    let raw = include_str!("fixtures/outlines.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("fixtures illisibles");
    assert!(!cases.is_empty(), "aucun cas de référence");

    for case in &cases {
        let got = outline(&case.shape, case.samples);

        assert_eq!(
            got.len(),
            case.points.len(),
            "[{}] nombre de points : {} côté Rust contre {} côté TypeScript",
            case.name,
            got.len(),
            case.points.len()
        );

        for (i, (r, ts)) in got.iter().zip(&case.points).enumerate() {
            let dx = (r.x - ts[0]).abs();
            let dy = (r.y - ts[1]).abs();
            assert!(
                dx < TOLERANCE && dy < TOLERANCE,
                "[{}] point {i} : Rust ({}, {}) contre TypeScript ({}, {})",
                case.name,
                r.x,
                r.y,
                ts[0],
                ts[1]
            );
        }
    }
}

/// Garde-fou : si un preset est ajouté côté TypeScript sans être exporté ici,
/// la conformité ne couvrirait plus toutes les familles de formes.
#[test]
fn toutes_les_familles_de_formes_sont_couvertes() {
    let raw = include_str!("fixtures/outlines.json");
    let cases: Vec<Case> = serde_json::from_str(raw).unwrap();
    let names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();

    for attendu in ["vanilla", "rounded", "circle", "squircle", "asymetrique", "hexagone"] {
        assert!(names.contains(&attendu), "cas de référence « {attendu} » manquant");
    }
}
