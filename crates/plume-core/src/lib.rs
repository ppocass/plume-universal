//! Plume, le cœur partagé (Linux, Windows). Aucune dépendance à Tauri ici :
//! l'app Tauri et l'outil `plume-cli` s'en servent tous les deux.
pub mod audio;
pub mod engine;
pub mod text;
pub mod input;
