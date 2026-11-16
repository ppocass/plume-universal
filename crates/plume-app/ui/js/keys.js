// Le libellé d'une touche, à un seul endroit : Accueil, Historique, Réglages,
// 1337 et l'assistant s'en servent tous.
//
// La pastille était un portage du Mac : les libellés y étaient « Left ⌥ »,
// « Right ⌘ », « Left ⌃ ». Sous Linux et Windows, la même touche s'appelle
// Alt, Ctrl, Maj (Shift) et Super : c'est ce que dit ce module, dans la langue
// de l'interface (`ui/i18n/*.json`).

import { t } from "./i18n.js";

/** Les noms de touche du cœur (`alt_l`, `super`…), en minuscules sans
 *  séparateur, vers la clé de traduction. */
const LABELS = {
  altl: "key_left_alt",
  altr: "key_right_alt",
  alt: "key_left_alt",
  leftalt: "key_left_alt",
  rightalt: "key_right_alt",
  altleft: "key_left_alt",
  altright: "key_right_alt",
  altgr: "key_right_alt",
  option: "key_left_alt",
  optionleft: "key_left_alt",
  optionright: "key_right_alt",
  ctrll: "key_left_ctrl",
  ctrlr: "key_right_ctrl",
  ctrl: "key_left_ctrl",
  control: "key_left_ctrl",
  controlleft: "key_left_ctrl",
  controlright: "key_right_ctrl",
  leftctrl: "key_left_ctrl",
  rightctrl: "key_right_ctrl",
  shiftl: "key_left_shift",
  shiftr: "key_right_shift",
  shift: "key_left_shift",
  shiftleft: "key_left_shift",
  shiftright: "key_right_shift",
  leftshift: "key_left_shift",
  rightshift: "key_right_shift",
  super: "key_super",
  superl: "key_super",
  lsuper: "key_super",
  superr: "key_right_super",
  rsuper: "key_right_super",
  metaleft: "key_super",
  metaright: "key_right_super",
  meta: "key_super",
  cmd: "key_super",
  win: "key_super",
  windows: "key_super",
};

/** `F1`…`F24` : le nom est le même dans toutes les langues. */
const FUNCTION_KEY = /^f(\d{1,2})$/i;

/**
 * Le libellé d'une touche (`Shortcut.label` du Mac, transposé).
 *
 * `AltLeft`, `alt_l`, `altleft`, `super_r`, `F5`… sont tous acceptés ; une
 * touche inconnue est rendue telle quelle, sans jamais perdre l'utilisateur.
 */
export function keyLabel(name) {
  const raw = String(name ?? "").trim();
  if (!raw) {
    return t("key_left_alt");
  }
  const key = raw.toLowerCase().replace(/[-_\s]/g, "");
  const labelKey = LABELS[key];
  if (labelKey) {
    return t(labelKey);
  }
  const functionKey = FUNCTION_KEY.exec(key);
  if (functionKey) {
    return `F${Number(functionKey[1])}`;
  }
  return raw;
}

/** Les choix d'une touche seule, dans l'ordre des menus (Réglages, 1337). */
export const KEY_CHOICES = [
  "super",
  "super_r",
  "ctrl_l",
  "ctrl_r",
  "alt_l",
  "alt_r",
  "shift_l",
  "shift_r",
];

/** La liste `[valeur, libellé]` prête pour un menu déroulant. */
export function keyChoices() {
  return KEY_CHOICES.map((value) => [value, keyLabel(value)]);
}
