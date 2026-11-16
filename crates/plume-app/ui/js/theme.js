// Le thème : clair par défaut, sombre si le système le demande (media query),
// `data-theme` si les réglages imposent clair/sombre, et `PLUME_THEME` (test)
// par-dessus tout.

import { invoke, hasTauri } from "./tauri.js";

export async function initTheme(prefs = null) {
  let theme = null;

  if (hasTauri()) {
    try {
      const env = await invoke("ui_env");
      theme = env?.theme ?? null;
    } catch {
      theme = null;
    }
  }

  if (!theme && prefs?.appearance === "dark") {
    theme = "dark";
  }
  if (!theme && prefs?.appearance === "light") {
    theme = "light";
  }

  if (theme === "dark" || theme === "light") {
    document.documentElement.dataset.theme = theme;
  } else {
    delete document.documentElement.dataset.theme;
  }
  return theme;
}
