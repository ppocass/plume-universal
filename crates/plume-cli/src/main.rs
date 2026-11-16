//! Outil de test sans interface. Sous-commandes ajoutées par module.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("transcribe") => {
            if let Err(error) = transcribe(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(1);
            }
        }
        Some("leet") => {
            if let Err(error) = leet(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(2);
            }
        }
        Some("polish") => polish(&args[1..]),
        Some("keys") => {
            if let Err(error) = keys(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(1);
            }
        }
        Some("paste") => {
            if let Err(error) = paste(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(1);
            }
        }
        Some("record") => {
            if let Err(error) = record(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(1);
            }
        }
        Some("run") => {
            if let Err(error) = run(&args[1..]) {
                eprintln!("erreur : {error:#}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage : plume-cli <commande> …");
            std::process::exit(2);
        }
    }
}

/// `plume-cli transcribe <dossier-modele> <fichier.wav>…` : charge le modèle
/// une seule fois, puis affiche pour chaque fichier
/// `<nom> <secondes d'audio> <ms d'inférence> <texte ou [rejected: raison]>`.
fn transcribe(args: &[String]) -> anyhow::Result<()> {
    use anyhow::Context;
    use plume_core::audio::gate::{self, GateVerdict};
    use plume_core::audio::wav::read_wav_16k_mono;
    use plume_core::engine::{transcribe_dictation, Parakeet};
    use std::path::Path;
    use std::time::Instant;

    const USAGE: &str = "usage : plume-cli transcribe <dossier-modele> <fichier.wav>…";
    let model_dir = args.first().context(USAGE)?;
    let wavs = &args[1..];
    if wavs.is_empty() {
        anyhow::bail!("{USAGE}");
    }

    let engine = Parakeet::load(Path::new(model_dir), 4)
        .with_context(|| format!("chargement du moteur Parakeet ({model_dir})"))?;

    for wav in wavs {
        let samples = read_wav_16k_mono(wav)?;
        let seconds = samples.len() as f64 / gate::SAMPLE_RATE as f64;
        let name = Path::new(wav)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| wav.clone());

        match gate::analyse(&samples, gate::SAMPLE_RATE) {
            GateVerdict::Rejected { reason } => {
                println!("{name}\t{seconds:.2}\t0\t[rejected: {reason}]");
            }
            GateVerdict::Speech { .. } => {
                let started = Instant::now();
                let text = transcribe_dictation(&engine, &samples)?;
                let ms = started.elapsed().as_millis();
                println!("{name}\t{seconds:.2}\t{ms}\t{}", text.unwrap_or_default());
            }
        }
    }

    Ok(())
}

/// `plume-cli leet <light|medium|full> <texte…>` : maquille le texte en 1337.
fn leet(args: &[String]) -> anyhow::Result<()> {
    use anyhow::Context;
    use plume_core::text::{Level, Leet};

    const USAGE: &str = "usage : plume-cli leet <light|medium|full> <texte>";
    let level = args.first().context(USAGE)?;
    let level = Level::parse(level)
        .with_context(|| format!("niveau 1337 inconnu : {level} ({USAGE})"))?;
    let text = args[1..].join(" ");
    if text.is_empty() {
        anyhow::bail!("{USAGE}");
    }
    println!("{}", Leet::transform(&text, level));
    Ok(())
}

/// `plume-cli polish <base_url> <model> <texte…>` : lisse le texte par un
/// service compatible OpenAI (clé dans `PLUME_POLISH_KEY`). Sans clé ou en cas
/// d'échec, le texte brut est rendu (même contrat que `Polisher.polish`).
fn polish(args: &[String]) {
    use plume_core::text::{polish, PolishConfig};

    const USAGE: &str = "usage : plume-cli polish <base_url> <model> <texte>";
    if args.len() < 3 {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let cfg = PolishConfig::new(
        args[0].clone(),
        args[1].clone(),
        std::env::var("PLUME_POLISH_KEY")
            .ok()
            .filter(|k| !k.is_empty()),
    );
    let text = args[2..].join(" ");
    println!("{}", polish(&cfg, &text, "auto"));
}

// --- input : le geste de dictée, le mic, le collage ---

/// `plume-cli keys [touche] [leet|none]` : écoute le geste de dictée et affiche
/// `Start` / `Stop { leet }` / `Cancel` en direct, pour tester à la main.
/// Touches : `alt_l` (défaut), `alt_r`, `ctrl_r`, `super`, `super_r`, `f1`…`f24`.
fn keys(args: &[String]) -> anyhow::Result<()> {
    use plume_core::input::gesture::{GestureConfig, Key, Output};

    let mut config = GestureConfig::default();
    if let Some(name) = args.first() {
        config.key = key_from_name(name)?;
    }
    if let Some(name) = args.get(1) {
        config.leet_key = if name.eq_ignore_ascii_case("none") || name.eq_ignore_ascii_case("aucune")
        {
            None
        } else {
            Some(key_from_name(name)?)
        };
    }
    println!(
        "geste : {} (1337 : {}) — Ctrl-C pour quitter",
        config.key.name(),
        config
            .leet_key
            .map(Key::name)
            .unwrap_or_else(|| "aucun".into())
    );

    let outputs = plume_core::input::listener::start(config)?;
    for output in outputs {
        match output {
            Output::Start => println!("Start"),
            Output::Stop { leet } => println!("Stop {{ leet: {leet} }}"),
            Output::Cancel => println!("Cancel"),
        }
        // En direct : sans ça, la sortie reste dans le tampon du terminal.
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    Ok(())
}

/// `plume-cli paste <texte>` : le collage complet (presse-papiers + Ctrl+V +
/// restauration).
fn paste(args: &[String]) -> anyhow::Result<()> {
    let text = args.join(" ");
    if text.is_empty() {
        anyhow::bail!("usage : plume-cli paste <texte>");
    }
    plume_core::input::paste(&text)?;
    // Sous X11, le presse-papiers appartient au processus : il doit vivre le
    // temps que l'app cible lise le texte, puis que l'ancien contenu revienne
    // (0,8 s).
    std::thread::sleep(std::time::Duration::from_millis(1_000));
    println!("collé ({} caractères)", text.chars().count());
    Ok(())
}

/// `plume-cli record <secondes> <sortie.wav>` : le micro de bout en bout, en
/// WAV 16 kHz mono. `PLUME_FAKE_AUDIO=<fichier.wav>` remplace le micro.
fn record(args: &[String]) -> anyhow::Result<()> {
    use anyhow::Context;
    use plume_core::input::mic::{write_wav, Mic, SAMPLE_RATE};

    const USAGE: &str = "usage : plume-cli record <secondes> <sortie.wav>";
    let seconds: f64 = args.first().context(USAGE)?.parse().context("durée invalide")?;
    let path = args.get(1).context(USAGE)?;
    if !(seconds > 0.0) {
        anyhow::bail!("la durée doit être positive");
    }

    let mut mic = Mic::new();
    mic.start()?;
    let fake = mic.is_fake();
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    let mut samples = mic.stop();

    // En mode factice, le fichier fait rarement la durée demandée : on le répète
    // (ou on le coupe) pour que la sortie dure exactement ce qu'on a demandé.
    if fake {
        samples = fit_len(&samples, (seconds * SAMPLE_RATE as f64).round() as usize);
    }
    if samples.is_empty() {
        anyhow::bail!("rien enregistré (micro muet ?)");
    }

    write_wav(path, &samples)?;
    println!(
        "{} échantillons ({:.2} s à {} Hz) → {}",
        samples.len(),
        samples.len() as f64 / SAMPLE_RATE as f64,
        SAMPLE_RATE,
        path
    );
    Ok(())
}

// --- controller : la dictée de bout en bout ---

/// `plume-cli run --model <dossier> [--key AltLeft] [--polish-url URL --polish-model M]` :
/// Plume sans interface — on appuie sur la touche, on parle, le texte est collé
/// au curseur. Les phases partent sur stderr ; Ctrl-C quitte.
///
/// Le lissage prend sa clé dans `PLUME_POLISH_KEY` (comme `plume-cli polish`).
fn run(args: &[String]) -> anyhow::Result<()> {
    use plume_core::controller::{channel, Controller, Settings};
    use plume_core::input::gesture::Key;
    use plume_core::text::PolishConfig;

    const USAGE: &str = "usage : plume-cli run --model <dossier> [--key AltLeft] \
                         [--polish-url URL --polish-model M]";

    let mut model: Option<String> = None;
    let mut key: Option<Key> = None;
    let mut polish_url: Option<String> = None;
    let mut polish_model: Option<String> = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--model" => {
                model = Some(option_value(args, index, USAGE)?.to_string());
                index += 2;
            }
            "--key" => {
                key = Some(key_from_name(option_value(args, index, USAGE)?)?);
                index += 2;
            }
            "--polish-url" => {
                polish_url = Some(option_value(args, index, USAGE)?.to_string());
                index += 2;
            }
            "--polish-model" => {
                polish_model = Some(option_value(args, index, USAGE)?.to_string());
                index += 2;
            }
            other => anyhow::bail!("option inconnue « {other} » ({USAGE})"),
        }
    }

    let model = model.ok_or_else(|| anyhow::anyhow!("--model manquant ({USAGE})"))?;
    let mut settings = Settings::new(&model);
    if let Some(key) = key {
        settings.key = key;
    }
    match (polish_url, polish_model) {
        (Some(url), Some(model)) => {
            let api_key = std::env::var("PLUME_POLISH_KEY")
                .ok()
                .filter(|key| !key.is_empty());
            settings.polish = Some(PolishConfig::new(url, model, api_key));
        }
        (None, None) => {}
        (Some(_), None) => anyhow::bail!("--polish-model manquant ({USAGE})"),
        (None, Some(_)) => anyhow::bail!("--polish-url manquant ({USAGE})"),
    }

    let (sender, events) = channel();
    std::thread::Builder::new()
        .name("plume-phases".into())
        .spawn(move || print_events(events))?;

    let mut controller = Controller::system(settings, sender);
    let gesture = controller.gesture_config();
    eprintln!(
        "geste : {} (1337 : {}) — Ctrl-C pour quitter",
        gesture.key.name(),
        gesture
            .leet_key
            .map(Key::name)
            .unwrap_or_else(|| "aucun".into())
    );
    controller.run()
}

/// La valeur qui suit une option (`--model <dossier>`), avec le mode d'emploi.
fn option_value<'a>(args: &'a [String], index: usize, usage: &str) -> anyhow::Result<&'a str> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("valeur manquante pour {} ({usage})", args[index]))
}

/// Les événements sur stderr, une ligne par phase (les niveaux, 20 par seconde,
/// ne sont pas affichés).
fn print_events(events: std::sync::mpsc::Receiver<plume_core::controller::Event>) {
    use plume_core::controller::Event;

    for event in events {
        match event {
            Event::Phase(phase) => eprintln!("{}", phase_line(&phase)),
            Event::Text(text) => eprintln!("texte : {text}"),
            Event::Level(_) => {}
        }
    }
}

/// La phase en une ligne, pour le terminal.
fn phase_line(phase: &plume_core::controller::Phase) -> String {
    use plume_core::controller::Phase;

    match phase {
        Phase::Idle => "inactif".to_string(),
        Phase::Recording => "enregistrement…".to_string(),
        Phase::Processing => "traitement…".to_string(),
        Phase::Done { pasted: true } => "collé".to_string(),
        Phase::Done { pasted: false } => "texte prêt (non collé)".to_string(),
        Phase::Cancelled => "annulé".to_string(),
        Phase::Rejected(reason) => format!("rien à transcrire ({reason})"),
        Phase::Error(error) => format!("erreur : {error}"),
    }
}

fn key_from_name(name: &str) -> anyhow::Result<plume_core::input::gesture::Key> {
    plume_core::input::gesture::Key::parse(name).ok_or_else(|| {
        anyhow::anyhow!("touche inconnue « {name} » (alt_l, alt_r, ctrl_r, super, f5…)")
    })
}

/// Coupe ou répète `samples` pour qu'il fasse exactement `wanted` échantillons.
fn fit_len(samples: &[f32], wanted: usize) -> Vec<f32> {
    if wanted == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![0.0; wanted];
    }
    if samples.len() >= wanted {
        return samples[..wanted].to_vec();
    }
    let mut out = Vec::with_capacity(wanted);
    while out.len() < wanted {
        let take = (wanted - out.len()).min(samples.len());
        out.extend_from_slice(&samples[..take]);
    }
    out
}
