//! Lecture et écriture de DDS non compressés.
//!
//! On n'écrit délibérément pas de BC3/DXT5 : le masque est presque
//! entièrement composé d'aplats et d'un dégradé de bord fin, exactement le
//! motif sur lequel la compression par blocs laisse des artefacts visibles.
//! Un bord de minimap crénelé se voit immédiatement en jeu.

use std::io::{self, Read};

const MAGIC: u32 = 0x2053_4444; // « DDS »
const HEADER_SIZE: u32 = 124;
const PF_SIZE: u32 = 32;

const DDSD_CAPS: u32 = 0x1;
const DDSD_HEIGHT: u32 = 0x2;
const DDSD_WIDTH: u32 = 0x4;
const DDSD_PITCH: u32 = 0x8;
const DDSD_PIXELFORMAT: u32 = 0x1000;

const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;

const DDSCAPS_TEXTURE: u32 = 0x1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdsInfo {
    pub width: u32,
    pub height: u32,
    /// `None` pour un format non compressé, sinon le FourCC (`DXT5`, `DX10`…).
    pub four_cc: Option<[u8; 4]>,
    pub bit_count: u32,
}

/// Lit l'en-tête d'un DDS.
///
/// Sert à reprendre les dimensions exactes de `radarmasklg` / `radarmasksm`
/// vanilla : les coder en dur casserait au moindre changement de version du jeu.
pub fn read_info(mut r: impl Read) -> io::Result<DdsInfo> {
    let mut head = [0u8; 128];
    r.read_exact(&mut head)?;

    let u32_at = |o: usize| {
        u32::from_le_bytes([head[o], head[o + 1], head[o + 2], head[o + 3]])
    };

    if u32_at(0) != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "signature DDS absente"));
    }
    if u32_at(4) != HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "taille d'en-tête DDS invalide"));
    }

    let height = u32_at(12);
    let width = u32_at(16);

    // Le bloc DDS_PIXELFORMAT commence à l'octet 76 (4 de magic + 72 d'en-tête).
    let pf_flags = u32_at(80);
    let four_cc = if pf_flags & DDPF_FOURCC != 0 {
        Some([head[84], head[85], head[86], head[87]])
    } else {
        None
    };
    let bit_count = u32_at(88);

    Ok(DdsInfo { width, height, four_cc, bit_count })
}

/// Sérialise un masque alpha en DDS BGRA 32 bits non compressé.
///
/// Les canaux RGB reçoivent la même valeur que l'alpha. C'est délibéré : selon
/// la façon dont le masque est échantillonné (alpha ou luminance), les deux
/// lectures donnent alors le même résultat, au lieu de dépendre d'une hypothèse
/// invérifiable sans le jeu sous la main.
pub fn write_mask(mask: &crate::mask::Mask) -> Vec<u8> {
    let w = mask.width;
    let h = mask.height;
    let mut out = Vec::with_capacity(128 + (w as usize) * (h as usize) * 4);

    let mut push = |v: u32| out.extend_from_slice(&v.to_le_bytes());

    push(MAGIC);
    push(HEADER_SIZE);
    push(DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_PIXELFORMAT);
    push(h);
    push(w);
    push(w * 4); // pitch, en octets par ligne
    push(0); // depth
    push(0); // mipMapCount
    for _ in 0..11 {
        push(0); // reserved1
    }

    // DDS_PIXELFORMAT
    push(PF_SIZE);
    push(DDPF_RGB | DDPF_ALPHAPIXELS);
    push(0); // fourCC
    push(32); // bits par pixel
    push(0x00FF_0000); // masque R
    push(0x0000_FF00); // masque G
    push(0x0000_00FF); // masque B
    push(0xFF00_0000); // masque A

    push(DDSCAPS_TEXTURE);
    push(0); // caps2
    push(0); // caps3
    push(0); // caps4
    push(0); // reserved2

    debug_assert_eq!(out.len(), 128, "en-tête DDS de taille inattendue");

    for &a in &mask.alpha {
        out.extend_from_slice(&[a, a, a, a]); // B, G, R, A
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mask::rasterize;
    use crate::shape::{MinimapShape, Preset};

    fn sample_mask(w: u32, h: u32) -> crate::mask::Mask {
        rasterize(&MinimapShape { preset: Preset::Circle, ..Default::default() }, w, h)
    }

    #[test]
    fn l_en_tete_ecrit_se_relit() {
        let dds = write_mask(&sample_mask(64, 32));
        let info = read_info(&dds[..]).expect("en-tête illisible");
        assert_eq!(info.width, 64);
        assert_eq!(info.height, 32);
        assert_eq!(info.four_cc, None);
        assert_eq!(info.bit_count, 32);
    }

    #[test]
    fn la_taille_du_fichier_correspond_au_contenu() {
        let m = sample_mask(16, 8);
        assert_eq!(write_mask(&m).len(), 128 + 16 * 8 * 4);
    }

    /// L'alpha du masque doit se retrouver tel quel dans le fichier : c'est lui
    /// qui porte la forme.
    #[test]
    fn l_alpha_est_preserve_octet_pour_octet() {
        let m = sample_mask(32, 32);
        let dds = write_mask(&m);
        for (i, &a) in m.alpha.iter().enumerate() {
            assert_eq!(dds[128 + i * 4 + 3], a, "alpha divergent au pixel {i}");
        }
    }

    #[test]
    fn un_fichier_non_dds_est_rejete() {
        let junk = vec![0u8; 128];
        assert!(read_info(&junk[..]).is_err());
    }
}
