// Fenêtre « Nouveautés » (§2.9) : une page par nouveauté, retour/suivant,
// points de progression, et « C'est parti » qui marque la version comme vue.

import { applyI18n, initI18n, t } from "./i18n.js";
import { hydrateIcons } from "./icons.js";
import { initTheme } from "./theme.js";
import { closeCurrentWindow, hasTauri, invoke } from "./tauri.js";

/** L'icône Lucide de chaque page (les notes de version n'en portent pas). */
const ICONS = {
  "1337_dictation": "case-sensitive",
  indicator_style: "panel-top",
  skip: "skip-forward",
};

const stage = document.querySelector("[data-stage]");
const eyebrow = document.querySelector("[data-eyebrow]");
const dots = document.querySelector("[data-dots]");
const back = document.querySelector("[data-back]");
const next = document.querySelector("[data-next]");

let pages = [];
let index = 0;
let forward = true;

function renderDots() {
  if (!dots) {
    return;
  }
  dots.textContent = "";
  pages.forEach((_, position) => {
    const dot = document.createElement("span");
    dot.className = "wn-dot";
    if (position === index) {
      dot.setAttribute("aria-current", "true");
    }
    dots.append(dot);
  });
}

function renderPage() {
  if (!stage || !pages.length) {
    return;
  }
  const page = pages[index];
  stage.textContent = "";

  const wrapper = document.createElement("div");
  wrapper.className = "wn-page";
  wrapper.classList.add(forward ? "is-forward" : "is-backward");

  const tile = document.createElement("div");
  tile.className = "wn-tile";
  const icon = document.createElement("span");
  icon.dataset.icon = ICONS[page.titleKey] || "sparkles";
  tile.append(icon);

  const title = document.createElement("h1");
  title.className = "wn-title";
  title.textContent = t(page.titleKey);

  const text = document.createElement("p");
  text.className = "wn-text";
  text.textContent = t(page.bodyKey);

  wrapper.append(tile, title, text);
  stage.append(wrapper);
  hydrateIcons(wrapper);

  renderDots();

  if (back) {
    back.disabled = index === 0;
  }
  if (next) {
    next.textContent = index === pages.length - 1 ? t("got_it") : t("next");
  }
}

function go(delta) {
  const target = index + delta;
  if (target < 0 || target >= pages.length) {
    return;
  }
  forward = delta > 0;
  index = target;
  renderPage();
}

async function finish() {
  if (hasTauri()) {
    try {
      await invoke("mark_whats_new_seen");
    } catch (error) {
      console.warn("version vue non enregistrée :", error);
    }
  }
  await closeCurrentWindow();
}

async function boot() {
  let prefs = null;
  let data = null;

  if (hasTauri()) {
    try {
      prefs = await invoke("get_prefs");
      data = await invoke("get_whats_new");
    } catch (error) {
      console.warn("nouveautés indisponibles :", error);
    }
  }

  await initI18n(prefs);
  await initTheme(prefs);
  applyI18n();

  pages = Array.isArray(data?.items) && data.items.length ? data.items : [];
  if (!pages.length) {
    pages = [{ titleKey: "you_re_all_set", bodyKey: "your_settings_history_and_models_are_kept" }];
  }

  if (eyebrow) {
    eyebrow.textContent = t("what_s_new_in_plume_version", {
      version: data?.version ?? "0.1.0",
    });
  }

  renderPage();

  back?.addEventListener("click", () => go(-1));
  next?.addEventListener("click", () => {
    if (index === pages.length - 1) {
      finish();
    } else {
      go(1);
    }
  });
}

boot().catch((error) => console.error("nouveautés :", error));
