// Page « Réglages » (§2.7) : formulaire groupé natif. Chaque changement part
// aussitôt dans `set_prefs` — pas de bouton « Enregistrer ».
// Les lignes propres à macOS (autorisations, mises à jour signées) sont
// remplacées par ce qui existe hors macOS : icône de la zone de notification,
// choix du micro, position de la pastille, touche de dictée capturée.

import { applyI18n, initI18n, t } from "../js/i18n.js";
import { hydrateIcons } from "../js/icons.js";
import { initTheme } from "../js/theme.js";
import { hasTauri, invoke, listen } from "../js/tauri.js";

const CONTENT = document.getElementById("content");
const TINTS = [
  "cyan",
  "blue",
  "lavender",
  "violet",
  "pink",
  "coral",
  "red",
  "amber",
  "orange",
  "yellow",
  "mint",
  "green",
];
const IDLE_CHOICES = [15, 35, 60, 120, 300, 900, -1];
const CHEVRONS =
  '<svg class="picker-chevrons" viewBox="0 0 9 13" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1 4.5 4.5 1 8 4.5"/><path d="M1 8.5 4.5 12 8 8.5"/></svg>';
const CHECK =
  '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';

let demo = null;
let root = null;
let prefs = null;
let mics = [];
let models = [];
let engine = null;
let appInfo = null;
let capturing = false;
let editingKey = false;
let testing = false;

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

function keyLabel(name) {
  const labels = {
    altl: "key_left_alt",
    altr: "key_right_alt",
    altleft: "key_left_alt",
    altright: "key_right_alt",
    ctrll: "key_left_ctrl",
    ctrlr: "key_right_ctrl",
    controlleft: "key_left_ctrl",
    controlright: "key_right_ctrl",
    shiftl: "key_left_shift",
    shiftr: "key_right_shift",
    shiftleft: "key_left_shift",
    shiftright: "key_right_shift",
    super: "key_super",
    superr: "key_right_super",
    metaleft: "key_super",
    metaright: "key_right_super",
    cmd: "key_super",
    win: "key_super",
  };
  const raw = String(name || "");
  const key = raw.toLowerCase().replace(/[_\s-]/g, "");
  return labels[key] ? t(labels[key]) : raw;
}

function idleLabel(seconds) {
  if (seconds < 0) {
    return t("never");
  }
  if (seconds < 60) {
    return `${Math.round(seconds)} s`;
  }
  return `${Math.round(seconds / 60)} min`;
}

function polishDefaultModel(service) {
  switch (service) {
    case "groq":
      return "openai/gpt-oss-120b";
    case "openai":
    case "custom":
      return "gpt-4o-mini";
    default:
      return prefs?.polishModel || "gpt-4o-mini";
  }
}

async function save(patch) {
  try {
    if (hasTauri()) {
      prefs = await invoke("set_prefs", { patch });
    } else {
      prefs = { ...(prefs || {}), ...patch };
    }
  } catch (error) {
    console.warn("réglages non enregistrés :", error);
  }
  return prefs;
}

/* --- Pop-up button macOS ------------------------------------------------- */

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

/* --- Rendu --------------------------------------------------------------- */

function renderSwitches() {
  for (const toggle of root.querySelectorAll("[data-toggle]")) {
    const key = toggle.dataset.toggle;
    toggle.setAttribute("aria-checked", prefs?.[key] ? "true" : "false");
    const label = toggle.parentElement?.querySelector(".form-label");
    if (label) {
      toggle.setAttribute("aria-label", label.textContent);
    }
  }
}

function renderPickers() {
  const slots = {
    appLanguage: {
      options: [
        { value: "en", label: "English" },
        { value: "fr", label: "Français" },
      ],
      value: prefs?.appLanguage || "en",
      onChange: async (value) => {
        prefs = { ...prefs, appLanguage: value };
        await save({ appLanguage: value });
        await initI18n(prefs);
        applyI18n(document);
        render();
      },
    },
    appearance: {
      options: [
        { value: "system", label: t("system") },
        { value: "light", label: t("light") },
        { value: "dark", label: t("dark") },
      ],
      value: prefs?.appearance || "system",
      onChange: async (value) => {
        await save({ appearance: value });
        await initTheme(prefs);
        render();
      },
    },
    language: {
      options: [
        { value: "auto", label: t("automatic_detection_recommended") },
        { value: "fr", label: t("french") },
        { value: "en", label: t("english") },
        { value: "es", label: t("spanish") },
        { value: "de", label: t("german") },
        { value: "it", label: t("italian") },
        { value: "pt", label: t("portuguese") },
      ],
      value: prefs?.language || "auto",
      onChange: async (value) => {
        await save({ language: value });
        renderDictation();
      },
    },
    micDevice: {
      options: [
        { value: "", label: t("system_default") },
        ...mics.map((name) => ({ value: name, label: name })),
        ...(prefs?.micDevice && !mics.includes(prefs.micDevice)
          ? [{ value: prefs.micDevice, label: t("disconnected_microphone") }]
          : []),
      ],
      value: prefs?.micDevice || "",
      onChange: (value) => save({ micDevice: value || null }),
    },
    polishService: {
      options: [
        { value: "server", label: t("personal_server") },
        { value: "groq", label: "Groq" },
        { value: "openai", label: "OpenAI" },
        { value: "custom", label: t("other_openai_compatible") },
      ],
      value: prefs?.polishService || "groq",
      onChange: async (value) => {
        await save({ polishService: value });
        render();
      },
    },
    polishPrompt: {
      options: [
        { value: "auto", label: t("automatic") },
        { value: "fr", label: t("french") },
        { value: "en", label: t("english") },
      ],
      value: prefs?.polishPrompt || "auto",
      onChange: (value) => save({ polishPrompt: value }),
    },
    idleSeconds: {
      options: IDLE_CHOICES.map((value) => ({
        value: String(value),
        label: idleLabel(value),
      })),
      value: String(prefs?.idleSeconds ?? 35),
      onChange: (value) => save({ idleSeconds: Number(value) }),
    },
    leetKey: {
      options: [
        ["super", keyLabel("super")],
        ["super_r", keyLabel("super_r")],
        ["ctrl_l", keyLabel("ctrl_l")],
        ["ctrl_r", keyLabel("ctrl_r")],
        ["alt_l", keyLabel("alt_l")],
        ["alt_r", keyLabel("alt_r")],
        ["shift_l", keyLabel("shift_l")],
        ["shift_r", keyLabel("shift_r")],
      ].map(([value, label]) => ({ value, label })),
      value: prefs?.leetKey || "super",
      onChange: (value) => save({ leetKey: value }),
    },
  };

  for (const slot of root.querySelectorAll("[data-picker]")) {
    const config = slots[slot.dataset.picker];
    if (!config) {
      continue;
    }
    slot.textContent = "";
    slot.append(picker(config.options, config.value, config.onChange));
  }
}

function renderShortcut() {
  const chip = root.querySelector("[data-key-chip]");
  const button = root.querySelector("[data-capture]");
  const footer = root.querySelector("[data-shortcut-footer]");
  if (chip) {
    chip.textContent = capturing ? t("press_a_key") : keyLabel(prefs?.dictationKey || "AltLeft");
    chip.dataset.recording = capturing ? "true" : "false";
  }
  if (button) {
    button.textContent = capturing ? t("cancel") : t("change");
  }
  if (footer) {
    // La légende de Swift : `prefs.shortcut.explanation` (§2.7, section 2).
    footer.textContent = capturing
      ? t("single_key_or_combo_ctrl_shift_w_esc_to_cancel")
      : t("pressed_on_its_own_with_other_keys_it_works_as_usual");
  }
}

function renderDictation() {
  const voiceCommands = root.querySelector("[data-voice-commands]");
  if (voiceCommands) {
    voiceCommands.hidden = !prefs?.voiceCommands;
  }
  const active = models.find((view) => view.active);
  const whisperNote = root.querySelector("[data-whisper-note]");
  if (whisperNote) {
    whisperNote.hidden = !(active?.family === "whisper" && (prefs?.language || "auto") === "auto");
  }
}

function renderSegmented() {
  const group = root.querySelector("[data-segmented]");
  if (!group) {
    return;
  }
  group.textContent = "";
  group.setAttribute("aria-label", t("indicator_style"));
  for (const style of ["top", "bottom"]) {
    const button = el("button", "", t(style));
    button.type = "button";
    button.setAttribute("role", "radio");
    button.setAttribute("aria-checked", (prefs?.pillStyle || "top") === style ? "true" : "false");
    button.addEventListener("click", async () => {
      await save({ pillStyle: style });
      renderSegmented();
      renderStyleDetail();
    });
    group.append(button);
  }
}

function renderStyleDetail() {
  const detail = root.querySelector("[data-style-detail]");
  if (detail) {
    detail.textContent =
      (prefs?.pillStyle || "top") === "bottom"
        ? t("above_the_dock_as_always")
        : t("the_same_pill_at_the_top_it_glides_in_from_the_screen_edge");
  }
}

function renderTints() {
  for (const group of root.querySelectorAll("[data-tints]")) {
    const key = group.dataset.tints;
    group.textContent = "";
    for (const tint of TINTS) {
      const button = el("button", "tint");
      button.type = "button";
      button.dataset.tint = tint;
      button.setAttribute("role", "radio");
      button.setAttribute("aria-checked", prefs?.[key] === tint ? "true" : "false");
      button.setAttribute("aria-label", t(tint));
      button.title = t(tint);
      button.addEventListener("click", async () => {
        await save({ [key]: tint });
        for (const sibling of group.querySelectorAll(".tint")) {
          sibling.setAttribute(
            "aria-checked",
            sibling.dataset.tint === tint ? "true" : "false",
          );
        }
        renderPreviews();
      });
      group.append(button);
    }
  }
}

function renderPreviews() {
  for (const preview of root.querySelectorAll("[data-preview]")) {
    const key = preview.dataset.preview;
    preview.dataset.tint = prefs?.[key] || "cyan";
    preview.textContent = "";
    const bars = el("div", "pill-bars");
    for (let index = 0; index < 13; index += 1) {
      bars.append(el("span"));
    }
    preview.append(bars);
  }
}

function renderKey() {
  const box = root.querySelector("[data-key-row]");
  if (!box) {
    return;
  }
  box.textContent = "";
  if (prefs?.polishApiKey && !editingKey) {
    const saved = el("span", "key-saved");
    const icon = el("span");
    icon.dataset.icon = "circle-check";
    saved.append(icon, document.createTextNode(t("saved")));
    const change = el("button", "button-small", t("change_3"));
    change.type = "button";
    change.addEventListener("click", () => {
      editingKey = true;
      renderKey();
    });
    box.append(saved, change);
  } else {
    const input = el("input", "text-field is-key");
    input.type = "password";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.placeholder = t("paste_key");
    const saveButton = el("button", "button-small", t("save"));
    saveButton.type = "button";
    saveButton.disabled = true;
    input.addEventListener("input", () => {
      saveButton.disabled = input.value.trim().length === 0;
    });
    const commit = async () => {
      const value = input.value.trim();
      if (!value) {
        return;
      }
      await save({ polishApiKey: value });
      editingKey = false;
      renderKey();
    };
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        commit();
      }
    });
    saveButton.addEventListener("click", commit);
    box.append(input, saveButton);
  }
  hydrateIcons(box);
}

function renderCleanup() {
  const service = prefs?.polishService || "groq";
  const urlRow = root.querySelector('[data-row="polishUrl"]');
  const modelRow = root.querySelector('[data-row="polishModel"]');
  if (urlRow) {
    urlRow.hidden = service !== "custom" && service !== "server";
  }
  if (modelRow) {
    modelRow.hidden = service === "server";
  }
  const url = root.querySelector('[data-field="polishCustomUrl"]');
  if (url) {
    url.value = prefs?.polishCustomUrl || "";
  }
  const model = root.querySelector('[data-field="polishModel"]');
  if (model) {
    model.value = prefs?.polishModel || "";
    model.placeholder = polishDefaultModel(service);
  }
}

function engineLabel(state) {
  switch (state?.state) {
    case "downloading":
      return t("downloading");
    case "ready":
    case "loaded":
      return t("ready");
    case "missing":
      return t("model_not_downloaded");
    default:
      return state?.state || "—";
  }
}

function renderEngine() {
  const status = root.querySelector("[data-engine-status]");
  if (status) {
    status.textContent = engineLabel(engine);
  }
  const name = root.querySelector("[data-engine-model]");
  if (name) {
    const active = models.find((view) => view.active);
    name.textContent = active ? active.name : engine?.modelId || "—";
  }
}

function renderVersion() {
  const version = root.querySelector("[data-version]");
  if (version) {
    version.textContent = appInfo?.version || "—";
  }
}

function render() {
  if (!root) {
    return;
  }
  renderSwitches();
  renderPickers();
  renderShortcut();
  renderDictation();
  renderSegmented();
  renderStyleDetail();
  renderTints();
  renderPreviews();
  renderKey();
  renderCleanup();
  renderEngine();
  renderVersion();
}

/* --- Actions ------------------------------------------------------------- */

function wireToggles() {
  for (const toggle of root.querySelectorAll("[data-toggle]")) {
    toggle.addEventListener("click", async () => {
      const key = toggle.dataset.toggle;
      const next = toggle.getAttribute("aria-checked") !== "true";
      toggle.setAttribute("aria-checked", next ? "true" : "false");
      await save({ [key]: next });
      renderDictation();
      renderCleanup();
    });
  }
}

function wireShortcut() {
  const button = root.querySelector("[data-capture]");
  if (!button) {
    return;
  }
  button.addEventListener("click", async () => {
    if (capturing) {
      capturing = false;
      renderShortcut();
      try {
        await invoke("set_dictation_key_capture", { on: false });
      } catch {
        /* déjà désarmé */
      }
      return;
    }
    capturing = true;
    renderShortcut();
    try {
      await invoke("set_dictation_key_capture", { on: true });
    } catch (error) {
      console.warn("capture de touche indisponible :", error);
      capturing = false;
      renderShortcut();
    }
  });
}

function wireFields() {
  const url = root.querySelector('[data-field="polishCustomUrl"]');
  url?.addEventListener("change", () => save({ polishCustomUrl: url.value.trim() }));
  const model = root.querySelector('[data-field="polishModel"]');
  model?.addEventListener("change", () => save({ polishModel: model.value.trim() }));
}

function wireTest() {
  const button = root.querySelector("[data-test]");
  const result = root.querySelector("[data-test-result]");
  if (!button || !result) {
    return;
  }
  button.addEventListener("click", async () => {
    if (testing) {
      return;
    }
    testing = true;
    button.disabled = true;
    button.textContent = t("testing");
    result.textContent = "";
    result.dataset.ok = "true";

    const service = prefs?.polishService || "groq";
    let base = prefs?.polishCustomUrl || "";
    if (service === "groq") {
      base = "https://api.groq.com/openai/v1";
    } else if (service === "openai") {
      base = "https://api.openai.com/v1";
    }
    const model = prefs?.polishModel || polishDefaultModel(service);

    try {
      const reply = await invoke("polish_test", {
        baseUrl: base,
        model,
        apiKey: prefs?.polishApiKey || "",
      });
      if (reply?.ok) {
        result.dataset.ok = "true";
        result.textContent = t("ok_seconds", { seconds: (Number(reply.ms) / 1000).toFixed(2) });
      } else {
        result.dataset.ok = "false";
        result.textContent = t("connection_failed");
      }
    } catch (error) {
      console.warn("test du nettoyage :", error);
      result.dataset.ok = "false";
      result.textContent = t("connection_failed");
    } finally {
      testing = false;
      button.disabled = false;
      button.textContent = t("test");
    }
  });
}

function wireExtras() {
  root.querySelector("[data-change-model]")?.addEventListener("click", () => {
    document.querySelector('.sidebar-row[data-page="models"]')?.click();
  });
  root.querySelector("[data-run-setup]")?.addEventListener("click", async () => {
    try {
      await invoke("open_window", { name: "onboarding" });
    } catch (error) {
      console.warn("assistant indisponible :", error);
    }
  });
}

/* --- Démarrage ----------------------------------------------------------- */

async function mount(node) {
  root = node;
  try {
    prefs = hasTauri() ? await invoke("get_prefs") : null;
  } catch {
    prefs = null;
  }
  try {
    mics = hasTauri() ? await invoke("list_mics") : [];
  } catch {
    mics = [];
  }
  try {
    models = hasTauri() ? await invoke("list_models") : [];
  } catch {
    models = [];
  }
  try {
    engine = hasTauri() ? await invoke("engine_status") : null;
  } catch {
    engine = null;
  }
  try {
    appInfo = hasTauri() ? await invoke("app_info") : null;
  } catch {
    appInfo = null;
  }

  wireToggles();
  wireShortcut();
  wireFields();
  wireTest();
  wireExtras();
  render();
  await hydrateIcons(root);

  if (demo === "page=settings-cleanup") {
    requestAnimationFrame(() => {
      const section = root?.querySelector('[data-section="cleanup"]');
      const content = document.getElementById("content");
      if (!section || !content) {
        return;
      }
      section.scrollIntoView({ block: "start" });
      content.scrollTop = Math.max(0, content.scrollTop - 20);
    });
  }
}

listen("plume://key-captured", async (event) => {
  if (!capturing || !root) {
    return;
  }
  const name = event.payload?.name;
  capturing = false;
  try {
    await invoke("set_dictation_key_capture", { on: false });
  } catch {
    /* déjà désarmé */
  }
  if (name) {
    await save({ dictationKey: name });
  }
  renderShortcut();
});

async function ensureMounted() {
  const node = CONTENT?.querySelector('[data-page="settings"]');
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
