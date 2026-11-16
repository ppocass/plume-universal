// Accueil (§2.2) : en-tête, configuration manquante, gestes, chiffres, dernière
// dictée. Les valeurs viennent de `totals`, `get_history`, `system_check` et
// `engine_status` ; en démonstration (`PLUME_DEMO`) elles sont simulées.

import { t, currentLanguage } from "../js/i18n.js";
import { invoke } from "../js/tauri.js";
import { hydrateIcons } from "../js/icons.js";

const ROOT = "home";
const content = document.getElementById("content");
const locale = currentLanguage() === "fr" ? "fr-FR" : "en-US";
const numberFormat = new Intl.NumberFormat(locale);

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

function icon(name, className = "icon") {
  const node = el("span", className);
  node.dataset.icon = name;
  node.setAttribute("aria-hidden", "true");
  return node;
}

function parseDemo(value) {
  if (!value || !value.startsWith("page=")) {
    return null;
  }
  const state = value.slice("page=".length);
  return { page: state.split("-")[0], state };
}

async function readDemo() {
  try {
    const env = await invoke("ui_env");
    return parseDemo(env?.demo);
  } catch {
    return null;
  }
}

function formatNumber(value) {
  return numberFormat.format(Number.isFinite(value) ? value : 0);
}

/** Le libellé du raccourci, comme `Shortcut.label` sur macOS. */
function shortcutLabel(name) {
  const key = String(name || "")
    .toLowerCase()
    .replace(/[-_\s]/g, "");
  switch (key) {
    case "altleft":
    case "alt":
    case "option":
    case "optionleft":
      return t("left_opt");
    case "altright":
    case "altr":
    case "altgr":
    case "optionright":
      return t("right_opt");
    case "controlleft":
    case "ctrll":
      return t("left_ctrl");
    case "controlright":
    case "ctrlr":
      return t("right_ctrl");
    case "shiftleft":
    case "shiftl":
      return t("left_shift");
    case "shiftright":
    case "shiftr":
      return t("right_shift");
    case "metaleft":
    case "meta":
    case "cmd":
    case "super":
      return t("left_cmd");
    case "metaright":
    case "cmdr":
    case "superr":
      return t("right_cmd");
    default: {
      const match = key.match(/^f(\d{1,2})$/);
      return match ? `F${match[1]}` : t("left_opt");
    }
  }
}

function demoLatest() {
  return {
    id: "demo-latest",
    date: new Date(Date.now() - 42 * 60 * 1000).toISOString(),
    text: "Let's meet at the studio at 9:30 — bring the microphone and the notes from yesterday's session.",
    raw: "So, um, let's meet at the studio at 9:30, bring the microphone and the notes from yesterday's session.",
    polished: true,
    pasted: true,
    appName: "Notes",
    audioSeconds: 8.4,
    failed: false,
    audioFile: null,
    leet: false,
  };
}

async function loadData(state) {
  if (state) {
    const empty = state === "home-empty";
    return {
      prefs: { dictationKey: "AltLeft" },
      totals: empty
        ? { today: 0, last7Days: 0, dictations: 0, words: 0 }
        : { today: 342, last7Days: 1287, dictations: 1846, words: 163204 },
      history: empty ? [] : [demoLatest()],
      check: empty
        ? { sessionType: "x11", pasteTool: false, pasteToolName: "", micCount: 0 }
        : { sessionType: "x11", pasteTool: true, pasteToolName: "wl-copy", micCount: 2 },
      engine: empty
        ? { state: "missing", modelId: "parakeet-v3" }
        : { state: "ready", modelId: "parakeet-v3" },
    };
  }

  const safe = (command) => invoke(command).catch(() => null);
  const [prefs, totals, history, check, engine] = await Promise.all([
    safe("get_prefs"),
    safe("totals"),
    safe("get_history"),
    safe("system_check"),
    safe("engine_status"),
  ]);
  return {
    prefs,
    totals: totals || { today: 0, last7Days: 0, dictations: 0, words: 0 },
    history: Array.isArray(history) ? history : [],
    check,
    engine,
  };
}

function renderHeader(root) {
  const header = el("header", "home-header");
  const glyph = el("img", "home-glyph");
  glyph.src = new URL("../assets/app-icon.png", import.meta.url).href;
  glyph.alt = "";
  glyph.width = 58;
  glyph.height = 58;

  const heading = el("div", "home-heading");
  const title = el("h1", "home-title", "Plume");
  const subtitle = el("p", "home-subtitle", t("voice_dictation_in_every_app"));
  heading.append(title, subtitle);
  header.append(glyph, heading);
  root.append(header);
}

function setupRow(name, detail, ok, iconName) {
  const row = el("div", "setup-row");
  const status = icon(iconName, ok ? "icon setup-status is-ok" : "icon setup-status");
  const copy = el("div", "setup-copy");
  copy.append(el("span", "setup-name", name), el("span", "setup-detail", detail));
  row.append(status, copy);
  return row;
}

function renderSetup(root, check, engine) {
  if (!check && !engine) {
    return;
  }
  const micOk = Boolean(check && check.micCount > 0);
  const pasteOk = Boolean(check && check.pasteTool);
  const modelOk = Boolean(engine && (engine.state === "ready" || engine.state === "loaded"));
  const modelDownloading = Boolean(engine && engine.state === "downloading");
  if (micOk && pasteOk && modelOk) {
    return;
  }

  const card = el("section", "setup-card");
  const title = el("h2", "setup-title");
  title.append(icon("triangle-alert", "icon"), el("span", "", t("setup_needed")));
  card.append(title);

  card.append(
    setupRow(
      t("microphone"),
      micOk ? t("to_hear_your_voice") : t("no_microphone_found"),
      micOk,
      micOk ? "circle-check" : "circle-dashed",
    ),
  );
  card.append(
    setupRow(
      t("paste_tool"),
      t("to_paste_text_into_your_apps"),
      pasteOk,
      pasteOk ? "circle-check" : "circle-dashed",
    ),
  );
  card.append(
    setupRow(
      t("transcription_model"),
      modelOk
        ? t("to_transcribe_your_voice")
        : modelDownloading
          ? t("model_downloading")
          : t("model_not_downloaded"),
      modelOk,
      modelOk ? "circle-check" : "circle-dashed",
    ),
  );

  root.append(card);
}

function keycap(text) {
  return el("span", "keycap", text);
}

function gestureRow(keys, label, withDivider) {
  const row = el("div", "gesture-row");
  const keyStack = el("div", "gesture-keys");
  for (const key of keys) {
    keyStack.append(keycap(key));
  }
  row.append(keyStack, el("span", "gesture-label", label));
  if (withDivider) {
    const divider = el("div", "gesture-divider");
    const wrapper = el("div");
    wrapper.append(row, divider);
    return wrapper;
  }
  return row;
}

function renderGestures(root, prefs) {
  const shortcut = shortcutLabel(prefs?.dictationKey);
  const section = el("section", "home-section");
  section.append(el("h2", "home-section-title", t("how_to_use")));

  const card = el("div", "gesture-card");
  const first = gestureRow([shortcut], t("start_dictation"), true);
  const second = gestureRow([shortcut], t("stop_and_insert_text"), true);
  const third = gestureRow([shortcut, "×2"], t("cancel"), false);
  card.append(first, second, third);
  section.append(card);
  root.append(section);
}

function statTile(value, label, iconName) {
  const tile = el("div", "stat-tile");
  tile.append(
    icon(iconName, "icon stat-icon"),
    el("div", "stat-value", formatNumber(value)),
    el("div", "stat-label", label),
  );
  return tile;
}

function renderStats(root, totals) {
  const row = el("div", "stats-row");
  row.append(
    statTile(totals.today, t("words_today"), "case-upper"),
    statTile(totals.last7Days, t("words_in_7_days"), "calendar"),
    statTile(totals.dictations, t("dictations"), "audio-waveform"),
  );
  root.append(row);
}

function formatTime(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" }).format(date);
}

function historyMeta(item) {
  const meta = el("div", "history-meta");
  const parts = [];
  const time = formatTime(item.date);
  if (time) {
    parts.push(time);
  }
  if (item.appName) {
    parts.push(item.appName);
  }
  if (!item.failed) {
    parts.push(t("count_words", { count: item.text.split(/\s+/).filter(Boolean).length }));
  }
  parts.forEach((part, index) => {
    if (index > 0) {
      meta.append(el("span", "dot", "·"));
    }
    meta.append(el("span", "", part));
  });
  if (!item.pasted && !item.failed) {
    meta.append(el("span", "tag is-accent", t("not_inserted")));
  }
  if (item.leet) {
    meta.append(el("span", "tag is-accent", "1337"));
  }
  if (item.polished) {
    meta.append(el("span", "tag", t("cleaned")));
  }
  if (item.failed) {
    meta.append(el("span", "tag is-orange", t("failed")));
  }
  return meta;
}

function copyButton(item, onCopied) {
  const button = el("button", "row-action");
  button.type = "button";
  button.title = t("copy");
  button.append(icon("copy"));
  button.addEventListener("click", async (event) => {
    event.stopPropagation();
    try {
      await invoke("copy_text", { text: item.text });
    } catch (error) {
      console.warn("copie impossible :", error);
    }
    button.classList.add("is-done");
    button.textContent = "";
    button.append(icon("check"));
    await hydrateIcons(button);
    onCopied?.();
  });
  return button;
}

function renderLatest(root, history) {
  const latest = history.find((item) => !item.failed);
  if (!latest) {
    return;
  }

  const block = el("section", "latest-block");
  const head = el("div", "latest-head");
  head.append(el("h2", "latest-title", t("latest")));
  const link = el("button", "latest-link");
  link.type = "button";
  link.append(el("span", "", t("history")), icon("chevron-right"));
  link.addEventListener("click", () => {
    document.querySelector('.sidebar-row[data-page="history"]')?.click();
  });
  head.append(link);

  const card = el("div", "latest-card");
  const row = el("div", "history-row");
  const main = el("div", "history-main");

  const isLong = String(latest.text).length > 420;
  let expanded = false;
  const text = el("div", "history-text");
  text.textContent = latest.text;
  const more = el("button", "history-link");
  more.type = "button";
  const update = () => {
    text.classList.toggle("is-clamped", isLong && !expanded);
    more.hidden = !isLong;
    more.textContent = expanded ? t("show_less") : t("show_all");
  };
  text.addEventListener("click", () => {
    if (isLong && !expanded) {
      expanded = true;
      update();
    }
  });
  more.addEventListener("click", () => {
    expanded = !expanded;
    update();
  });
  update();

  main.append(text, more, historyMeta(latest));
  row.append(main, copyButton(latest));
  card.append(row);
  block.append(head, card);
  root.append(block);
}

async function renderPage(root, state) {
  root.textContent = "";
  const data = await loadData(state);

  const layout = el("div", "home-layout");
  renderHeader(layout);
  renderSetup(layout, data.check, data.engine);
  renderGestures(layout, data.prefs);
  renderStats(layout, data.totals);
  renderLatest(layout, data.history);
  root.append(layout);

  await hydrateIcons(root);
}

function mount(demoState) {
  const root = document.querySelector(`[data-page-root="${ROOT}"]`);
  if (!root || root.dataset.mounted === "true") {
    return;
  }
  root.dataset.mounted = "true";
  const state = demoState && demoState.page === ROOT ? demoState.state : null;
  renderPage(root, state).catch((error) => {
    console.error("accueil :", error);
    root.dataset.mounted = "";
  });
}

const demo = await readDemo();
const observer = new MutationObserver(() => mount(demo));
observer.observe(content, { childList: true });

if (demo && demo.page !== ROOT) {
  document.querySelector(`.sidebar-row[data-page="${demo.page}"]`)?.click();
} else {
  mount(demo);
}
