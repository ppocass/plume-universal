// Page « 1337 » (§2.5) : l'interrupteur, le raccourci, les trois niveaux avec
// leur phrase d'exemple, et le convertisseur (coller → résultat → Copier).
// Tout est local : `leet_convert` ne quitte jamais la machine.

import { t } from "../js/i18n.js";
import { keyChoices } from "../js/keys.js";
import { hydrateIcons } from "../js/icons.js";
import { hasTauri, invoke } from "../js/tauri.js";

const CONTENT = document.getElementById("content");
const LEVELS = { light: 1, medium: 2, full: 3 };
const CHEVRONS =
  '<svg class="picker-chevrons" viewBox="0 0 9 13" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1 4.5 4.5 1 8 4.5"/><path d="M1 8.5 4.5 12 8 8.5"/></svg>';
const CHECK =
  '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';

let demo = null;
let prefs = null;
let root = null;
let convertTimer = null;
let copyTimer = null;
let level = "medium";

/** Un élément avec sa classe et, si besoin, son texte. */
function el(tag, className = "", text = null) {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  if (text !== null) {
    node.textContent = text;
  }
  return node;
}

/** Le menu déroulant macOS (pop-up button) : bouton + liste, sans focus ring. */
function picker(options, value, onChange) {
  const wrap = el("div", "picker");
  const button = el("button", "picker-button");
  button.type = "button";
  const current = el("span", "picker-value");
  button.append(current);
  button.insertAdjacentHTML("beforeend", CHEVRONS);
  const menu = el("div", "picker-menu");
  menu.hidden = true;
  wrap.append(button, menu);

  let selected = value;

  const renderValue = () => {
    const option = options.find((entry) => entry.value === selected) || options[0];
    current.textContent = option ? option.label : "";
  };

  const onDocument = (event) => {
    if (!wrap.contains(event.target)) {
      close();
    }
  };

  const onKey = (event) => {
    if (event.key === "Escape") {
      close();
      button.focus();
    }
  };

  function close() {
    menu.hidden = true;
    document.removeEventListener("pointerdown", onDocument, true);
    document.removeEventListener("keydown", onKey, true);
  }

  function open() {
    menu.textContent = "";
    for (const option of options) {
      const item = el("button", "picker-item");
      item.type = "button";
      const check = el("span", "picker-check");
      if (option.value === selected) {
        check.innerHTML = CHECK;
      }
      item.append(check, document.createTextNode(option.label));
      item.addEventListener("click", () => {
        selected = option.value;
        renderValue();
        close();
        button.focus();
        onChange(option.value);
      });
      menu.append(item);
    }
    menu.hidden = false;
    document.addEventListener("pointerdown", onDocument, true);
    document.addEventListener("keydown", onKey, true);
  }

  button.addEventListener("click", () => {
    if (menu.hidden) {
      open();
    } else {
      close();
    }
  });

  renderValue();
  return wrap;
}

async function save(patch) {
  try {
    prefs = await invoke("set_prefs", { patch });
  } catch (error) {
    console.warn("réglage 1337 non enregistré :", error);
  }
}

async function convert() {
  const input = root?.querySelector("[data-input]");
  const result = root?.querySelector("[data-result]");
  const output = root?.querySelector("[data-output]");
  if (!input || !result || !output) {
    return;
  }
  const text = input.value;
  if (!text.trim()) {
    result.hidden = true;
    output.textContent = "";
    return;
  }
  try {
    const converted = await invoke("leet_convert", {
      text,
      level: LEVELS[level] ?? 2,
      reverse: false,
    });
    output.textContent = converted;
    result.hidden = false;
  } catch (error) {
    console.warn("conversion 1337 impossible :", error);
  }
}

function scheduleConvert() {
  if (convertTimer) {
    clearTimeout(convertTimer);
  }
  convertTimer = setTimeout(() => {
    convertTimer = null;
    convert();
  }, 120);
}

/** Les trois phrases d'exemple, maquillées à leur niveau. */
async function renderSamples() {
  const sample = t("the_quick_brown_fox_jumps_over_the_lazy_dog");
  await Promise.all(
    Object.entries(LEVELS).map(async ([name, value]) => {
      const node = root?.querySelector(`[data-sample="${name}"]`);
      if (!node) {
        return;
      }
      node.textContent = sample;
      try {
        node.textContent = await invoke("leet_convert", {
          text: sample,
          level: value,
          reverse: false,
        });
      } catch {
        /* le texte de départ reste affiché */
      }
    }),
  );
}

function renderLevels() {
  for (const row of root.querySelectorAll(".level-row")) {
    row.setAttribute("aria-checked", row.dataset.level === level ? "true" : "false");
  }
}

function wireLevels() {
  for (const row of root.querySelectorAll(".level-row")) {
    row.addEventListener("click", async () => {
      level = row.dataset.level;
      renderLevels();
      await save({ leetLevel: level });
      renderSamples();
      convert();
    });
  }
}

function wireToggle() {
  const toggle = root.querySelector('[data-toggle="leetEnabled"]');
  if (!toggle) {
    return;
  }
  toggle.setAttribute("aria-checked", prefs.leetEnabled ? "true" : "false");
  toggle.setAttribute("aria-label", t("enable_1337_dictation"));
  toggle.addEventListener("click", async () => {
    const next = toggle.getAttribute("aria-checked") !== "true";
    toggle.setAttribute("aria-checked", next ? "true" : "false");
    await save({ leetEnabled: next });
  });
}

function wireCopy() {
  const button = root.querySelector("[data-copy]");
  const output = root.querySelector("[data-output]");
  if (!button || !output) {
    return;
  }
  const label = button.querySelector("[data-copy-label]");
  const icon = button.querySelector("[data-copy-icon]");

  button.addEventListener("click", async () => {
    const text = output.textContent || "";
    if (!text) {
      return;
    }
    try {
      await invoke("copy_text", { text });
      label.textContent = t("copied");
      if (icon) {
        icon.dataset.icon = "check";
        delete icon.dataset.iconReady;
        await hydrateIcons(root);
      }
      if (copyTimer) {
        clearTimeout(copyTimer);
      }
      copyTimer = setTimeout(async () => {
        label.textContent = t("copy");
        if (icon) {
          icon.dataset.icon = "clipboard";
          delete icon.dataset.iconReady;
          await hydrateIcons(root);
        }
      }, 1400);
    } catch (error) {
      console.warn("copie impossible :", error);
    }
  });
}

async function mount(node) {
  root = node;
  try {
    prefs = hasTauri() ? await invoke("get_prefs") : null;
  } catch {
    prefs = null;
  }
  level = prefs?.leetLevel || "medium";

  wireToggle();
  wireLevels();
  renderLevels();

  const slot = root.querySelector('[data-picker="leetKey"]');
  if (slot) {
    const keys = keyChoices().map(([value, label]) => ({ value, label }));
    slot.append(
      picker(keys, prefs?.leetKey || "super", (value) => save({ leetKey: value })),
    );
  }

  root.querySelector("[data-input]")?.addEventListener("input", scheduleConvert);
  wireCopy();

  await renderSamples();
  await hydrateIcons(root);

  if (demo === "page=leet-converted") {
    const input = root.querySelector("[data-input]");
    if (input) {
      input.value = t("the_quick_brown_fox_jumps_over_the_lazy_dog");
      await convert();
    }
  }
}

async function ensureMounted() {
  const node = CONTENT?.querySelector('[data-page="leet"]');
  if (!node || node.dataset.mounted === "true") {
    return;
  }
  node.dataset.mounted = "true";
  if (!demo && hasTauri()) {
    try {
      demo = (await invoke("ui_env"))?.demo ?? null;
    } catch {
      demo = null;
    }
  }
  await mount(node);
}

if (CONTENT) {
  new MutationObserver(ensureMounted).observe(CONTENT, { childList: true, subtree: true });
  ensureMounted();
}
