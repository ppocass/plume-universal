// L'écran d'accueil de l'assistant (placeholder de cette tâche) : finir
// l'assistant pose les deux preuves des réglages, ouvre la fenêtre principale
// et ferme celle-ci.

import { applyI18n, initI18n } from "./i18n.js";
import { initTheme } from "./theme.js";
import { closeCurrentWindow, hasTauri, invoke } from "./tauri.js";

async function finish() {
  if (hasTauri()) {
    try {
      await invoke("set_prefs", {
        patch: {
          onboardingDone: true,
          onboardingCompletedAt: new Date().toISOString(),
        },
      });
      await invoke("open_window", { name: "main" });
    } catch (error) {
      console.warn("fin de l'assistant :", error);
    }
  }
  await closeCurrentWindow();
}

async function boot() {
  let prefs = null;
  if (hasTauri()) {
    try {
      prefs = await invoke("get_prefs");
    } catch (error) {
      console.warn("réglages indisponibles :", error);
    }
  }
  await initI18n(prefs);
  await initTheme(prefs);
  applyI18n();

  document.getElementById("skip")?.addEventListener("click", finish);
  document.getElementById("continue")?.addEventListener("click", finish);
}

boot().catch((error) => console.error("assistant :", error));
