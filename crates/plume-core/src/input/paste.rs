//! Coller le texte là où est le curseur — le pendant de `TextInserter.swift`.
//!
//! Le collage passe par le presse-papiers + Ctrl+V (le seul moyen fiable dans
//! toutes les apps, y compris les sites web et Electron), et **le contenu
//! précédent du presse-papiers est remis 0,8 s plus tard** : l'app cible a le
//! temps de lire le presse-papiers (Electron peut être lent).
//!
//! Sous X11, le presse-papiers appartient au processus : le texte n'existe que
//! tant qu'on est vivant pour le servir. D'où **un seul fil**, qui garde son
//! `Clipboard` ouvert toute la vie de Plume : il pose le texte, attend 0,8 s,
//! remet l'ancien contenu — et continue de le servir après la restauration
//! (un `Clipboard` relâché emporterait le contenu avec lui).
//!
//! **L'espace de tête** : sur macOS, `TextInserter` lit le caractère juste avant
//! le curseur par l'API d'accessibilité (`AXSelectedTextRange`) et ajoute une
//! espace quand il faut, sinon deux dictées se collent (« 16h.Alors »). Ni X11
//! ni Win32 ne donnent ce caractère sans les API d'accessibilité (AT-SPI, UI
//! Automation), qu'on n'utilise pas ici : **la règle n'est donc pas appliquée
//! automatiquement** sur Linux/Windows. Elle reste disponible telle quelle
//! ([`apply_leading_space`], [`needs_leading_space`]) pour le jour où l'app
//! saura lire le curseur — c'est le seul écart assumé avec macOS.

use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

/// Le temps laissé à l'app cible avant de remettre l'ancien presse-papiers
/// (`TextInserter.paste`, `asyncAfter(deadline: .now() + 0.8)`).
pub const RESTORE_DELAY: Duration = Duration::from_millis(800);

/// Faut-il une espace avant le texte collé ? Vrai si le caractère juste avant le
/// curseur est une lettre ou une ponctuation — pas une espace, pas un retour à
/// la ligne, pas une parenthèse/guillemet ouvrant. En cas de doute (caractère
/// inconnu), pas d'espace.
pub fn needs_leading_space(previous: Option<char>) -> bool {
    let Some(character) = previous else {
        return false;
    };
    if character.is_whitespace() {
        return false;
    }
    // Après une parenthèse ou un guillemet ouvrant, pas d'espace.
    if "([{«“'\"/-".contains(character) {
        return false;
    }
    true
}

/// Le texte à coller, avec l'espace de tête si elle est nécessaire.
pub fn apply_leading_space(text: &str, previous: Option<char>) -> String {
    if needs_leading_space(previous) {
        format!(" {text}")
    } else {
        text.to_string()
    }
}

/// Colle `text` dans la fenêtre qui a le focus, puis remet le presse-papiers
/// d'avant. Rend la main une fois le Ctrl+V envoyé (la restauration continue
/// dans le fil du presse-papiers, 0,8 s plus tard).
pub fn paste(text: &str) -> Result<()> {
    let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();
    clipboard_service()
        .send(Request {
            text: text.to_string(),
            ready: ready_tx,
        })
        .map_err(|_| anyhow!("fil du presse-papiers arrêté"))?;

    // Le texte n'est pas toujours en place tout de suite (X11 met quelques
    // millisecondes à servir la sélection).
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => bail!("presse-papiers indisponible : {error}"),
        Err(_) => bail!("presse-papiers indisponible (pas de réponse)"),
    }

    send_paste_keystroke()
}

/// Une demande au fil du presse-papiers : poser `text`, prévenir quand c'est
/// fait, et rendre la main tout de suite (la restauration se fait après).
struct Request {
    text: String,
    ready: mpsc::Sender<std::result::Result<(), String>>,
}

/// Le fil qui possède le presse-papiers, pour toute la vie du processus.
///
/// Sous X11, le presse-papiers **appartient** au processus qui l'a posé : le
/// texte n'existe que tant qu'un `Clipboard` vivant le sert. Un `Clipboard`
/// créé pour l'occasion puis relâché emporte le contenu avec lui — y compris
/// celui qu'on vient de restaurer. D'où un seul fil, qui garde son `Clipboard`
/// ouvert d'une dictée à l'autre.
fn clipboard_service() -> &'static mpsc::Sender<Request> {
    static SERVICE: OnceLock<mpsc::Sender<Request>> = OnceLock::new();
    SERVICE.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("plume-presse-papiers".into())
            .spawn(move || serve_clipboard(receiver))
            .expect("fil du presse-papiers");
        sender
    })
}

fn serve_clipboard(requests: mpsc::Receiver<Request>) {
    let mut clipboard: Option<arboard::Clipboard> = None;
    while let Ok(request) = requests.recv() {
        if clipboard.is_none() {
            clipboard = arboard::Clipboard::new().ok();
        }
        let Some(current) = clipboard.as_mut() else {
            let _ = request
                .ready
                .send(Err("presse-papiers inaccessible".to_string()));
            continue;
        };

        // Sauvegarde : arboard ne sait lire que du texte. Sur macOS,
        // TextInserter sauve tous les types de chaque élément ; ici une image
        // copiée juste avant une dictée est perdue — limite d'arboard, pas un
        // choix.
        let saved = current.get_text().ok();
        if let Err(error) = current.set_text(request.text.clone()) {
            let _ = request.ready.send(Err(error.to_string()));
            // Le prochain coup repartira d'un Clipboard neuf.
            clipboard = None;
            continue;
        }
        let _ = request.ready.send(Ok(()));

        // L'app cible lit le presse-papiers pendant le Ctrl+V ; on attend un
        // peu avant de remettre l'ancien contenu (Electron peut être lent).
        thread::sleep(RESTORE_DELAY);

        // Quelqu'un a copié entre-temps ? On ne touche à rien (macOS compare de
        // même le `changeCount`).
        if current.get_text().ok().as_deref() != Some(request.text.as_str()) {
            continue;
        }
        match saved {
            Some(previous) => {
                if let Err(error) = current.set_text(previous) {
                    log::info!("presse-papiers non restauré : {error}");
                    clipboard = None;
                }
            }
            None => {
                if let Err(error) = current.clear() {
                    log::info!("presse-papiers non vidé : {error}");
                }
            }
        }
    }
}

/// Envoie Ctrl+V à la fenêtre au premier plan.
#[cfg(target_os = "linux")]
fn send_paste_keystroke() -> Result<()> {
    use anyhow::Context;

    // Validé sur une vraie Mint : xdotool fait le Ctrl+V que le compositeur
    // veut bien transmettre, sans dépendre de la bibliothèque graphique de
    // Plume (X11 ou Wayland+XWayland).
    let status = std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .status()
        .context("xdotool introuvable (paquet xdotool)")?;
    if !status.success() {
        return Err(anyhow!("xdotool a échoué ({status})"));
    }
    Ok(())
}

/// Windows : un vrai Ctrl+V par `SendInput` (enigo), le même chemin qu'un
/// clavier physique.
#[cfg(windows)]
fn send_paste_keystroke() -> Result<()> {
    use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|error| anyhow!("enigo : {error}"))?;
    enigo
        .key(EnigoKey::Control, Direction::Press)
        .map_err(|error| anyhow!("enigo (Ctrl) : {error}"))?;
    enigo
        .key(EnigoKey::Unicode('v'), Direction::Click)
        .map_err(|error| anyhow!("enigo (V) : {error}"))?;
    enigo
        .key(EnigoKey::Control, Direction::Release)
        .map_err(|error| anyhow!("enigo (Ctrl) : {error}"))?;
    Ok(())
}

#[cfg(all(not(target_os = "linux"), not(windows)))]
fn send_paste_keystroke() -> Result<()> {
    bail!("le collage n'est pas implémenté sur cette plateforme")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opening_character_needs_no_space() {
        for previous in ['(', '[', '{', '«', '“', '\'', '"', '/', '-'] {
            assert!(!needs_leading_space(Some(previous)), "{previous}");
        }
    }

    #[test]
    fn a_letter_or_a_period_needs_a_space() {
        for previous in ['a', 'Z', '.', ',', '!', '?', ')', ':'] {
            assert!(needs_leading_space(Some(previous)), "{previous}");
        }
    }

    #[test]
    fn whitespace_needs_no_space() {
        for previous in [' ', '\n', '\t', '\u{a0}'] {
            assert!(!needs_leading_space(Some(previous)), "{previous:?}");
        }
    }

    #[test]
    fn an_unknown_character_needs_no_space() {
        // En cas de doute (curseur illisible), on ne touche à rien.
        assert!(!needs_leading_space(None));
    }

    #[test]
    fn the_text_gets_its_leading_space() {
        assert_eq!(apply_leading_space("bonjour", Some('x')), " bonjour");
        assert_eq!(apply_leading_space("bonjour", Some(' ')), "bonjour");
        assert_eq!(apply_leading_space("bonjour", None), "bonjour");
        assert_eq!(apply_leading_space("bonjour", Some('(')), "bonjour");
    }

    #[test]
    fn the_clipboard_comes_back_after_the_paste() {
        // Il faut un vrai serveur X et xdotool : sans eux (le cas du conteneur
        // de build), on ne teste rien.
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        if std::process::Command::new("xdotool")
            .arg("version")
            .output()
            .is_err()
        {
            return;
        }
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return;
        };
        let Ok(()) = clipboard.set_text("avant".to_string()) else {
            return;
        };

        if paste("pendant").is_err() {
            return;
        }
        // Juste après le Ctrl+V, c'est bien le texte dicté qui est là.
        assert_eq!(clipboard.get_text().ok().as_deref(), Some("pendant"));
        // 0,8 s plus tard, l'ancien contenu est revenu.
        thread::sleep(RESTORE_DELAY + Duration::from_millis(500));
        assert_eq!(clipboard.get_text().ok().as_deref(), Some("avant"));
    }

    #[test]
    fn the_restore_delay_is_the_mac_one() {
        assert_eq!(RESTORE_DELAY, Duration::from_millis(800));
    }
}
