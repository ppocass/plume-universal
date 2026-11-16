fn main() {
    // Les .so de sherpa-onnx vivent dans /usr/lib/plume une fois le .deb
    // installé ; le `$ORIGIN` de .cargo/config.toml continue de couvrir le
    // dossier du binaire. Seul le binaire de l'app reçoit ce chemin (les tests
    // de plume-core gardent `$ORIGIN`).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bin=plume=-Wl,-rpath,/usr/lib/plume");
    }
    tauri_build::build()
}
