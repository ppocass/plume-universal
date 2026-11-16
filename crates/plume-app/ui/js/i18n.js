// Les textes de l'interface : ui/i18n/en.json et fr.json, les mêmes clés que
// l'app macOS (docs/strings). La langue vient des réglages (`appLanguage`,
// anglais par défaut).

const LANGUAGES = ["en", "fr"];
const DIR = new URL("../i18n/", import.meta.url);

let strings = {};
let language = "en";

function stringsUrl(code) {
  return new URL(`${code}.json`, DIR);
}

/** Charge la langue des réglages (ou l'anglais), puis remplit la page. */
export async function initI18n(prefs = null) {
  const wanted = prefs?.appLanguage;
  language = LANGUAGES.includes(wanted) ? wanted : "en";
  try {
    const response = await fetch(stringsUrl(language));
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    strings = await response.json();
  } catch (error) {
    console.warn("textes indisponibles :", error);
    strings = {};
  }
  applyI18n();
  return language;
}

/** La langue active (`en` ou `fr`). */
export function currentLanguage() {
  return language;
}

/** Un texte par sa clé, avec les `{placeholders}` remplacés. */
export function t(key, params = null) {
  let value = Object.prototype.hasOwnProperty.call(strings, key) ? strings[key] : key;
  if (params) {
    for (const [name, replacement] of Object.entries(params)) {
      value = value.split(`{${name}}`).join(String(replacement));
    }
  }
  return value;
}

/** Remplit tous les `data-i18n` (et `data-i18n-title`) d'un sous-arbre. */
export function applyI18n(root = document) {
  for (const node of root.querySelectorAll("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n);
  }
  for (const node of root.querySelectorAll("[data-i18n-title]")) {
    node.setAttribute("title", t(node.dataset.i18nTitle));
  }
  for (const node of root.querySelectorAll("[data-i18n-placeholder]")) {
    node.setAttribute("placeholder", t(node.dataset.i18nPlaceholder));
  }
}
