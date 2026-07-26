//! Mise à jour du binaire portable, par remplacement et relance.
//!
//! Windows interdit de *supprimer* un exécutable en cours d'exécution, mais
//! autorise à le *renommer*. Toute la manœuvre repose là-dessus :
//!
//! 1. renommer l'exe courant en `<nom>.old` ;
//! 2. écrire le nouveau binaire à l'emplacement d'origine ;
//! 3. relancer, puis quitter ;
//! 4. supprimer le `.old` au démarrage suivant, quand il n'est plus verrouillé.
//!
//! Le contenu téléchargé est vérifié par SHA-256 contre la somme publiée dans
//! la release. C'est une garantie d'**intégrité**, pas d'authenticité : elle
//! détecte un téléchargement corrompu ou tronqué, pas une release
//! compromise. L'authenticité demanderait le plugin updater de Tauri et sa
//! paire de clés de signature.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Suffixe du binaire évincé, en attente de suppression.
const OLD_SUFFIX: &str = ".old";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn old_path(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_os_string();
    s.push(OLD_SUFFIX);
    PathBuf::from(s)
}

/// Supprime le binaire évincé par une mise à jour précédente.
///
/// Appelé au démarrage : à ce moment le fichier n'est plus verrouillé, alors
/// qu'il l'était encore juste après la relance. Un échec est sans gravité —
/// il reste un fichier inerte, qu'on retentera au prochain lancement.
pub fn cleanup_previous() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_path(&exe);
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// L'exécutable est-il une installation NSIS plutôt qu'un portable ?
///
/// L'installeur dépose son désinstalleur à côté du binaire. Se remplacer
/// soi-même dans ce cas désynchroniserait l'application de ce que le
/// désinstalleur connaît, et l'écriture échouerait de toute façon sous
/// Program Files sans élévation.
pub fn is_installed(exe: &Path) -> bool {
    exe.parent()
        .map(|dir| dir.join("uninstall.exe").exists())
        .unwrap_or(false)
}

#[derive(Debug)]
pub enum UpdateError {
    Installed,
    Checksum { expected: String, got: String },
    Io(std::io::Error),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installed => write!(
                f,
                "cette version est installée, pas portable : passez par l'installeur"
            ),
            Self::Checksum { expected, got } => write!(
                f,
                "empreinte du téléchargement invalide (attendu {expected}, obtenu {got})"
            ),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Installe `bytes` à la place de l'exécutable courant.
///
/// Ne relance pas : c'est à l'appelant de le faire, une fois qu'il a décidé
/// comment quitter proprement.
pub fn swap_binary(bytes: &[u8], expected_sha256: &str) -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe()?;
    if is_installed(&exe) {
        return Err(UpdateError::Installed);
    }

    // Vérifier **avant** de toucher au disque : un binaire corrompu écrit à la
    // place de l'exe courant laisserait l'utilisateur sans application.
    let got = sha256_hex(bytes);
    if !got.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(UpdateError::Checksum {
            expected: expected_sha256.trim().to_string(),
            got,
        });
    }

    let old = old_path(&exe);
    // Un .old résiduel bloquerait le renommage.
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old)?;

    // À partir d'ici l'exe d'origine n'existe plus sous son nom : si l'écriture
    // échoue, il faut impérativement le remettre en place.
    if let Err(e) = std::fs::write(&exe, bytes) {
        let _ = std::fs::rename(&old, &exe);
        return Err(UpdateError::Io(e));
    }

    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_empreinte_correspond_au_vecteur_connu() {
        // SHA-256 de la chaîne vide, valeur de référence du standard.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn le_chemin_du_binaire_evince_derive_de_l_exe() {
        let p = old_path(Path::new("/tmp/RockYouGFX.exe"));
        assert_eq!(p, PathBuf::from("/tmp/RockYouGFX.exe.old"));
    }

    /// Une empreinte qui ne correspond pas doit être rejetée avant toute
    /// écriture : c'est la seule protection contre un binaire tronqué.
    #[test]
    fn une_empreinte_divergente_est_rejetee() {
        let err = swap_binary(b"contenu", "0000").unwrap_err();
        match err {
            UpdateError::Checksum { .. } => {}
            other => panic!("attendu une erreur d'empreinte, obtenu {other}"),
        }
    }

    #[test]
    fn la_comparaison_d_empreinte_ignore_la_casse_et_les_espaces() {
        let h = sha256_hex(b"abc");
        assert!(h.eq_ignore_ascii_case(&format!("  {}  ", h.to_uppercase()).trim()));
    }
}
