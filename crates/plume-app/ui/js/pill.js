// La pastille (placeholder) : surface de la spéc et nom de la phase en cours.

import { initI18n, t } from "./i18n.js";
import { initTheme } from "./theme.js";
import { hasTauri, invoke, listen } from "./tauri.js";

const stage = document.querySelector(".pill-stage");
const label = document.getElementById("pill-label");

function phaseLabel(phase, text) {
  switch (phase) {
    case "recording":
      return t("listening");
    case "processing":
      return t("processing");
    case "done":
      return "Done";
    case "cancelled":
      return "Cancelled";
    case "rejected":
      return text || "No speech";
    case "error":
      return text || t("error");
    default:
      return t("idle");
  }
}

function show(phase, text = null) {
  stage.dataset.phase = phase;
  label.textContent = phaseLabel(phase, text);
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
  stage.dataset.style = prefs?.pillStyle === "bottom" ? "bottom" : "top";
  await initI18n(prefs);
  await initTheme(prefs);
  show("idle");

  await listen("plume://phase", (event) => {
    const payload = event.payload ?? {};
    show(payload.phase ?? "idle", payload.text ?? null);
  });
}

boot().catch((error) => console.error("pastille :", error));
