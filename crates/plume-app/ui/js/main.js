// La coquille : barre latérale, chargement des pages, textes, thème, badges.

import { applyI18n, initI18n, t } from "./i18n.js";
import { hydrateIcons } from "./icons.js";
import { initTheme } from "./theme.js";
import { invoke, listen, hasTauri } from "./tauri.js";

const PAGES = ["home", "history", "vocabulary", "leet", "models", "settings"];

/** Le titre de la page (clé de texte ; « 1337 » est écrit tel quel). */
const TITLES = {
  home: "home",
  history: "history",
  vocabulary: "vocabulary",
  leet: null,
  models: "models",
  settings: "settings_2",
};

const content = document.getElementById("content");
const rows = [...document.querySelectorAll(".sidebar-row[data-page]")];

let current = "home";

function pageTitle(page) {
  const key = TITLES[page];
  return key ? t(key) : "1337";
}

function markSelected(page) {
  for (const row of rows) {
    const selected = row.dataset.page === page;
    row.classList.toggle("is-selected", selected);
    if (selected) {
      row.setAttribute("aria-current", "page");
    } else {
      row.removeAttribute("aria-current");
    }
  }
}

async function loadPage(page) {
  if (!PAGES.includes(page)) {
    return;
  }
  current = page;
  markSelected(page);
  document.title = `Plume — ${pageTitle(page)}`;
  try {
    const response = await fetch(new URL(`../pages/${page}.html`, import.meta.url));
    content.innerHTML = await response.text();
  } catch (error) {
    console.warn(`page ${page} indisponible :`, error);
    content.textContent = "";
    return;
  }
  applyI18n(content);
  await hydrateIcons(content);
  // Une page peut apporter son script ; les placeholders de cette tâche n'en
  // ont pas, l'import échoue donc sans bruit.
  try {
    await import(new URL(`../pages/${page}.js`, import.meta.url));
  } catch {
    /* page sans script */
  }
}

function selectPage(page) {
  if (PAGES.includes(page) && page !== current) {
    return loadPage(page);
  }
  return Promise.resolve();
}

function wireNavigation() {
  for (const row of rows) {
    row.addEventListener("click", () => loadPage(row.dataset.page));
  }
}

/** Le badge de l'historique est le nombre de dictées (0 → pas de badge). */
async function refreshHistoryBadge() {
  const badge = document.querySelector('[data-badge="history"]');
  if (!badge) {
    return;
  }
  try {
    const history = await invoke("get_history");
    const count = Array.isArray(history) ? history.length : 0;
    badge.textContent = count > 0 ? String(count) : "";
  } catch {
    badge.textContent = "";
  }
}

/** Le pied : point et libellé de l'état du moteur (§2.1). */
async function refreshEngineState() {
  const dot = document.querySelector(".engine-dot");
  const label = document.querySelector(".engine-text");
  if (!dot || !label) {
    return;
  }
  try {
    const info = await invoke("app_info");
    if (info.modelPresent) {
      dot.dataset.state = "ready";
      label.textContent = t("name_ready", { name: "Parakeet" });
    } else {
      dot.dataset.state = "loading";
      label.textContent = t("model_not_downloaded");
    }
  } catch {
    dot.dataset.state = "idle";
    label.textContent = t("idle");
  }
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
  await hydrateIcons(document);
  wireNavigation();

  await refreshHistoryBadge();
  await refreshEngineState();

  const requested = window.location.hash.slice(1);
  await loadPage(PAGES.includes(requested) ? requested : "home");

  await listen("plume://navigate", (event) => {
    selectPage(event.payload);
  });

  // Réglages changés ailleurs (assistant, Nouveautés…) : la fenêtre a été
  // créée au démarrage, avant l'assistant, et gardait l'anglais et « modèle non
  // téléchargé ». On ne recharge la page que si la langue change, pour ne pas
  // couper une saisie en cours dans les Réglages.
  let shownLanguage = prefs?.appLanguage ?? null;
  let shownAppearance = prefs?.appearance ?? null;
  await listen("plume://prefs-changed", async (event) => {
    const next = event.payload ?? {};
    if (next.appearance !== shownAppearance) {
      shownAppearance = next.appearance;
      await initTheme(next);
    }
    if (next.appLanguage !== shownLanguage) {
      shownLanguage = next.appLanguage;
      await initI18n(next);
      await hydrateIcons(document);
      await loadPage(current);
    } else if (current === "home") {
      // La carte « Configuration requise » de l'Accueil relit son état.
      await loadPage(current);
    }
    await refreshEngineState();
  });
  await listen("plume://model-done", async () => {
    await refreshEngineState();
    if (current === "home") {
      await loadPage(current);
    }
  });
}

boot().catch((error) => console.error("démarrage de la coquille :", error));
