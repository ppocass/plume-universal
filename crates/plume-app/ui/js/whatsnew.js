// « Nouveautés » (placeholder) : le titre et la version, rien de plus.

import { initI18n, t } from "./i18n.js";
import { initTheme } from "./theme.js";
import { hasTauri, invoke } from "./tauri.js";

async function boot() {
  let prefs = null;
  let info = null;
  if (hasTauri()) {
    try {
      prefs = await invoke("get_prefs");
      info = await invoke("app_info");
    } catch (error) {
      console.warn("informations indisponibles :", error);
    }
  }
  await initI18n(prefs);
  await initTheme(prefs);
  const title = document.getElementById("whatsnew-title");
  if (title) {
    title.textContent = t("what_s_new_in_plume_version", {
      version: info?.version ?? "0.1.0",
    });
  }
}

boot().catch((error) => console.error("nouveautés :", error));
