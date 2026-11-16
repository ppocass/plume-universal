// L'assistant de premier lancement (Linux/Windows), spec §2.8 : huit étapes,
// le pied commun (Passer/Retour · points · Continuer) et les transitions
// glissées de SwiftUI. Les réglages passent par les commandes de docs/ipc.md.
//
// `PLUME_DEMO=step=N` (rendu par `ui_env`) joue un état de démonstration avec
// des données factices, pour les captures : "step=3-missing", "step=3-wayland",
// "step=6-downloading".

import { applyI18n, initI18n, t } from "./i18n.js";
import { hydrateIcons } from "./icons.js";
import { initTheme } from "./theme.js";
import { closeCurrentWindow, hasTauri, invoke, listen } from "./tauri.js";

const STEPS = [
  "welcome",
  "microphone",
  "paste",
  "shortcut",
  "language",
  "model",
  "cleanup",
  "done",
];

const STEP_ICONS = {
  microphone: "mic",
  paste: "clipboard",
  shortcut: "keyboard",
  language: "globe",
  model: "cpu",
  cleanup: "sparkles",
};

// Le ressort de §2.8 : response 0,42 / damping 0,88.
const SPRING = "cubic-bezier(0.34, 1.14, 0.64, 1)";

const DEMO_PREFS = {
  appLanguage: "en",
  appearance: "system",
  language: "auto",
  dictationKey: "AltLeft",
  leetEnabled: false,
  leetKey: "Super",
  micDevice: null,
  model: "parakeet-v3",
  polishService: "groq",
  polishCustomUrl: "",
  polishModel: "",
  polishApiKey: "",
  polishEnabled: true,
  onboardingDone: false,
};

const DEMO_MICS = [
  "Built-in Audio Analog Stereo",
  "USB Headset Mono Hifi Audio",
];

const DEMO_MODELS = [
  {
    id: "parakeet-v3",
    family: "parakeet",
    name: "Parakeet TDT 0.6B v3",
    sizeMb: 640,
    languages: "25_european_languages",
    speed: 5,
    accuracy: 5,
    descriptionKey: "the_most_accurate_for_european_languages_and_very_fast",
    installed: false,
    downloading: false,
    progress: 0,
    recommended: true,
    active: true,
  },
  {
    id: "whisper-small",
    family: "whisper",
    name: "Whisper Small",
    sizeMb: 358,
    languages: "99_languages",
    speed: 2,
    accuracy: 3,
    descriptionKey: "light_whisper_99_languages_and_it_uses_your_vocabulary",
    installed: false,
    downloading: false,
    progress: 0,
    recommended: false,
    active: false,
  },
  {
    id: "whisper-base",
    family: "whisper",
    name: "Whisper Base",
    sizeMb: 153,
    languages: "99_languages",
    speed: 3,
    accuracy: 2,
    descriptionKey: "light_whisper_99_languages_and_it_uses_your_vocabulary",
    installed: false,
    downloading: false,
    progress: 0,
    recommended: false,
    active: false,
  },
];

const state = {
  prefs: null,
  demo: null,
  variant: "",
  index: 0,
  forward: true,
  sys: null,
  mics: [],
  models: [],
  rec: null,
  chosenModel: null,
  progress: null,
  downloading: null,
  downloadError: null,
  capturing: false,
  micError: null,
  micTarget: 0,
  micLevel: 0,
  levelFrame: 0,
  levelStarted: 0,
  testResult: null,
  testing: false,
  polish: { service: "groq", url: "", model: "", key: "" },
  polishTimer: 0,
};

// --- Petits outils DOM -----------------------------------------------------

function h(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === null || value === undefined || value === false) {
      continue;
    }
    if (key === "class") {
      node.className = value;
    } else if (key === "text") {
      node.textContent = value;
    } else if (key.startsWith("on")) {
      node.addEventListener(key.slice(2).toLowerCase(), value);
    } else {
      node.setAttribute(key, value === true ? "" : String(value));
    }
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined) {
      continue;
    }
    node.append(child);
  }
  return node;
}

const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

function modelById(id) {
  return state.models.find((model) => model.id === id) ?? null;
}

function memoryGb(value) {
  const number = Number(value);
  if (!Number.isFinite(number) || number <= 0) {
    return "—";
  }
  return String(Math.round(number));
}

function sizeLabel(model) {
  if (!model) {
    return "";
  }
  return `${model.sizeMb} ${t("mb")}`;
}

const SPEED_WORDS = ["slow", "slow", "fast", "fast", "very_fast"];
const ACCURACY_WORDS = ["good", "good", "very_good", "very_good", "excellent"];

function modelLabel(model) {
  let label = `${model.name} · ${sizeLabel(model)}`;
  if (model.recommended) {
    label += ` · ${t("recommended")}`;
  } else if (model.installed && model.family !== "cloud") {
    label += ` · ${t("already_here")}`;
  }
  return label;
}

const KEY_LABELS = {
  altl: "key_left_alt",
  altleft: "key_left_alt",
  alt: "key_left_alt",
  option: "key_left_alt",
  altr: "key_right_alt",
  altright: "key_right_alt",
  altgr: "key_right_alt",
  ctrll: "key_left_ctrl",
  controlleft: "key_left_ctrl",
  ctrlr: "key_right_ctrl",
  controlright: "key_right_ctrl",
  shiftl: "key_left_shift",
  shiftleft: "key_left_shift",
  shiftr: "key_right_shift",
  shiftright: "key_right_shift",
  super: "key_super",
  metaleft: "key_super",
  meta: "key_super",
  cmd: "key_super",
  superr: "key_right_super",
  metaright: "key_right_super",
};

function keyLabel(name) {
  const raw = String(name ?? "").trim();
  if (!raw) {
    return t("key_left_alt");
  }
  const nameKey = KEY_LABELS[raw.toLowerCase()];
  if (nameKey) {
    return t(nameKey);
  }
  const fKey = /^f(\d{1,2})$/i.exec(raw);
  if (fKey) {
    return `F${fKey[1]}`;
  }
  return raw;
}

// --- Réglages --------------------------------------------------------------

async function savePrefs(patch) {
  state.prefs = { ...state.prefs, ...patch };
  if (!hasTauri()) {
    return;
  }
  try {
    await invoke("set_prefs", { patch });
  } catch (error) {
    console.warn("réglages :", error);
  }
}

function applyTheme(appearance) {
  if (appearance === "dark" || appearance === "light") {
    document.documentElement.dataset.theme = appearance;
  } else {
    delete document.documentElement.dataset.theme;
  }
}

function spokenLanguageForRecommendation() {
  const language = state.prefs?.language ?? "auto";
  if (language && language !== "auto") {
    return language;
  }
  // « détection automatique » : la langue de l'interface est une des 25 de
  // Parakeet, elle sert de guide à la recommandation.
  return state.prefs?.appLanguage === "fr" ? "fr" : "en";
}

// --- Données (factices seulement en démonstration) ------------------------

function demoSystemCheck(variant) {
  if (variant === "missing") {
    return { sessionType: "x11", pasteTool: false, pasteToolName: "", micCount: 2 };
  }
  if (variant === "wayland") {
    return { sessionType: "wayland", pasteTool: false, pasteToolName: "", micCount: 2 };
  }
  return { sessionType: "x11", pasteTool: true, pasteToolName: "xdotool", micCount: 2 };
}

async function loadData() {
  if (state.demo) {
    state.sys = demoSystemCheck(state.variant);
    state.mics = [...DEMO_MICS];
    state.models = DEMO_MODELS.map((model) => ({ ...model }));
    state.rec = {
      id: "parakeet-v3",
      reasonKey: "the_most_accurate_for_european_languages_and_very_fast",
      machine: {
        cpu: "AMD Ryzen 5 5600X 6-Core Processor",
        physicalCores: 6,
        ramTotalGb: 16,
        ramAvailableGb: 11.4,
        avx2: true,
        nvidia: false,
      },
    };
    if (state.variant === "downloading") {
      const model = state.models[0];
      model.installed = false;
      model.downloading = true;
      model.progress = 0.42;
      state.progress = {
        id: model.id,
        doneBytes: Math.round(model.sizeMb * 0.42) * 1024 * 1024,
        totalBytes: model.sizeMb * 1024 * 1024,
      };
      state.downloading = model.id;
    }
    return;
  }

  // La recommandation d'abord : `list_models` marque « recommended » avec ce
  // qu'elle a enregistré, donc elle doit passer après elle.
  const settled = await Promise.allSettled([
    invoke("system_check"),
    invoke("list_mics"),
    invoke("recommend_model", { spokenLanguage: spokenLanguageForRecommendation() }),
  ]);
  const [sys, mics, rec] = settled.map((result) =>
    result.status === "fulfilled" ? result.value : null,
  );
  state.sys = sys;
  state.mics = Array.isArray(mics) ? mics : [];
  state.rec = rec;
  try {
    const models = await invoke("list_models");
    state.models = Array.isArray(models) ? models : [];
  } catch (error) {
    console.warn("modèles :", error);
    state.models = [];
  }
}

// --- Contrôles -------------------------------------------------------------

function chevronUpDown() {
  const namespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(namespace, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", "popup-chevron");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  for (const d of ["m7 15 5 5 5-5", "m7 9 5-5 5 5"]) {
    const path = document.createElementNS(namespace, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}

function segmented(options, value, onPick) {
  const box = h("div", { class: "segmented", role: "radiogroup" });
  for (const option of options) {
    const selected = option.value === value;
    const button = h("button", {
      type: "button",
      class: `segmented-item${selected ? " is-selected" : ""}`,
      role: "radio",
      "aria-checked": selected ? "true" : "false",
      text: option.label,
    });
    button.addEventListener("click", () => {
      if (!selected) {
        onPick(option.value);
      }
    });
    box.append(button);
  }
  return box;
}

function popup(options, value, onPick) {
  const root = h("div", { class: "popup" });
  const label = h("span", { class: "popup-label" });
  const button = h(
    "button",
    { type: "button", class: "popup-button", "aria-haspopup": "listbox", "aria-expanded": "false" },
    label,
    chevronUpDown(),
  );
  let current = value;
  let menu = null;
  const paint = () => {
    const selected = options.find((option) => option.value === current) ?? options[0];
    label.textContent = selected?.label ?? "";
  };
  paint();

  const close = () => {
    if (!menu) {
      return;
    }
    menu.remove();
    menu = null;
    button.classList.remove("is-open");
    button.setAttribute("aria-expanded", "false");
    document.removeEventListener("pointerdown", outside, true);
  };
  const outside = (event) => {
    if (!root.contains(event.target)) {
      close();
    }
  };

  button.addEventListener("click", () => {
    if (menu) {
      close();
      return;
    }
    menu = h("div", { class: "popup-menu", role: "listbox" });
    for (const option of options) {
      const selected = option.value === current;
      const item = h("button", {
        type: "button",
        class: `popup-item${selected ? " is-selected" : ""}`,
        role: "option",
        "aria-selected": selected ? "true" : "false",
        text: option.label,
      });
      item.addEventListener("click", (event) => {
        event.stopPropagation();
        current = option.value;
        paint();
        close();
        onPick(option.value);
      });
      menu.append(item);
    }
    root.append(menu);
    button.classList.add("is-open");
    button.setAttribute("aria-expanded", "true");
    document.addEventListener("pointerdown", outside, true);
  });

  root.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      close();
    }
  });

  root.append(button);
  return root;
}

function radioGroup(options, value, onPick) {
  const box = h("div", { class: "radios", role: "radiogroup" });
  for (const option of options) {
    const selected = option.value === value;
    const button = h(
      "button",
      {
        type: "button",
        class: `radio${selected ? " is-selected" : ""}`,
        role: "radio",
        "aria-checked": selected ? "true" : "false",
      },
      h("span", { class: "radio-dot" }),
      h("span", { text: option.label }),
    );
    button.addEventListener("click", () => {
      if (!selected) {
        onPick(option.value);
      }
    });
    box.append(button);
  }
  return box;
}

function progressBar() {
  return h(
    "div",
    { class: "progress" },
    h("div", { class: "progress-fill" }),
  );
}

function progressPercent(progress) {
  const total = Number(progress?.totalBytes) || 0;
  const done = Number(progress?.doneBytes) || 0;
  if (total <= 0) {
    return 0;
  }
  return clamp((done / total) * 100, 0, 100);
}

function progressText(model, progress) {
  const total = Number(progress?.totalBytes) || 0;
  const done = Number(progress?.doneBytes) || 0;
  const percent = total > 0 ? clamp(Math.round((done / total) * 100), 0, 100) : 0;
  return t("downloading_model_percent_of_size", {
    model: model?.name ?? "",
    percent,
    size: sizeLabel(model),
  });
}

// --- Pages -----------------------------------------------------------------

function stepIcon(name) {
  if (name === "welcome" || name === "done") {
    return h("img", {
      class: "step-app",
      src: "./assets/app-icon.png",
      width: "88",
      height: "88",
      alt: "",
    });
  }
  return h(
    "div",
    { class: "step-icon" },
    h("span", { class: "step-glyph", "data-icon": STEP_ICONS[name] }),
  );
}

function stepTitle(name) {
  switch (name) {
    case "welcome":
      return t("welcome_to_plume");
    case "microphone":
      return t("microphone");
    case "paste":
      return t("paste");
    case "shortcut":
      return t("shortcut");
    case "language":
      return t("spoken_language");
    case "model":
      return t("voice_model");
    case "cleanup":
      return t("text_cleanup");
    default:
      return t("you_re_all_set");
  }
}

function stepSubtitle(name) {
  switch (name) {
    case "welcome":
      return t("voice_dictation_in_every_app_2");
    case "microphone":
      return t("to_hear_your_voice_it_stays_on_your_computer");
    case "paste":
      return t("for_pasting_plume_uses_a_small_system_tool");
    case "shortcut":
      return t("press_once_to_start_once_to_stop");
    case "language":
      return t("the_language_you_dictate_in");
    case "model":
      return t("the_recommended_model_is_already_selected_you_can_change_it_in_models");
    case "cleanup":
      return t("fixes_punctuation_and_filler_words_optional_your_own_api_key");
    default:
      return t("press_shortcut_to_dictate", { shortcut: keyLabel(state.prefs.dictationKey) });
  }
}

function contentWelcome(container) {
  const grid = h("div", { class: "welcome-grid" });
  grid.append(
    h("span", { class: "welcome-label", text: t("language") }),
    segmented(
      // Chaque langue s'écrit dans sa propre langue (« Français », « English »),
      // comme `AppLanguage.label` côté Swift : pas de clé de traduction.
      [
        { value: "fr", label: "Français" },
        { value: "en", label: "English" },
      ],
      state.prefs.appLanguage ?? "en",
      pickLanguage,
    ),
    h("span", { class: "welcome-label", text: t("appearance") }),
    segmented(
      [
        { value: "system", label: t("system") },
        { value: "light", label: t("light") },
        { value: "dark", label: t("dark") },
      ],
      state.prefs.appearance ?? "system",
      pickAppearance,
    ),
  );
  container.append(grid);
}

function contentMicrophone(container) {
  const wrap = h("div", { class: "mic" });
  const options = [{ value: "", label: t("system_default") }];
  for (const mic of state.mics) {
    options.push({ value: mic, label: mic });
  }
  wrap.append(popup(options, state.prefs.micDevice ?? "", pickMicrophone));

  const meter = h("div", { class: "meter", role: "img", "aria-label": t("level") });
  for (let index = 0; index < 13; index += 1) {
    meter.append(h("span", { class: "meter-bar" }));
  }
  wrap.append(meter);

  const hint = state.micError
    ? state.micError
    : state.mics.length === 0
      ? t("no_microphone_found")
      : t("speak_to_check_the_level");
  wrap.append(h("p", { class: "mic-hint", text: hint }));
  container.append(wrap);
}

function pasteCommand(command) {
  const copy = h("button", { type: "button", class: "command-copy", text: t("copy") });
  copy.addEventListener("click", async () => {
    await copyText(command);
    copy.textContent = t("copied");
    setTimeout(() => {
      copy.textContent = t("copy");
    }, 1200);
  });
  return h(
    "div",
    { class: "command-box" },
    h("code", { class: "command-text", text: command }),
    copy,
  );
}

function contentPaste(container) {
  const wrap = h("div", { class: "paste" });
  const sys = state.sys;

  if (sys && sys.sessionType === "wayland") {
    wrap.append(
      h(
        "div",
        { class: "paste-status is-warning" },
        h("span", { class: "paste-status-icon", "data-icon": "triangle-alert" }),
        h("span", { text: t("wayland_pasting_needs_ydotool") }),
      ),
      h("p", { class: "paste-note", text: t("x11_sessions_work_best") }),
    );
    if (!sys.pasteTool) {
      wrap.append(pasteCommand("sudo apt install ydotool"));
    }
  } else if (sys?.pasteTool) {
    wrap.append(
      h(
        "div",
        { class: "paste-status is-ok" },
        h("span", { class: "paste-status-icon", "data-icon": "circle-check" }),
        h("span", { text: t("plume_can_paste_where_your_cursor_is") }),
      ),
    );
  } else {
    wrap.append(
      h(
        "div",
        { class: "paste-status is-warning" },
        h("span", { class: "paste-status-icon", "data-icon": "triangle-alert" }),
        h("span", { text: t("the_paste_tool_is_missing") }),
      ),
      h("p", { class: "paste-note", text: t("run_this_command_then_check_again") }),
      pasteCommand("sudo apt install xdotool"),
    );
  }

  if (!sys || sys.sessionType === "wayland" || !sys.pasteTool) {
    const recheck = h("button", { type: "button", class: "btn-soft", text: t("re_check") });
    recheck.addEventListener("click", recheckPaste);
    wrap.append(recheck);
  }
  container.append(wrap);
}

function contentShortcut(container) {
  const wrap = h("div", { class: "shortcut" });
  const keycap = h("div", {
    class: "keycap",
    "data-capturing": state.capturing ? "true" : "false",
    text: state.capturing ? t("press_a_key") : keyLabel(state.prefs.dictationKey),
  });
  const change = h("button", {
    type: "button",
    class: "btn-soft",
    text: state.capturing ? t("press_a_key") : t("change"),
  });
  change.addEventListener("click", () => {
    if (state.capturing) {
      cancelCapture();
    } else {
      beginCapture();
    }
  });
  const gestures = h(
    "div",
    { class: "gestures" },
    h("span", { text: t("double_tap_quickly_to_cancel") }),
    h("span", { text: t("hold_key_at_stop_for_1337", { key: keyLabel(state.prefs.leetKey) }) }),
  );
  wrap.append(keycap, change, gestures);
  container.append(wrap);
}

function contentLanguage(container) {
  const options = [
    { value: "auto", label: t("automatic_detection_recommended") },
    { value: "fr", label: t("french") },
    { value: "en", label: t("english") },
    { value: "es", label: t("spanish") },
    { value: "de", label: t("german") },
    { value: "it", label: t("italian") },
    { value: "pt", label: t("portuguese") },
  ];
  container.append(radioGroup(options, state.prefs.language ?? "auto", pickSpokenLanguage));
}

function contentModel(container) {
  const wrap = h("div", { class: "model" });
  const chosen =
    modelById(state.chosenModel) ?? modelById(state.prefs.model) ?? state.models[0] ?? null;

  if (state.models.length > 0) {
    wrap.append(
      popup(
        state.models.map((model) => ({ value: model.id, label: modelLabel(model) })),
        chosen?.id,
        pickModel,
      ),
    );
  }

  if (chosen) {
    if (chosen.descriptionKey) {
      wrap.append(h("p", { class: "model-summary", text: t(chosen.descriptionKey) }));
    }
    const facts = [
      t(chosen.languages),
      t("speed_speed", { speed: t(SPEED_WORDS[clamp(chosen.speed - 1, 0, 4)]) }),
      t("accuracy_accuracy", {
        accuracy: t(ACCURACY_WORDS[clamp(chosen.accuracy - 1, 0, 4)]),
      }),
    ];
    wrap.append(h("p", { class: "model-facts", text: facts.join(" · ") }));

    const machine = state.rec?.machine;
    if (machine) {
      wrap.append(
        h("p", {
          class: "model-machine",
          text: t("this_computer_chip_memory_gb_of_memory", {
            cpu: machine.cpu,
            memory: memoryGb(machine.ramTotalGb),
          }),
        }),
      );
    }

    if (!chosen.installed) {
      const download = h("div", { class: "download" });
      const button = h("button", { type: "button", class: "btn-soft", text: t("download") });
      button.disabled = Boolean(state.downloading);
      button.addEventListener("click", () => startDownload(chosen.id));
      download.append(button);

      if (state.progress && state.progress.id === chosen.id) {
        const bar = progressBar();
        bar.querySelector(".progress-fill").style.width = `${progressPercent(state.progress)}%`;
        download.append(
          bar,
          h("p", { class: "progress-text", text: progressText(chosen, state.progress) }),
        );
      }
      wrap.append(download);
    }
  }

  container.append(wrap);
}

function textRow(label, value, placeholder, onInput, type = "text") {
  const input = h("input", {
    class: "field",
    type,
    value: value ?? "",
    placeholder: placeholder ?? "",
    spellcheck: "false",
    autocomplete: "off",
  });
  input.addEventListener("input", () => onInput(input.value));
  return h(
    "div",
    { class: "form-row" },
    h("span", { class: "form-label", text: label }),
    h("div", { class: "form-control" }, input),
  );
}

function defaultPolishModel() {
  switch (state.polish.service) {
    case "groq":
      return "openai/gpt-oss-120b";
    case "openai":
      return "gpt-4o-mini";
    default:
      return state.polish.model || "";
  }
}

function keyPage(service) {
  if (service === "groq") {
    return { url: "https://console.groq.com/keys", label: "Groq" };
  }
  if (service === "openai") {
    return { url: "https://platform.openai.com/api-keys", label: "OpenAI" };
  }
  return null;
}

function testResultText() {
  if (!state.testResult) {
    return "";
  }
  if (state.testResult.ok) {
    return t("ok_seconds", { seconds: (Number(state.testResult.ms) / 1000).toFixed(2) });
  }
  return state.testResult.error || t("connection_failed");
}

function contentCleanup(container) {
  const form = h("div", { class: "form" });
  const section = h("div", { class: "form-section" });

  const service = h(
    "div",
    { class: "form-row" },
    h("span", { class: "form-label", text: t("service") }),
    h(
      "div",
      { class: "form-control" },
      popup(
        [
          { value: "server", label: t("personal_server") },
          { value: "groq", label: "Groq" },
          { value: "openai", label: "OpenAI" },
          { value: "custom", label: t("other_openai_compatible") },
        ],
        state.polish.service,
        pickPolishService,
      ),
    ),
  );

  const url = textRow(t("url"), state.polish.url || polishBaseUrl(), "https://…/v1", (value) => {
    state.polish.url = value;
    schedulePolishSave();
  });
  const model = textRow(t("model"), state.polish.model || defaultPolishModel(), "", (value) => {
    state.polish.model = value;
    schedulePolishSave();
  });
  const key = textRow(
    t("api_key"),
    state.polish.key,
    "",
    (value) => {
      state.polish.key = value;
      schedulePolishSave();
    },
    "password",
  );

  const result = h("span", {
    class: "test-result",
    text: state.testing ? t("testing") : testResultText(),
  });
  const test = h("button", {
    type: "button",
    class: "btn-soft",
    text: state.testing ? t("testing") : t("test"),
  });
  test.disabled = state.testing;
  test.addEventListener("click", runPolishTest);
  const connection = h(
    "div",
    { class: "form-row" },
    h("span", { class: "form-label", text: t("connection") }),
    h("div", { class: "form-control" }, result, test),
  );

  section.append(service, url, model, key, connection);

  const page = keyPage(state.polish.service);
  if (page) {
    section.append(
      h("a", {
        class: "form-link",
        href: page.url,
        target: "_blank",
        rel: "noreferrer",
        text: t("create_a_service_key", { service: page.label }),
      }),
    );
  }

  form.append(section);
  container.append(form);
}

function contentDone(container) {
  const chosen =
    modelById(state.chosenModel) ?? modelById(state.prefs.model) ?? state.models[0] ?? null;
  const progress =
    state.progress && (!chosen || state.progress.id === chosen.id) ? state.progress : null;

  if (progress) {
    const bar = progressBar();
    bar.querySelector(".progress-fill").style.width = `${progressPercent(progress)}%`;
    container.append(
      h(
        "div",
        { class: "done-download" },
        bar,
        h("p", { class: "progress-text", text: progressText(chosen, progress) }),
      ),
    );
  } else if (state.downloadError) {
    container.append(h("p", { class: "done-error", text: state.downloadError }));
  }
}

// --- Modifications d'étape -------------------------------------------------

async function pickLanguage(value) {
  await savePrefs({ appLanguage: value });
  await initI18n(state.prefs);
  applyI18n();
  renderFooter();
  await renderPage(false);
}

async function pickAppearance(value) {
  await savePrefs({ appearance: value });
  applyTheme(value);
  await renderPage(false);
}

async function pickMicrophone(value) {
  await savePrefs({ micDevice: value || null });
  if (STEPS[state.index] === "microphone") {
    await stopMicTest();
    await startMicTest();
  }
}

async function pickSpokenLanguage(value) {
  await savePrefs({ language: value });
  await renderPage(false);
}

async function pickModel(value) {
  state.chosenModel = value;
  await renderPage(false);
}

async function pickPolishService(value) {
  state.polish.service = value;
  await commitCleanup();
  await renderPage(false);
}

async function runPolishTest() {
  if (state.testing) {
    return;
  }
  state.testing = true;
  state.testResult = null;
  await renderPage(false);
  try {
    if (hasTauri() && !state.demo) {
      state.testResult = await invoke("polish_test", {
        baseUrl: polishBaseUrl(),
        model: state.polish.model.trim() || defaultPolishModel(),
        apiKey: state.polish.key,
      });
    } else {
      state.testResult = { ok: true, ms: 420, sample: "", error: null };
    }
  } catch (error) {
    state.testResult = { ok: false, ms: 0, sample: "", error: String(error) };
  }
  state.testing = false;
  await renderPage(false);
}

function polishBaseUrl() {
  switch (state.polish.service) {
    case "groq":
      return "https://api.groq.com/openai/v1";
    case "openai":
      return "https://api.openai.com/v1";
    default:
      return state.polish.url.trim();
  }
}

function schedulePolishSave() {
  clearTimeout(state.polishTimer);
  state.polishTimer = setTimeout(() => commitCleanup(), 400);
}

async function commitCleanup() {
  await savePrefs({
    polishService: state.polish.service,
    polishCustomUrl: state.polish.url,
    polishModel: state.polish.model,
    polishApiKey: state.polish.key,
  });
}

async function copyText(text) {
  if (hasTauri()) {
    try {
      await invoke("copy_text", { text });
      return;
    } catch (error) {
      console.warn("copie :", error);
    }
  }
  if (navigator.clipboard) {
    try {
      await navigator.clipboard.writeText(text);
    } catch (error) {
      console.warn("copie :", error);
    }
  }
}

async function recheckPaste() {
  if (!hasTauri() || state.demo) {
    return;
  }
  try {
    state.sys = await invoke("system_check");
  } catch (error) {
    console.warn("système :", error);
  }
  await renderPage(false);
}

async function startDownload(id) {
  state.downloadError = null;
  state.downloading = id;
  const model = modelById(id);
  if (!state.progress || state.progress.id !== id) {
    state.progress = {
      id,
      doneBytes: 0,
      totalBytes: (model?.sizeMb ?? 0) * 1024 * 1024,
    };
  }
  await renderPage(false);
  if (hasTauri() && !state.demo) {
    try {
      await invoke("download_model", { id });
    } catch (error) {
      state.downloadError = String(error);
      state.downloading = null;
      await renderPage(false);
    }
  }
}

function updateProgressUi() {
  const fill = document.querySelector(".progress-fill");
  const text = document.querySelector(".progress-text");
  const model = modelById(state.progress?.id) ?? modelById(state.chosenModel);
  if (fill && state.progress) {
    fill.style.width = `${progressPercent(state.progress)}%`;
  }
  if (text && state.progress) {
    text.textContent = progressText(model, state.progress);
  }
}

// --- Capture de touche -----------------------------------------------------

async function beginCapture() {
  state.capturing = true;
  await renderPage(false);
  if (hasTauri() && !state.demo) {
    try {
      await invoke("set_dictation_key_capture", { on: true });
    } catch (error) {
      console.warn("capture :", error);
      state.capturing = false;
    }
  }
}

async function cancelCapture() {
  state.capturing = false;
  if (hasTauri() && !state.demo) {
    try {
      await invoke("set_dictation_key_capture", { on: false });
    } catch (error) {
      console.warn("capture :", error);
    }
  }
  await renderPage(false);
}

// --- Micro : niveau en direct ---------------------------------------------

function stopLevelLoop() {
  if (state.levelFrame) {
    cancelAnimationFrame(state.levelFrame);
    state.levelFrame = 0;
  }
}

function startLevelLoop() {
  stopLevelLoop();
  const meter = document.querySelector(".meter");
  if (!meter) {
    return;
  }
  const bars = [...meter.children];
  state.levelStarted = performance.now();
  let last = 0;
  const animate = (now) => {
    state.levelFrame = requestAnimationFrame(animate);
    if (now - last < 33) {
      return;
    }
    last = now;
    const seconds = (now - state.levelStarted) / 1000;
    const target = state.demo ? 0.6 + 0.18 * Math.sin(seconds * 1.6) : state.micTarget;
    state.micLevel += (target - state.micLevel) * 0.25;
    for (let index = 0; index < bars.length; index += 1) {
      const shimmer = 0.5 + 0.5 * Math.sin(7 * seconds + 1.3 * index);
      const height = Math.max(3, 3 + 15 * Math.pow(state.micLevel, 0.8) + 4 * shimmer);
      bars[index].style.height = `${height.toFixed(1)}px`;
    }
  };
  state.levelFrame = requestAnimationFrame(animate);
}

async function startMicTest() {
  state.micError = null;
  state.micTarget = 0;
  state.micLevel = 0;
  if (hasTauri() && !state.demo) {
    try {
      await invoke("mic_test_start", { device: state.prefs.micDevice ?? null });
    } catch (error) {
      console.warn("micro :", error);
      state.micError = t("microphone_unavailable");
      const hint = document.querySelector(".mic-hint");
      if (hint) {
        hint.textContent = state.micError;
      }
    }
  }
  startLevelLoop();
}

async function stopMicTest() {
  stopLevelLoop();
  if (hasTauri() && !state.demo) {
    try {
      await invoke("mic_test_stop");
    } catch (error) {
      console.warn("micro :", error);
    }
  }
}

// --- Pied et navigation ----------------------------------------------------

function renderFooter() {
  const left = document.getElementById("footer-left");
  const right = document.getElementById("footer-right");
  const dots = document.getElementById("footer-dots");
  if (!left || !right || !dots) {
    return;
  }

  left.replaceChildren();
  if (state.index === 0) {
    const skip = h("button", { type: "button", class: "footer-link", text: t("skip") });
    skip.addEventListener("click", skipAll);
    left.append(skip);
  } else {
    const back = h("button", { type: "button", class: "footer-link", text: t("back") });
    back.addEventListener("click", () => go(-1));
    left.append(back);
  }

  dots.replaceChildren();
  STEPS.forEach((_, index) => {
    dots.append(
      h("span", { class: `footer-dot${index === state.index ? " is-current" : ""}` }),
    );
  });

  right.replaceChildren();
  const primary = h("button", {
    type: "button",
    class: "button-prominent",
    text: STEPS[state.index] === "done" ? t("get_started") : t("continue"),
  });
  primary.addEventListener("click", () => {
    if (STEPS[state.index] === "done") {
      finish();
    } else {
      go(1);
    }
  });
  right.append(primary);
}

async function renderPage(animate) {
  const stage = document.getElementById("stage");
  if (!stage) {
    return;
  }
  const name = STEPS[state.index];
  const page = h("section", { class: "step" });
  page.append(h("div", { class: "step-spacer step-spacer-top" }));
  page.append(stepIcon(name));
  page.append(h("h1", { class: "step-title", text: stepTitle(name) }));
  page.append(h("p", { class: "step-subtitle", text: stepSubtitle(name) }));
  const content = h("div", { class: "step-content" });
  const builder = {
    welcome: contentWelcome,
    microphone: contentMicrophone,
    paste: contentPaste,
    shortcut: contentShortcut,
    language: contentLanguage,
    model: contentModel,
    cleanup: contentCleanup,
    done: contentDone,
  }[name];
  builder?.(content);
  page.append(content);
  page.append(h("div", { class: "step-spacer step-spacer-bottom" }));

  await hydrateIcons(page);

  const previous = stage.firstElementChild;
  stage.append(page);

  const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches ?? false;
  if (animate && previous && !reduced) {
    const direction = state.forward ? 1 : -1;
    const timing = { duration: 420, easing: SPRING, fill: "both" };
    previous.animate(
      [
        { transform: "translateX(0)", opacity: 1 },
        { transform: `translateX(${-direction * 100}%)`, opacity: 0 },
      ],
      timing,
    );
    page.animate(
      [
        { transform: `translateX(${direction * 100}%)`, opacity: 0 },
        { transform: "translateX(0)", opacity: 1 },
      ],
      timing,
    );
    window.setTimeout(() => previous.remove(), 430);
  } else if (previous) {
    previous.remove();
  }
}

async function commitStep(name) {
  if (name === "microphone") {
    await savePrefs({ micDevice: state.prefs.micDevice ?? null });
  }
  if (name === "model") {
    const id = state.chosenModel;
    if (!id) {
      return;
    }
    await savePrefs({ model: id });
    const model = modelById(id);
    if (hasTauri() && !state.demo) {
      try {
        await invoke("use_model", { id });
      } catch (error) {
        console.warn("modèle :", error);
      }
      if (model && !model.installed && !model.downloading) {
        try {
          await invoke("download_model", { id });
          state.downloading = id;
          state.progress = {
            id,
            doneBytes: 0,
            totalBytes: model.sizeMb * 1024 * 1024,
          };
        } catch (error) {
          console.warn("téléchargement :", error);
          state.downloadError = String(error);
        }
      }
    } else if (model && !model.installed && !state.progress) {
      state.downloading = id;
      state.progress = {
        id,
        doneBytes: Math.round(model.sizeMb * 0.42) * 1024 * 1024,
        totalBytes: model.sizeMb * 1024 * 1024,
      };
    }
  }
  if (name === "cleanup") {
    await commitCleanup();
  }
}

async function go(delta) {
  const next = state.index + delta;
  if (next < 0 || next >= STEPS.length) {
    return;
  }
  const leaving = STEPS[state.index];
  if (delta > 0) {
    await commitStep(leaving);
  }
  if (leaving === "microphone") {
    await stopMicTest();
  }
  state.forward = delta > 0;
  state.index = next;
  renderFooter();
  await renderPage(true);
  if (STEPS[state.index] === "microphone") {
    await startMicTest();
  }
}

async function skipAll() {
  let recommended = state.rec?.id ?? state.prefs.model ?? "parakeet-v3";
  if (!state.rec && hasTauri() && !state.demo) {
    try {
      state.rec = await invoke("recommend_model", {
        spokenLanguage: spokenLanguageForRecommendation(),
      });
      recommended = state.rec?.id ?? recommended;
    } catch (error) {
      console.warn("recommandation :", error);
    }
  }
  const patch = {
    appLanguage: "en",
    language: "auto",
    appearance: "light",
    pillStyle: "bottom",
    idleSeconds: 35,
    dictationKey: "AltLeft",
    micDevice: null,
    model: recommended,
  };
  await savePrefs(patch);
  applyTheme(patch.appearance);
  if (hasTauri() && !state.demo) {
    try {
      await invoke("use_model", { id: recommended });
    } catch (error) {
      console.warn("modèle :", error);
    }
  }
  await finish();
}

async function finish() {
  if (hasTauri() && !state.demo) {
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

// --- Démonstration ---------------------------------------------------------

function parseDemo(raw) {
  if (!raw) {
    return null;
  }
  const match = /^step=(\d+)(?:-(.+))?$/.exec(raw.trim());
  if (!match) {
    return null;
  }
  const number = Number(match[1]);
  return {
    index: clamp(number - 1, 0, STEPS.length - 1),
    variant: match[2] ?? "",
  };
}

// --- Démarrage -------------------------------------------------------------

function registerEvents() {
  if (!hasTauri()) {
    return;
  }
  listen("plume://mic-level", (event) => {
    const level = Number(event.payload?.level);
    if (Number.isFinite(level)) {
      state.micTarget = clamp(level, 0, 1);
    }
  });
  listen("plume://model-progress", (event) => {
    const payload = event.payload ?? {};
    if (!payload.id) {
      return;
    }
    state.progress = payload;
    state.downloading = payload.id;
    if (STEPS[state.index] === "model" || STEPS[state.index] === "done") {
      updateProgressUi();
    }
  });
  listen("plume://model-done", async (event) => {
    const payload = event.payload ?? {};
    state.downloading = null;
    state.progress = null;
    if (payload.ok) {
      try {
        state.models = await invoke("list_models");
      } catch (error) {
        console.warn("modèles :", error);
      }
    } else {
      state.downloadError = payload.error ?? t("download_failed");
    }
    if (STEPS[state.index] === "model" || STEPS[state.index] === "done") {
      await renderPage(false);
    }
  });
  listen("plume://key-captured", async (event) => {
    const name = String(event.payload?.name ?? "").trim();
    if (!name) {
      return;
    }
    state.capturing = false;
    await savePrefs({ dictationKey: name });
    try {
      await invoke("set_dictation_key_capture", { on: false });
    } catch (error) {
      console.warn("capture :", error);
    }
    if (STEPS[state.index] === "shortcut") {
      await renderPage(false);
    }
  });
}

async function boot() {
  let env = null;
  if (hasTauri()) {
    try {
      env = await invoke("ui_env");
    } catch (error) {
      console.warn("environnement :", error);
    }
  }
  state.demo = env?.demo ?? null;
  const demo = parseDemo(state.demo);
  if (demo) {
    state.index = demo.index;
    state.variant = demo.variant;
  }

  let prefs = null;
  if (state.demo) {
    prefs = { ...DEMO_PREFS };
  } else if (hasTauri()) {
    try {
      prefs = await invoke("get_prefs");
    } catch (error) {
      console.warn("réglages indisponibles :", error);
    }
  }
  state.prefs = { ...DEMO_PREFS, ...(prefs ?? {}) };
  state.polish = {
    service: state.prefs.polishService ?? "groq",
    url: state.prefs.polishCustomUrl ?? "",
    model: state.prefs.polishModel ?? "",
    key: state.prefs.polishApiKey ?? "",
  };

  await initI18n(state.prefs);
  const theme = await initTheme(state.prefs);
  if (state.demo) {
    // La capture montre le réglage d'apparence qui correspond au thème joué.
    state.prefs.appearance = theme === "dark" || theme === "light" ? theme : "system";
  }
  applyI18n();

  if (!state.demo) {
    registerEvents();
  }
  await loadData();
  state.chosenModel = state.rec?.id ?? state.prefs.model ?? state.models[0]?.id ?? null;

  renderFooter();
  await renderPage(false);
  if (STEPS[state.index] === "microphone") {
    await startMicTest();
  }

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.defaultPrevented) {
      return;
    }
    const target = event.target;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      return;
    }
    document.querySelector(".footer-side-right .button-prominent")?.click();
  });
}

boot().catch((error) => console.error("assistant :", error));
