//! Génération du script client de la resource.
//!
//! Pourquoi passer par du Lua plutôt que par un patch du `.gfx` : le
//! `minimap.gfx` n'est pas un fichier passif, le jeu *appelle dedans*.
//! `SETUP_HEALTH_ARMOUR` est une méthode Scaleform invoquée sur ce movie.
//! On peut donc l'appeler soi-même en Lua sur le Scaleform vanilla, et obtenir
//! le même effet sans toucher au fichier.
//!
//! L'intérêt est décisif pour l'outil : aucun asset Rockstar n'est nécessaire
//! pour le volet HUD, alors qu'un patch du `.gfx` exigerait que l'utilisateur
//! fournisse le sien.

use crate::shape::{HudMode, MinimapShape};

/// Valeur de `SETUP_HEALTH_ARMOUR` masquant les deux barres.
const HEALTH_ARMOUR_HIDDEN: u8 = 3;

/// Le script client, ou `None` si la forme ne demande aucune intervention.
///
/// Renvoyer `None` plutôt qu'un fichier vide évite de déclarer un
/// `client_script` inutile dans le manifeste.
pub fn client_script(shape: &MinimapShape) -> Option<String> {
    let hide_bars =
        shape.hud.health == HudMode::Hidden && shape.hud.armour == HudMode::Hidden;

    if !hide_bars {
        return None;
    }

    Some(format!(
        r#"-- Généré par RockYouGFX.
--
-- Les barres de vie et d'armure sont masquées en appelant la méthode
-- SETUP_HEALTH_ARMOUR du Scaleform minimap vanilla, plutôt qu'en modifiant
-- minimap.gfx. Aucun asset du jeu n'est donc redistribué.

CreateThread(function()
    local minimap = RequestScaleformMovie('minimap')

    -- Le Scaleform ne se rafraîchit pas à la demande : basculer le bigmap
    -- puis revenir est le seul déclencheur connu.
    SetRadarBigmapEnabled(true, false)
    Wait(0)
    SetRadarBigmapEnabled(false, false)

    while true do
        Wait(0)
        BeginScaleformMovieMethod(minimap, 'SETUP_HEALTH_ARMOUR')
        ScaleformMovieMethodAddParamInt({HEALTH_ARMOUR_HIDDEN})
        EndScaleformMovieMethod()
    end
end)
"#
    ))
}

/// Ce que le script ne peut pas faire, pour que l'interface le dise plutôt que
/// de laisser l'utilisateur le découvrir en jeu.
pub fn limitations(shape: &MinimapShape) -> Vec<String> {
    let mut out = Vec::new();

    let h = shape.hud.health == HudMode::Hidden;
    let a = shape.hud.armour == HudMode::Hidden;
    if h != a {
        out.push(
            "SETUP_HEALTH_ARMOUR pilote les deux barres d'un seul paramètre : \
             masquer la vie sans l'armure (ou l'inverse) demande un patch du \
             minimap.gfx."
                .into(),
        );
    }

    if shape.hud.health == HudMode::Follow || shape.hud.armour == HudMode::Follow {
        out.push(
            "Faire épouser la forme aux barres repositionne des éléments du \
             Scaleform : cela demande un minimap.gfx, le Lua ne peut que les \
             masquer ou les laisser en place."
                .into(),
        );
    }

    if shape.border.visible && shape.hud.compass != HudMode::Vanilla {
        out.push(
            "La boussole est dessinée par le Scaleform : la déplacer demande \
             un minimap.gfx."
                .into(),
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::Hud;

    fn with_hud(health: HudMode, armour: HudMode, compass: HudMode) -> MinimapShape {
        MinimapShape { hud: Hud { health, armour, compass }, ..Default::default() }
    }

    #[test]
    fn aucun_script_quand_le_hud_reste_vanilla() {
        let s = with_hud(HudMode::Vanilla, HudMode::Vanilla, HudMode::Vanilla);
        assert!(client_script(&s).is_none());
    }

    #[test]
    fn les_deux_barres_masquees_produisent_le_script() {
        let s = with_hud(HudMode::Hidden, HudMode::Hidden, HudMode::Vanilla);
        let lua = client_script(&s).expect("script attendu");
        assert!(lua.contains("SETUP_HEALTH_ARMOUR"));
        assert!(lua.contains("ScaleformMovieMethodAddParamInt(3)"));
        // Le rafraîchissement par bascule du bigmap est indispensable.
        assert!(lua.contains("SetRadarBigmapEnabled"));
    }

    /// Le native ne prend qu'un paramètre pour les deux barres : masquer l'une
    /// sans l'autre sort de ce que le Lua peut faire.
    #[test]
    fn masquer_une_seule_barre_est_signale_comme_limite() {
        let s = with_hud(HudMode::Hidden, HudMode::Vanilla, HudMode::Vanilla);
        assert!(client_script(&s).is_none());
        let lims = limitations(&s);
        assert!(lims.iter().any(|l| l.contains("SETUP_HEALTH_ARMOUR")));
    }

    #[test]
    fn le_mode_suit_est_signale_comme_demandant_un_gfx() {
        let s = with_hud(HudMode::Follow, HudMode::Follow, HudMode::Vanilla);
        let lims = limitations(&s);
        assert!(lims.iter().any(|l| l.contains("minimap.gfx")));
    }
}
