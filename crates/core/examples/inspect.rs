//! Inspecte un `.gfx` réel et vérifie que le round-trip est neutre.
//!
//! Les fichiers du jeu n'étant pas distribuables, cet exemple n'est pas un
//! test : il se lance à la main sur un fichier fourni par l'utilisateur.
//!
//! ```sh
//! cargo run -p rockyougfx-core --example inspect -- fixtures/game/minimap.gfx
//! ```

use rockyougfx_core::gfx::{tag, GfxFile};
use std::collections::BTreeMap;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage : inspect <fichier.gfx>");
            std::process::exit(2);
        }
    };

    let data = std::fs::read(&path).expect("lecture du fichier");
    let file = GfxFile::parse(&data).expect("analyse du .gfx");
    let tags = file.tags().expect("parcours des tags");

    println!("fichier    : {path}");
    println!("taille     : {} octets", data.len());
    println!("signature  : {:?} (compressé : {})", file.signature, file.signature.is_compressed());
    println!("version    : {}", file.version);
    println!("corps      : {} octets", file.body.len());
    println!("tags       : {}", tags.len());

    let mut par_code: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
    for t in &tags {
        let e = par_code.entry(t.code).or_insert((0, 0));
        e.0 += 1;
        e.1 += t.body_len;
    }

    println!("\n{:<6} {:>6} {:>12}  {}", "code", "nb", "octets", "nom");
    for (code, (n, bytes)) in &par_code {
        println!("{code:<6} {n:>6} {bytes:>12}  {}", nom_tag(*code));
    }

    let scaleform: usize = tags.iter().filter(|t| t.code >= tag::EXPORTER_INFO).count();
    println!("\ntags propriétaires Scaleform (>= 1000) : {scaleform}");

    // Le test qui compte : réécrire sans modifier doit rendre le fichier
    // d'origine, octet pour octet. C'est la garantie que le splice ne perd
    // rien, tags Scaleform inconnus compris.
    let rewritten = file.write().expect("réécriture");
    if rewritten == data {
        println!("\nround-trip neutre : IDENTIQUE ({} octets)", rewritten.len());
    } else {
        println!(
            "\nround-trip neutre : DIVERGENT ({} octets contre {})",
            rewritten.len(),
            data.len()
        );
        if let Some(i) = rewritten.iter().zip(&data).position(|(a, b)| a != b) {
            println!("  première divergence à l'octet {i}");
        }
        std::process::exit(1);
    }
}

fn nom_tag(code: u16) -> &'static str {
    match code {
        tag::END => "End",
        tag::SHOW_FRAME => "ShowFrame",
        tag::DEFINE_SHAPE => "DefineShape",
        tag::DEFINE_SHAPE2 => "DefineShape2",
        tag::PLACE_OBJECT2 => "PlaceObject2",
        tag::REMOVE_OBJECT2 => "RemoveObject2",
        tag::DEFINE_SHAPE3 => "DefineShape3",
        tag::DEFINE_SPRITE => "DefineSprite",
        tag::PLACE_OBJECT3 => "PlaceObject3",
        tag::SYMBOL_CLASS => "SymbolClass",
        tag::DEFINE_SHAPE4 => "DefineShape4",
        tag::EXPORTER_INFO => "Scaleform ExporterInfo",
        tag::DEFINE_EXTERNAL_IMAGE => "Scaleform DefineExternalImage",
        9 => "SetBackgroundColor",
        12 => "DoAction",
        34 => "DefineButton2",
        36 => "DefineBitsLossless2",
        37 => "DefineEditText",
        43 => "FrameLabel",
        56 => "ExportAssets",
        59 => "DoInitAction",
        69 => "FileAttributes",
        77 => "Metadata",
        86 => "DefineSceneAndFrameLabelData",
        _ if code >= 1000 => "Scaleform (propriétaire)",
        _ => "",
    }
}
