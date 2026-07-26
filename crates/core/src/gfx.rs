//! Lecture et édition des fichiers Scaleform `.gfx`.
//!
//! Un `.gfx` est un SWF dont la signature a été remplacée : `GFX` (brut) ou
//! `CFX` (corps compressé en zlib), suivie de l'octet de version et de la
//! longueur totale sur 32 bits little-endian.
//!
//! **Le fichier n'est jamais ré-encodé.** On localise les tags visés au niveau
//! octet et on remplace uniquement leur contenu, en laissant le reste du flux
//! intact. Les tags propriétaires Scaleform (plage 1000+ : `ExporterInfo`,
//! `DefineExternalImage`, atlas de polices…) traversent donc l'opération sans
//! être interprétés — ce qu'aucun cycle parse → ré-encode ne garantirait.

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{self, Read, Write};

/// En-tête : 3 octets de signature + 1 de version + 4 de longueur.
const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    /// `GFX` — Scaleform, corps brut.
    Gfx,
    /// `CFX` — Scaleform, corps compressé en zlib.
    Cfx,
    /// `FWS` — SWF standard, corps brut.
    Fws,
    /// `CWS` — SWF standard, corps compressé en zlib.
    Cws,
}

impl Signature {
    fn from_bytes(b: &[u8]) -> Option<Self> {
        match b {
            b"GFX" => Some(Self::Gfx),
            b"CFX" => Some(Self::Cfx),
            b"FWS" => Some(Self::Fws),
            b"CWS" => Some(Self::Cws),
            _ => None,
        }
    }

    fn as_bytes(self) -> &'static [u8; 3] {
        match self {
            Self::Gfx => b"GFX",
            Self::Cfx => b"CFX",
            Self::Fws => b"FWS",
            Self::Cws => b"CWS",
        }
    }

    pub fn is_compressed(self) -> bool {
        matches!(self, Self::Cfx | Self::Cws)
    }

    pub fn is_scaleform(self) -> bool {
        matches!(self, Self::Gfx | Self::Cfx)
    }
}

/// Quelques codes de tags utiles. La liste n'a pas vocation à être exhaustive :
/// tout tag inconnu est conservé tel quel, c'est justement l'intérêt du splice.
pub mod tag {
    pub const END: u16 = 0;
    pub const SHOW_FRAME: u16 = 1;
    pub const DEFINE_SHAPE: u16 = 2;
    pub const DEFINE_SHAPE2: u16 = 22;
    pub const PLACE_OBJECT2: u16 = 26;
    pub const REMOVE_OBJECT2: u16 = 28;
    pub const DEFINE_SHAPE3: u16 = 32;
    pub const DEFINE_SPRITE: u16 = 39;
    pub const PLACE_OBJECT3: u16 = 70;
    pub const SYMBOL_CLASS: u16 = 76;
    pub const DEFINE_SHAPE4: u16 = 83;
    /// Premier tag propriétaire Scaleform. Le champ de code ne faisant que
    /// 10 bits, la plage Scaleform s'arrête à 1023.
    pub const EXPORTER_INFO: u16 = 1000;
    pub const DEFINE_EXTERNAL_IMAGE: u16 = 1001;
}

/// Position d'un tag dans le corps décompressé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagRef {
    pub code: u16,
    /// Décalage du début du tag (son en-tête) dans le corps.
    pub offset: usize,
    /// Longueur de l'en-tête : 2 octets, ou 6 pour un tag long.
    pub header_len: usize,
    pub body_len: usize,
}

impl TagRef {
    pub fn body_range(&self) -> std::ops::Range<usize> {
        let start = self.offset + self.header_len;
        start..start + self.body_len
    }

    pub fn total_len(&self) -> usize {
        self.header_len + self.body_len
    }
}

#[derive(Debug, Clone)]
pub struct GfxFile {
    pub signature: Signature,
    pub version: u8,
    /// Corps décompressé : rectangle de scène, cadence, nombre d'images, puis
    /// le flux de tags.
    pub body: Vec<u8>,
    /// Décalage du premier tag dans `body`.
    tags_start: usize,
}

impl GfxFile {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(invalid("fichier trop court pour un en-tête GFX/SWF"));
        }
        let signature = Signature::from_bytes(&data[0..3])
            .ok_or_else(|| invalid("signature inconnue : ni GFX/CFX ni FWS/CWS"))?;
        let version = data[3];

        let body = if signature.is_compressed() {
            let mut out = Vec::new();
            ZlibDecoder::new(&data[HEADER_LEN..]).read_to_end(&mut out)?;
            out
        } else {
            data[HEADER_LEN..].to_vec()
        };

        let tags_start = scan_tags_start(&body)?;
        Ok(Self { signature, version, body, tags_start })
    }

    /// Sérialise le fichier.
    ///
    /// La longueur est recalculée depuis le corps courant, ce qui la garde
    /// juste après une modification qui change la taille.
    ///
    /// Attention : pour une signature compressée, le round-trip est identique
    /// au niveau du *corps*, pas des octets du fichier — zlib ne garantit pas
    /// de reproduire le flux d'origine. Sur `GFX`/`FWS`, il l'est bien.
    pub fn write(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.body.len());
        out.extend_from_slice(self.signature.as_bytes());
        out.push(self.version);
        out.extend_from_slice(&((HEADER_LEN + self.body.len()) as u32).to_le_bytes());

        if self.signature.is_compressed() {
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&self.body)?;
            out.extend_from_slice(&enc.finish()?);
        } else {
            out.extend_from_slice(&self.body);
        }
        Ok(out)
    }

    /// Énumère les tags de premier niveau.
    ///
    /// Le corps d'un `DefineSprite` contient son propre flux de tags, qui n'est
    /// pas parcouru ici : il faut le traiter explicitement via [`Self::sprite_tags`].
    pub fn tags(&self) -> io::Result<Vec<TagRef>> {
        walk_tags(&self.body, self.tags_start)
    }

    /// Tags imbriqués dans un `DefineSprite`.
    ///
    /// Les décalages renvoyés sont absolus dans `body`, donc directement
    /// utilisables avec [`Self::replace_tag_body`].
    pub fn sprite_tags(&self, sprite: &TagRef) -> io::Result<Vec<TagRef>> {
        if sprite.code != tag::DEFINE_SPRITE {
            return Err(invalid("le tag fourni n'est pas un DefineSprite"));
        }
        // En-tête d'un DefineSprite : identifiant (u16) + nombre d'images (u16).
        let start = sprite.offset + sprite.header_len + 4;
        let end = sprite.offset + sprite.total_len();
        walk_tags_until(&self.body, start, end)
    }

    /// Remplace le contenu d'un tag, en réécrivant son en-tête si la nouvelle
    /// taille impose de passer en forme longue.
    ///
    /// Tous les `TagRef` obtenus précédemment deviennent caducs : les décalages
    /// suivants ont bougé. Il faut rappeler [`Self::tags`].
    pub fn replace_tag_body(&mut self, tag: &TagRef, new_body: &[u8]) {
        let mut replacement = encode_tag_header(tag.code, new_body.len());
        replacement.extend_from_slice(new_body);
        let range = tag.offset..tag.offset + tag.total_len();
        self.body.splice(range, replacement);
    }

    /// Supprime un tag du flux.
    ///
    /// C'est ainsi qu'on masque un élément du HUD : on retire le `PlaceObject`
    /// qui l'affiche, sans toucher à sa définition.
    pub fn remove_tag(&mut self, tag: &TagRef) {
        self.body.drain(tag.offset..tag.offset + tag.total_len());
    }
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// Localise le premier tag : il suit le rectangle de scène, la cadence et le
/// nombre d'images.
fn scan_tags_start(body: &[u8]) -> io::Result<usize> {
    if body.is_empty() {
        return Err(invalid("corps SWF vide"));
    }
    // Le rectangle est encodé en bits : 5 bits de largeur de champ, puis quatre
    // champs de cette largeur.
    let nbits = (body[0] >> 3) as usize;
    let rect_bits = 5 + 4 * nbits;
    let rect_bytes = rect_bits.div_ceil(8);
    // + 2 octets de cadence (virgule fixe 8.8) + 2 de nombre d'images.
    let start = rect_bytes + 4;
    if start > body.len() {
        return Err(invalid("corps SWF tronqué avant le flux de tags"));
    }
    Ok(start)
}

fn walk_tags(body: &[u8], start: usize) -> io::Result<Vec<TagRef>> {
    walk_tags_until(body, start, body.len())
}

fn walk_tags_until(body: &[u8], start: usize, end: usize) -> io::Result<Vec<TagRef>> {
    let mut tags = Vec::new();
    let mut off = start;

    while off + 2 <= end {
        let code_and_len = u16::from_le_bytes([body[off], body[off + 1]]);
        let code = code_and_len >> 6;
        let short_len = (code_and_len & 0x3F) as usize;

        // 0x3F signale une longueur sur 32 bits qui suit l'en-tête court.
        let (header_len, body_len) = if short_len == 0x3F {
            if off + 6 > end {
                return Err(invalid("en-tête de tag long tronqué"));
            }
            let len = u32::from_le_bytes([
                body[off + 2],
                body[off + 3],
                body[off + 4],
                body[off + 5],
            ]) as usize;
            (6, len)
        } else {
            (2, short_len)
        };

        if off + header_len + body_len > end {
            return Err(invalid("tag débordant de la zone parcourue"));
        }

        tags.push(TagRef { code, offset: off, header_len, body_len });
        off += header_len + body_len;

        if code == tag::END {
            break;
        }
    }
    Ok(tags)
}

/// Plus grand code de tag représentable : le champ ne fait que 10 bits, les
/// 6 bits de poids faible portant la longueur courte.
pub const MAX_TAG_CODE: u16 = 0x3FF;

/// Encode un en-tête de tag, en forme courte quand la taille le permet.
fn encode_tag_header(code: u16, len: usize) -> Vec<u8> {
    assert!(
        code <= MAX_TAG_CODE,
        "code de tag {code} hors des 10 bits du format : le décalage déborderait en silence"
    );
    // La forme courte ne peut pas coder 0x3F, valeur réservée au marqueur de
    // forme longue : à partir de 63 octets il faut donc passer en forme longue.
    if len < 0x3F {
        ((code << 6) | len as u16).to_le_bytes().to_vec()
    } else {
        let mut v = ((code << 6) | 0x3F).to_le_bytes().to_vec();
        v.extend_from_slice(&(len as u32).to_le_bytes());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un `.gfx` minimal mais structurellement valide.
    ///
    /// Aucun asset Rockstar n'étant distribuable, les tests s'appuient sur des
    /// fichiers synthétisés. Ils valident la mécanique de splice, pas la
    /// sémantique du `minimap.gfx` réel.
    fn synth(sig: Signature, tags: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        // Rectangle de scène : nbits = 15, soit 5 + 60 bits => 9 octets.
        body.push(15 << 3);
        body.extend_from_slice(&[0u8; 8]);
        body.extend_from_slice(&[0x00, 0x1E]); // cadence
        body.extend_from_slice(&[0x01, 0x00]); // nombre d'images

        for (code, payload) in tags {
            body.extend_from_slice(&encode_tag_header(*code, payload.len()));
            body.extend_from_slice(payload);
        }
        body.extend_from_slice(&encode_tag_header(tag::END, 0));

        let file = GfxFile { signature: sig, version: 8, body, tags_start: 0 };
        // On repasse par parse/write pour que l'en-tête soit cohérent.
        let mut out = Vec::new();
        out.extend_from_slice(sig.as_bytes());
        out.push(8);
        out.extend_from_slice(&((HEADER_LEN + file.body.len()) as u32).to_le_bytes());
        if sig.is_compressed() {
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&file.body).unwrap();
            out.extend_from_slice(&enc.finish().unwrap());
        } else {
            out.extend_from_slice(&file.body);
        }
        out
    }

    /// LE test du lot : relire puis réécrire sans rien modifier doit rendre un
    /// fichier identique octet pour octet. Il valide à lui seul que rien n'est
    /// perdu en route.
    #[test]
    fn le_round_trip_neutre_est_identique_octet_pour_octet() {
        let original = synth(
            Signature::Gfx,
            &[
                (tag::DEFINE_SHAPE4, vec![1, 2, 3, 4]),
                (tag::EXPORTER_INFO, vec![9; 40]), // tag propriétaire Scaleform
                (tag::PLACE_OBJECT2, vec![5, 6]),
            ],
        );
        let parsed = GfxFile::parse(&original).expect("parse");
        assert_eq!(parsed.write().expect("write"), original);
    }

    /// Un tag inconnu de la plage Scaleform doit ressortir intact : c'est
    /// précisément ce qu'un ré-encodage ne garantirait pas.
    #[test]
    fn les_tags_scaleform_inconnus_sont_conserves() {
        // 1015 : dans la plage propriétaire Scaleform, inconnu de ce code, et
        // représentable sur les 10 bits du champ.
        const INCONNU: u16 = 1015;
        let charge: Vec<u8> = (0..200u32).map(|i| (i % 251) as u8).collect();
        let original = synth(Signature::Gfx, &[(INCONNU, charge.clone())]);
        let parsed = GfxFile::parse(&original).unwrap();

        let tags = parsed.tags().unwrap();
        let t = tags.iter().find(|t| t.code == INCONNU).expect("tag inconnu absent");
        assert_eq!(&parsed.body[t.body_range()], &charge[..]);
        assert_eq!(parsed.write().unwrap(), original);
    }

    #[test]
    fn le_corps_compresse_survit_au_round_trip() {
        let original = synth(Signature::Cfx, &[(tag::DEFINE_SHAPE3, vec![7; 100])]);
        let a = GfxFile::parse(&original).unwrap();
        // zlib ne garantit pas de reproduire le même flux : c'est le corps
        // décompressé qui doit être stable, pas les octets compressés.
        let b = GfxFile::parse(&a.write().unwrap()).unwrap();
        assert_eq!(a.body, b.body);
        assert_eq!(b.signature, Signature::Cfx);
    }

    #[test]
    fn le_remplacement_ajuste_les_tags_suivants() {
        let original = synth(
            Signature::Gfx,
            &[(tag::DEFINE_SHAPE4, vec![1, 2]), (tag::PLACE_OBJECT2, vec![3, 4])],
        );
        let mut f = GfxFile::parse(&original).unwrap();
        let t = f.tags().unwrap()[0];
        f.replace_tag_body(&t, &[9; 10]);

        let tags = f.tags().unwrap();
        assert_eq!(tags[0].body_len, 10);
        // Le tag suivant doit rester lisible et intact malgré le décalage.
        assert_eq!(tags[1].code, tag::PLACE_OBJECT2);
        assert_eq!(&f.body[tags[1].body_range()], &[3, 4]);
    }

    /// Franchir 63 octets impose de passer l'en-tête en forme longue.
    #[test]
    fn le_passage_en_en_tete_long_est_gere() {
        let original = synth(Signature::Gfx, &[(tag::DEFINE_SHAPE4, vec![1, 2])]);
        let mut f = GfxFile::parse(&original).unwrap();
        let t = f.tags().unwrap()[0];
        assert_eq!(t.header_len, 2);

        f.replace_tag_body(&t, &vec![7; 500]);
        let tags = f.tags().unwrap();
        assert_eq!(tags[0].header_len, 6, "en-tête non passé en forme longue");
        assert_eq!(tags[0].body_len, 500);
    }

    #[test]
    fn la_suppression_retire_le_tag_et_garde_les_autres() {
        let original = synth(
            Signature::Gfx,
            &[
                (tag::DEFINE_SHAPE4, vec![1]),
                (tag::PLACE_OBJECT2, vec![2]),
                (tag::SHOW_FRAME, vec![]),
            ],
        );
        let mut f = GfxFile::parse(&original).unwrap();
        let place = *f.tags().unwrap().iter().find(|t| t.code == tag::PLACE_OBJECT2).unwrap();
        f.remove_tag(&place);

        let codes: Vec<u16> = f.tags().unwrap().iter().map(|t| t.code).collect();
        assert_eq!(codes, vec![tag::DEFINE_SHAPE4, tag::SHOW_FRAME, tag::END]);
    }

    #[test]
    fn une_signature_inconnue_est_rejetee() {
        let mut bad = synth(Signature::Gfx, &[]);
        bad[0] = b'X';
        assert!(GfxFile::parse(&bad).is_err());
    }

    #[test]
    fn un_swf_standard_est_accepte() {
        let original = synth(Signature::Fws, &[(tag::DEFINE_SHAPE, vec![1])]);
        let f = GfxFile::parse(&original).unwrap();
        assert!(!f.signature.is_scaleform());
        assert_eq!(f.write().unwrap(), original);
    }
}
