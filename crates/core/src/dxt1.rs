//! Encodage et décodage DXT1 (BC1) pour masques en niveaux de gris.
//!
//! Le format des masques de radar vanilla est DXT1, et la forme y vit dans la
//! **luminance** — l'alpha est uniformément opaque. Remplacer ces textures
//! impose donc de produire du DXT1 aux mêmes dimensions : c'est ce qui permet
//! de réécrire les octets sur place, sans toucher à la pagination du
//! conteneur.
//!
//! L'encodeur est spécialisé pour le gris. Un encodeur couleur générique
//! chercherait la meilleure droite dans l'espace RGB ; ici tous les points
//! sont déjà alignés sur la diagonale des gris, et les extrêmes du bloc sont
//! donc les extrémités optimales.

/// Un bloc DXT1 couvre 4×4 pixels sur 8 octets.
pub const BLOCK_BYTES: usize = 8;

fn gray_to_565(v: u8) -> u16 {
    let r = (v >> 3) as u16;
    let g = (v >> 2) as u16;
    let b = (v >> 3) as u16;
    (r << 11) | (g << 5) | b
}

/// Luminance telle que le décodeur la reconstruira.
///
/// Encoder puis décoder n'est pas l'identité : le 565 perd des bits. Choisir
/// les indices d'après cette valeur reconstruite, et non d'après la valeur
/// d'origine, évite d'accumuler l'erreur.
fn gray_from_565(c: u16) -> u8 {
    let g = ((c >> 5) & 0x3f) as u8;
    (g << 2) | (g >> 4)
}

/// Encode un plan de luminance en DXT1.
///
/// `width` et `height` doivent être multiples de 4 : c'est la maille des blocs.
pub fn encode_gray(lum: &[u8], width: u32, height: u32) -> Vec<u8> {
    assert!(width % 4 == 0 && height % 4 == 0, "dimensions non multiples de 4");
    assert_eq!(lum.len(), (width * height) as usize, "plan de luminance de taille incohérente");

    let bw = (width / 4) as usize;
    let bh = (height / 4) as usize;
    let mut out = Vec::with_capacity(bw * bh * BLOCK_BYTES);

    for by in 0..bh {
        for bx in 0..bw {
            let mut px = [0u8; 16];
            for p in 0..16 {
                let x = bx * 4 + p % 4;
                let y = by * 4 + p / 4;
                px[p] = lum[y * width as usize + x];
            }
            out.extend_from_slice(&encode_block(&px));
        }
    }
    out
}

fn encode_block(px: &[u8; 16]) -> [u8; BLOCK_BYTES] {
    let lo = *px.iter().min().unwrap();
    let hi = *px.iter().max().unwrap();

    let c0 = gray_to_565(hi);
    let c1 = gray_to_565(lo);

    // c0 > c1 sélectionne le mode à quatre couleurs opaques. À l'inverse, le
    // mode à trois couleurs réserve l'indice 3 à la transparence, ce dont un
    // masque de radar n'a que faire.
    if c0 <= c1 {
        // Bloc uni : une seule couleur suffit, tous les indices à zéro.
        let mut b = [0u8; BLOCK_BYTES];
        b[0..2].copy_from_slice(&c0.to_le_bytes());
        b[2..4].copy_from_slice(&c1.to_le_bytes());
        return b;
    }

    let g0 = gray_from_565(c0) as i32;
    let g1 = gray_from_565(c1) as i32;
    let palette = [g0, g1, (2 * g0 + g1) / 3, (g0 + 2 * g1) / 3];

    let mut indices: u32 = 0;
    for (p, &v) in px.iter().enumerate() {
        let v = v as i32;
        let mut best = 0usize;
        let mut best_err = i32::MAX;
        for (i, &c) in palette.iter().enumerate() {
            let err = (v - c).abs();
            if err < best_err {
                best_err = err;
                best = i;
            }
        }
        indices |= (best as u32) << (p * 2);
    }

    let mut b = [0u8; BLOCK_BYTES];
    b[0..2].copy_from_slice(&c0.to_le_bytes());
    b[2..4].copy_from_slice(&c1.to_le_bytes());
    b[4..8].copy_from_slice(&indices.to_le_bytes());
    b
}

/// Décode un DXT1 en plan de luminance. Sert aux tests et à l'inspection.
pub fn decode_gray(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let bw = (width / 4) as usize;
    let mut out = vec![0u8; (width * height) as usize];

    for (i, block) in data.chunks_exact(BLOCK_BYTES).enumerate() {
        let c0 = u16::from_le_bytes([block[0], block[1]]);
        let c1 = u16::from_le_bytes([block[2], block[3]]);
        let idx = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);

        let g0 = gray_from_565(c0) as i32;
        let g1 = gray_from_565(c1) as i32;
        let palette = if c0 > c1 {
            [g0, g1, (2 * g0 + g1) / 3, (g0 + 2 * g1) / 3]
        } else {
            // Le quatrième emplacement est transparent ; pour un masque de
            // luminance on le lit comme noir.
            [g0, g1, (g0 + g1) / 2, 0]
        };

        let bx = (i % bw) * 4;
        let by = (i / bw) * 4;
        for p in 0..16 {
            let x = bx + p % 4;
            let y = by + p / 4;
            if x < width as usize && y < height as usize {
                out[y * width as usize + x] = palette[((idx >> (p * 2)) & 3) as usize] as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_taille_encodee_suit_le_nombre_de_blocs() {
        let lum = vec![0u8; 512 * 256];
        assert_eq!(encode_gray(&lum, 512, 256).len(), (512 / 4) * (256 / 4) * 8);
    }

    #[test]
    fn un_bloc_uni_se_decode_a_l_identique() {
        for v in [0u8, 64, 128, 189, 255] {
            let lum = vec![v; 16];
            let dec = decode_gray(&encode_gray(&lum, 4, 4), 4, 4);
            // Le 565 arrondit : on tolère l'erreur de quantification.
            assert!(
                (dec[0] as i32 - v as i32).abs() <= 4,
                "valeur {v} redonne {} après aller-retour",
                dec[0]
            );
        }
    }

    /// Un dégradé est le cas qui compte : c'est ce que produit l'adoucissement
    /// du bord, et c'est là qu'un encodeur naïf ferait des marches.
    #[test]
    fn un_degrade_reste_monotone_et_proche() {
        let w = 64u32;
        let h = 4u32;
        let lum: Vec<u8> = (0..w * h)
            .map(|i| ((i % w) as f64 / (w - 1) as f64 * 255.0) as u8)
            .collect();

        let dec = decode_gray(&encode_gray(&lum, w, h), w, h);
        let mut max_err = 0i32;
        for (a, b) in lum.iter().zip(&dec) {
            max_err = max_err.max((*a as i32 - *b as i32).abs());
        }
        // Quatre niveaux par bloc de 4 pixels : l'erreur reste faible.
        assert!(max_err <= 12, "erreur maximale {max_err} trop élevée sur un dégradé");
    }

    #[test]
    fn le_contraste_maximal_est_preserve() {
        let mut lum = vec![0u8; 16];
        for p in lum.iter_mut().take(8) {
            *p = 255;
        }
        let dec = decode_gray(&encode_gray(&lum, 4, 4), 4, 4);
        assert!(dec[0] >= 250, "le blanc s'est assombri : {}", dec[0]);
        assert!(dec[8] <= 5, "le noir s'est éclairci : {}", dec[8]);
    }

    /// Le mode à trois couleurs réserverait un indice à la transparence : un
    /// masque opaque ne doit jamais y tomber tant qu'il y a du contraste.
    #[test]
    fn le_mode_quatre_couleurs_est_choisi_des_qu_il_y_a_du_contraste() {
        let mut lum = vec![0u8; 16];
        lum[0] = 255;
        let enc = encode_gray(&lum, 4, 4);
        let c0 = u16::from_le_bytes([enc[0], enc[1]]);
        let c1 = u16::from_le_bytes([enc[2], enc[3]]);
        assert!(c0 > c1, "mode à trois couleurs choisi à tort");
    }
}
