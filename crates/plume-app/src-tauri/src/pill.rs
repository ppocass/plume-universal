//! La géométrie de la fenêtre de la pastille (spec §3.1, `PillPanel.swift`).
//!
//! La fenêtre est plus grande que la carte : elle garde 26 pt de marge pour
//! l'ombre de chaque côté et 26 pt sous la carte. Elle change de taille quand
//! la pastille devient le pop-up « rien à coller » (430 × 96 au lieu de
//! 320 × 64), et **c'est le Rust qui la redimensionne** : sous Linux et
//! Windows, le front n'a pas la permission de le faire, et le redimensionnement
//! doit se faire avant l'affichage, sans que la carte saute de place.
//!
//! Le style « bottom » se pose **au-dessus de la zone de travail** du moniteur
//! (barre des tâches exclue), pas au bord de l'écran : c'est ce qui manquait,
//! et la pastille tombait hors de l'écran.

use plume_core::store::prefs::PillStyle;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// La marge d'ombre horizontale, de chaque côté de la carte (§3.1).
pub const SHADOW_MARGIN_X: f64 = 26.0;
/// La marge d'ombre sous la carte (§3.1).
pub const SHADOW_MARGIN_BOTTOM: f64 = 26.0;
/// La carte de la pastille ordinaire : 320 × 64 pt (§3.1).
pub const CARD_PILL: (f64, f64) = (320.0, 64.0);
/// La carte du pop-up « rien à coller » : 430 × 96 pt (§3.1).
pub const CARD_POPUP: (f64, f64) = (430.0, 96.0);
/// Le bas de la fenêtre, au-dessus de la zone de travail
/// (`visibleFrame.minY + 14`, §3.1).
pub const BOTTOM_MARGIN: f64 = 14.0;

/// La zone utile d'un moniteur, en points logiques : origine en haut à gauche.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Area {
    /// Le bord bas de la zone de travail (au-dessus de la barre des tâches).
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Le milieu horizontal, où la pastille se centre.
    pub fn mid_x(&self) -> f64 {
        self.x + self.width / 2.0
    }
}

/// La taille de la fenêtre pour une carte donnée (§3.1).
pub fn window_size(preview: bool) -> (f64, f64) {
    let (width, height) = if preview { CARD_POPUP } else { CARD_PILL };
    (width + SHADOW_MARGIN_X * 2.0, height + SHADOW_MARGIN_BOTTOM)
}

/// La position de la fenêtre, pour une taille réelle donnée.
///
/// - **bottom** : le bord bas de la fenêtre est fixe, à `BOTTOM_MARGIN` de la
///   zone de travail ; elle grandit vers le haut quand le pop-up s'ouvre ;
/// - **top** / **notch** : le bord haut est fixe, au bord supérieur de l'écran.
///
/// `height` est la hauteur **réelle** de la fenêtre : sous X11/GTK, une fenêtre
/// sans bordure ne descend pas toujours à la hauteur que WebKit réclame. En
/// raisonnant sur la taille effective, la carte reste collée à la zone de
/// travail au lieu de passer sous l'écran.
pub fn position(style: PillStyle, area: Area, width: f64, height: f64) -> (f64, f64) {
    let x = (area.mid_x() - width / 2.0).round();
    let y = if style.hangs_from_top() {
        area.y
    } else {
        (area.bottom() - height - BOTTOM_MARGIN).max(area.y)
    };
    (x, y.round())
}

/// La position et la taille nominales, sans tenir compte du système.
pub fn frame(style: PillStyle, preview: bool, area: Area) -> (f64, f64, f64, f64) {
    let (width, height) = window_size(preview);
    let (x, y) = position(style, area, width, height);
    (x, y, width, height)
}

/// La zone de travail du moniteur de la pastille, en points logiques.
fn work_area(app: &AppHandle) -> Option<(Area, f64)> {
    let window = app.get_webview_window("pill")?;
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = if monitor.scale_factor() > 0.0 {
        monitor.scale_factor()
    } else {
        1.0
    };
    let work = monitor.work_area();
    Some((
        Area {
            x: work.position.x as f64 / scale,
            y: work.position.y as f64 / scale,
            width: work.size.width as f64 / scale,
            height: work.size.height as f64 / scale,
        },
        scale,
    ))
}

/// Applique la taille et la position de la pastille à sa fenêtre.
///
/// Appelée au démarrage, à chaque changement de phase (le pop-up n'a pas la
/// même largeur) et quand le style change dans les Réglages.
pub fn place(app: &AppHandle, style: PillStyle, preview: bool) {
    apply(app, style, preview, false);

    // GTK/X11 applique `setSize` de façon asynchrone, et **rogne un
    // `setPosition` d'après la taille encore en vigueur** : la fenêtre du
    // pop-up restait collée à l'ordonnée de la pastille et dépassait sous
    // l'écran (les captures « bottom » étaient noires). Deux passes tardives,
    // sur la taille réelle, remettent la fenêtre à sa place.
    let generation = PLACE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    for delay in [Duration::from_millis(120), Duration::from_millis(420)] {
        let app = app.clone();
        std::thread::Builder::new()
            .name("plume-pastille-place".into())
            .spawn(move || {
                std::thread::sleep(delay);
                if PLACE_GENERATION.load(Ordering::SeqCst) == generation {
                    apply(&app, style, preview, true);
                }
            })
            .ok();
    }
}

/// Le compteur qui empêche une correction tardive d'écraser un placement plus
/// récent (deux changements de carte coup sur coup).
static PLACE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// `settle` : `false` au premier placement (taille et position nominales, en
/// points) ; `true` pour les passes de correction, qui repartent de la taille
/// que le système a réellement appliquée.
fn apply(app: &AppHandle, style: PillStyle, preview: bool, settle: bool) {
    let Some(window) = app.get_webview_window("pill") else {
        return;
    };
    let Some((area, scale)) = work_area(app) else {
        return;
    };
    let (x, y, width, height) = frame(style, preview, area);

    if !settle {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
        return;
    }

    let Ok(size) = window.outer_size() else {
        return;
    };
    let (real_width, real_height) = (size.width as f64 / scale, size.height as f64 / scale);
    let (x, y) = position(style, area, real_width, real_height);
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un écran de 1280 × 800 avec une barre des tâches de 48 pt en bas.
    fn area() -> Area {
        Area {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 752.0,
        }
    }

    #[test]
    fn la_fenetre_ajoute_les_marges_d_ombre() {
        assert_eq!(window_size(false), (372.0, 90.0));
        assert_eq!(window_size(true), (482.0, 122.0));
    }

    #[test]
    fn la_pastille_du_bas_reste_dans_la_zone_de_travail() {
        let (x, y, width, height) = frame(PillStyle::Bottom, false, area());
        assert_eq!(width, 372.0);
        assert_eq!(height, 90.0);
        // Centrée sur la largeur utile.
        assert_eq!(x, (1280.0 - 372.0) / 2.0);
        // Le bas de la fenêtre est 14 pt au-dessus du bas de la zone de
        // travail (au-dessus du Dock / de la barre des tâches), §3.1.
        assert_eq!(y + height, 752.0 - 14.0);
        assert!(y >= 0.0 && y + height <= 800.0, "y = {y}");
    }

    #[test]
    fn le_pop_up_du_bas_grandit_vers_le_haut_sans_bouger_le_bas() {
        let (_, pill_y, _, pill_h) = frame(PillStyle::Bottom, false, area());
        let (_, popup_y, _, popup_h) = frame(PillStyle::Bottom, true, area());
        // Bord bas identique : la pastille ne saute pas quand le pop-up s'ouvre.
        assert_eq!(pill_y + pill_h, popup_y + popup_h);
        // Et elle monte de la différence de hauteur (122 − 90 = 32).
        assert_eq!(pill_y - popup_y, 32.0);
    }

    #[test]
    fn la_pastille_du_haut_colle_au_bord_superieur() {
        let (x, y, width, height) = frame(PillStyle::Top, false, area());
        assert_eq!(y, 0.0);
        assert_eq!(x, (1280.0 - 372.0) / 2.0);
        assert_eq!((width, height), (372.0, 90.0));
    }

    #[test]
    fn la_taille_reelle_du_systeme_garde_la_carte_dans_l_ecran() {
        // GTK peut imposer une fenêtre plus haute (200 pt) que les 90 pt
        // demandés : la position doit alors remonter d'autant, sinon la carte
        // passe sous l'écran — c'était la capture « bottom » toute noire.
        let (_, y) = position(PillStyle::Bottom, area(), 372.0, 200.0);
        assert_eq!(y + 200.0, 752.0 - 14.0);
        assert!(y + 200.0 <= 800.0, "y = {y}");
    }

    #[test]
    fn un_ecran_plus_court_que_la_pastille_ne_donne_pas_d_ordonnee_negative() {
        let short = Area {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 60.0,
        };
        let (_, y, _, _) = frame(PillStyle::Bottom, true, short);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn la_zone_de_travail_decalee_est_respectee() {
        // Un second écran à droite, avec une barre des tâches de 40 pt.
        let second = Area {
            x: 1280.0,
            y: 0.0,
            width: 1920.0,
            height: 1040.0,
        };
        let (x, y, _, height) = frame(PillStyle::Bottom, false, second);
        assert_eq!(x, 1280.0 + (1920.0 - 372.0) / 2.0);
        assert_eq!(y + height, 1040.0 - 14.0);
    }
}
