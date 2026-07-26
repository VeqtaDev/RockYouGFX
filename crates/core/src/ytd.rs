//! Lecture et modification des dictionnaires de textures `.ytd` (RSC7).
//!
//! Un `.ytd` est une ressource RAGE : seize octets d'en-tête, puis le corps en
//! **deflate brut** — sans en-tête zlib. Le corps décompressé est la
//! concaténation de deux segments, système puis graphique, dont les tailles
//! sont encodées dans les drapeaux de l'en-tête.
//!
//! **Aucune pagination n'est reconstruite ici.** On remplace les octets de
//! pixels d'une texture existante, à dimensions et format identiques, donc à
//! taille identique. Les tailles de segments ne bougent pas, les drapeaux non
//! plus, et tous les pointeurs internes restent valides. C'est la même
//! philosophie que le patch des `.gfx` : éditer sans réécrire.

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{self, Read, Write};

const MAGIC: &[u8; 4] = b"RSC7";
const HEADER_LEN: usize = 16;

/// Décalages des champs dans une `grcTexture` 64 bits, relevés sur un
/// `graphics.ytd` réel.
mod field {
    pub const NAME_PTR: usize = 0x28;
    pub const WIDTH: usize = 0x50;
    pub const HEIGHT: usize = 0x52;
    pub const FORMAT: usize = 0x58;
    pub const LEVELS: usize = 0x5d;
    pub const DATA_PTR: usize = 0x70;
}

/// Segment désigné par le quartet de poids fort d'un pointeur RAGE.
const SEGMENT_SYSTEM: u64 = 5;
const SEGMENT_GRAPHICS: u64 = 6;

#[derive(Debug, Clone)]
pub struct Ytd {
    pub version: u32,
    pub system_flags: u32,
    pub graphics_flags: u32,
    /// Corps décompressé : segment système puis segment graphique.
    pub data: Vec<u8>,
    system_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureRef {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: [u8; 4],
    pub levels: u8,
    /// Décalage absolu des pixels dans `data`.
    pub data_offset: usize,
    /// Longueur du niveau 0.
    pub data_len: usize,
}

impl TextureRef {
    pub fn format_name(&self) -> String {
        String::from_utf8_lossy(&self.format).to_string()
    }

    pub fn is_dxt1(&self) -> bool {
        &self.format == b"DXT1"
    }
}

/// Taille d'un segment, décodée depuis ses drapeaux.
///
/// Les bits comptent des pages de tailles décroissantes, toutes multiples
/// d'une taille de base donnée par les quatre bits de poids faible.
pub fn size_from_flags(flags: u32) -> usize {
    let pages = ((flags >> 27) & 0x1)
        + (((flags >> 26) & 0x1) << 1)
        + (((flags >> 25) & 0x1) << 2)
        + (((flags >> 24) & 0x1) << 3)
        + (((flags >> 17) & 0x7f) << 4)
        + (((flags >> 11) & 0x3f) << 5)
        + (((flags >> 7) & 0xf) << 6)
        + (((flags >> 5) & 0x3) << 7)
        + (((flags >> 4) & 0x1) << 8);
    let base = 0x200usize << (flags & 0xf);
    base * pages as usize
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

impl Ytd {
    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC {
            return Err(invalid("signature RSC7 absente"));
        }
        let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let version = u32_at(4);
        let system_flags = u32_at(8);
        let graphics_flags = u32_at(12);

        let mut data = Vec::new();
        DeflateDecoder::new(&bytes[HEADER_LEN..]).read_to_end(&mut data)?;

        let system_size = size_from_flags(system_flags);
        let expected = system_size + size_from_flags(graphics_flags);
        if expected != data.len() {
            // Un écart signale un décodage de drapeaux erroné : mieux vaut
            // s'arrêter que patcher à l'aveugle des octets mal situés.
            return Err(invalid(format!(
                "tailles de segments incohérentes : {expected} attendus, {} décompressés",
                data.len()
            )));
        }

        Ok(Self { version, system_flags, graphics_flags, data, system_size })
    }

    pub fn system_size(&self) -> usize {
        self.system_size
    }

    /// Convertit un pointeur RAGE en décalage absolu dans `data`.
    fn resolve(&self, ptr: u64) -> Option<usize> {
        let offset = (ptr & 0x0fff_ffff) as usize;
        match (ptr >> 28) & 0xf {
            SEGMENT_SYSTEM => Some(offset),
            SEGMENT_GRAPHICS => Some(self.system_size + offset),
            _ => None,
        }
    }

    /// Localise une texture par son nom.
    ///
    /// La recherche part de la chaîne de caractères, puis remonte au
    /// descripteur par le pointeur qui la référence. C'est plus robuste que de
    /// parcourir la table du dictionnaire, dont la disposition varie selon les
    /// versions du jeu.
    pub fn find_texture(&self, name: &str) -> Option<TextureRef> {
        let needle: Vec<u8> = name.bytes().chain(std::iter::once(0)).collect();
        let sys = &self.data[..self.system_size];

        for start in 0..sys.len().saturating_sub(needle.len()) {
            if &sys[start..start + needle.len()] != needle.as_slice() {
                continue;
            }
            // Une sous-chaîne d'un nom plus long n'est pas le nom cherché.
            if start > 0 && sys[start - 1] != 0 {
                continue;
            }

            let ptr = ((SEGMENT_SYSTEM << 28) | start as u64).to_le_bytes();
            for base in 0..sys.len().saturating_sub(8) {
                if sys[base..base + 8] != ptr {
                    continue;
                }
                if base < field::NAME_PTR {
                    continue;
                }
                if let Some(t) = self.read_descriptor(base - field::NAME_PTR, name) {
                    return Some(t);
                }
            }
        }
        None
    }

    fn read_descriptor(&self, base: usize, name: &str) -> Option<TextureRef> {
        let d = &self.data;
        if base + field::DATA_PTR + 8 > d.len() {
            return None;
        }
        let u16_at = |o: usize| u16::from_le_bytes([d[base + o], d[base + o + 1]]) as u32;

        let width = u16_at(field::WIDTH);
        let height = u16_at(field::HEIGHT);
        if width == 0 || height == 0 {
            return None;
        }

        let format: [u8; 4] = d[base + field::FORMAT..base + field::FORMAT + 4].try_into().ok()?;
        let levels = d[base + field::LEVELS];
        let ptr = u64::from_le_bytes(
            d[base + field::DATA_PTR..base + field::DATA_PTR + 8].try_into().ok()?,
        );
        let data_offset = self.resolve(ptr)?;

        // Seul DXT1 est mesuré ici ; refuser le reste évite de calculer une
        // longueur fausse et d'écraser les octets d'une texture voisine.
        if &format != b"DXT1" {
            return None;
        }
        let data_len = (width as usize / 4) * (height as usize / 4) * 8;
        if data_offset + data_len > d.len() {
            return None;
        }

        Some(TextureRef {
            name: name.to_string(),
            width,
            height,
            format,
            levels,
            data_offset,
            data_len,
        })
    }

    /// Remplace les pixels d'une texture.
    ///
    /// La longueur doit être identique : c'est la condition qui permet de ne
    /// pas toucher à la pagination ni aux pointeurs.
    pub fn replace_texture_data(&mut self, tex: &TextureRef, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() != tex.data_len {
            return Err(invalid(format!(
                "{} : {} octets fournis, {} attendus",
                tex.name,
                bytes.len(),
                tex.data_len
            )));
        }
        self.data[tex.data_offset..tex.data_offset + tex.data_len].copy_from_slice(bytes);
        Ok(())
    }

    /// Sérialise le dictionnaire.
    ///
    /// Les drapeaux sont réémis tels quels : les tailles de segments n'ayant
    /// pas changé, les recalculer ne pourrait qu'introduire une erreur.
    pub fn write(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len() / 2);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.system_flags.to_le_bytes());
        out.extend_from_slice(&self.graphics_flags.to_le_bytes());

        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&self.data)?;
        out.extend_from_slice(&enc.finish()?);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les drapeaux d'un `graphics.ytd` réel, dont les tailles décodées
    /// tombaient exactement sur les 4 096 000 octets décompressés.
    #[test]
    fn le_decodage_des_drapeaux_reproduit_les_tailles_mesurees() {
        assert_eq!(size_from_flags(0x0006_0000), 24576);
        assert_eq!(size_from_flags(0xd81e_0044), 4_071_424);
        assert_eq!(size_from_flags(0x0006_0000) + size_from_flags(0xd81e_0044), 4_096_000);
    }

    #[test]
    fn un_fichier_sans_signature_est_rejete() {
        assert!(Ytd::parse(b"PAS UN YTD......").is_err());
        assert!(Ytd::parse(b"RSC7").is_err());
    }

    /// Une longueur différente casserait la pagination : le remplacement doit
    /// la refuser plutôt que de décaler tout ce qui suit.
    #[test]
    fn un_remplacement_de_taille_differente_est_refuse() {
        let mut ytd = Ytd {
            version: 13,
            system_flags: 0,
            graphics_flags: 0,
            data: vec![0u8; 1024],
            system_size: 512,
        };
        let tex = TextureRef {
            name: "test".into(),
            width: 16,
            height: 16,
            format: *b"DXT1",
            levels: 1,
            data_offset: 512,
            data_len: 128,
        };
        assert!(ytd.replace_texture_data(&tex, &vec![0u8; 64]).is_err());
        assert!(ytd.replace_texture_data(&tex, &vec![0u8; 128]).is_ok());
    }

    #[test]
    fn les_pointeurs_se_resolvent_selon_leur_segment() {
        let ytd = Ytd {
            version: 13,
            system_flags: 0,
            graphics_flags: 0,
            data: vec![0u8; 4096],
            system_size: 1024,
        };
        assert_eq!(ytd.resolve(0x5000_0100), Some(0x100));
        assert_eq!(ytd.resolve(0x6000_0100), Some(1024 + 0x100));
        // Un quartet de segment inconnu ne doit pas être interprété.
        assert_eq!(ytd.resolve(0x1000_0100), None);
    }
}
