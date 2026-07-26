//! Patche un `graphics.ytd` réel et vérifie le résultat.
//!
//! Les fichiers du jeu n'étant pas distribuables, ce n'est pas un test : il se
//! lance à la main sur un fichier fourni par l'utilisateur.
//!
//! ```sh
//! cargo run -p rockyougfx-core --example patch_ytd -- \
//!     fixtures/game/graphics.ytd /tmp/graphics-patche.ytd
//! ```

use rockyougfx_core::shape::{Corners, MinimapShape, Preset};
use rockyougfx_core::{dxt1, patch_graphics_ytd, vanilla, ytd::Ytd};

fn main() {
    let mut args = std::env::args().skip(1);
    let (input, output) = match (args.next(), args.next()) {
        (Some(i), Some(o)) => (i, o),
        _ => {
            eprintln!("usage : patch_ytd <entrée.ytd> <sortie.ytd>");
            std::process::exit(2);
        }
    };

    let raw = std::fs::read(&input).expect("lecture du .ytd");
    let mut y = Ytd::parse(&raw).expect("analyse du .ytd");

    println!("entrée        : {input} ({} octets)", raw.len());
    println!("version       : {}", y.version);
    println!("décompressé   : {} octets", y.data.len());
    println!("segment sys.  : {} octets", y.system_size());

    println!("\n--- textures de masque telles qu'elles sont dans le fichier ---");
    for spec in vanilla::RADAR_MASKS {
        match y.find_texture(spec.name) {
            Some(t) => {
                let mesure = t.width == spec.width && t.height == spec.height;
                println!(
                    "{:<12} {}×{} {} niveaux={} pixels@0x{:x} ({} octets){}",
                    t.name,
                    t.width,
                    t.height,
                    t.format_name(),
                    t.levels,
                    t.data_offset,
                    t.data_len,
                    if mesure { "" } else { "   [dimensions différentes des mesures]" }
                );
            }
            None => {
                eprintln!("{:<12} INTROUVABLE", spec.name);
                std::process::exit(1);
            }
        }
    }

    // Une forme franchement différente du vanilla : si le patch fonctionne,
    // la relecture doit montrer un disque, pas un rectangle.
    let shape = MinimapShape {
        preset: Preset::Circle,
        corners: Corners { tl: 1.0, tr: 1.0, br: 1.0, bl: 1.0 },
        smoothing: 0.0,
        feather: 0.02,
        ..Default::default()
    };

    println!("\n--- patch ---");
    match patch_graphics_ytd(&mut y, &shape) {
        Ok(done) => {
            for d in done {
                println!("remplacé : {d}");
            }
        }
        Err(e) => {
            eprintln!("échec : {e}");
            std::process::exit(1);
        }
    }

    let out = y.write().expect("réécriture");
    std::fs::write(&output, &out).expect("écriture du fichier");
    println!("\nsortie        : {output} ({} octets)", out.len());

    // Relecture : le fichier produit doit se réanalyser, et les pixels relus
    // doivent être ceux qu'on vient d'écrire.
    println!("\n--- vérification par relecture ---");
    let reread = std::fs::read(&output).expect("relecture");
    let y2 = Ytd::parse(&reread).expect("le fichier produit ne se réanalyse pas");

    if y2.data.len() != y.data.len() {
        eprintln!(
            "taille décompressée divergente : {} contre {}",
            y2.data.len(),
            y.data.len()
        );
        std::process::exit(1);
    }
    println!("taille décompressée identique : {} octets", y2.data.len());

    for spec in vanilla::RADAR_MASKS {
        let t = y2.find_texture(spec.name).expect("texture perdue après réécriture");
        let px = &y2.data[t.data_offset..t.data_offset + t.data_len];
        let lum = dxt1::decode_gray(px, t.width, t.height);

        // Sur un disque, le centre est plein et les coins sont vides.
        let center = lum[(t.height as usize / 2) * t.width as usize + t.width as usize / 2];
        let corner = lum[0];
        let plateau = lum.iter().copied().max().unwrap();
        println!(
            "{:<12} centre={} coin={} crête={} (attendu ~{})",
            t.name, center, corner, plateau, spec.peak_luminance
        );

        assert!(center > 100, "{} : centre du disque vide", t.name);
        assert!(corner < 20, "{} : coin non évidé", t.name);
    }

    println!("\nle .ytd patché se relit correctement et porte bien la nouvelle forme.");
}
