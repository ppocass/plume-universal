// Historique (§2.3) : dictées groupées par jour, texte long replié, menu
// contextuel (copier, corriger un mot, 1337, réessayer, supprimer) et « Tout
// effacer » avec confirmation. Les données viennent de `get_history` ; en
// démonstration (`PLUME_DEMO`) elles sont simulées.

import { t, currentLanguage } from "../js/i18n.js";
import { keyLabel } from "../js/keys.js";
import { invoke } from "../js/tauri.js";
import { hydrateIcons } from "../js/icons.js";

const ROOT = "history";
const content = document.getElementById("content");
const locale = currentLanguage() === "fr" ? "fr-FR" : "en-US";

let items = [];
let search = "";
let expanded = new Set();
let listWrap = null;
let clearButton = null;
let menuEl = null;
let menuListener = null;
let menuKeyListener = null;
let isDemo = false;
let shortcut = "";

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

function demoItems() {
  const now = new Date();
  const at = (daysAgo, hours, minutes) => {
    const date = new Date(now);
    date.setDate(date.getDate() - daysAgo);
    date.setHours(hours, minutes, 0, 0);
    return date.toISOString();
  };
  const longText =
    "Here's the plan for the next release. We ship the beta on the eighteenth, keep the rollout to ten people for the first week, and watch the crash reports every morning. The onboarding copy still needs a pass from Marie, and the pricing page has two broken links that I noticed on Friday. I'll take the links and the pricing table, you take the onboarding and the changelog, and we'll review everything together on Thursday afternoon before the build goes out.";
  return [
    {
      id: "demo-1",
      date: at(0, 9, 41),
      text: "The meeting is moved to Thursday at two. I'll send the updated deck before then, and I'll add the budget figures we discussed on Monday.",
      raw: "So, um, the meeting is moved to Thursday at two. I'll send the updated deck before then, and, uh, I'll add the budget figures we discussed on Monday.",
      polished: true,
      pasted: true,
      appName: "Notes",
      audioSeconds: 11.2,
      failed: false,
      audioFile: null,
      leet: false,
    },
    {
      id: "demo-2",
      date: at(0, 8, 12),
      text: longText,
      raw: longText,
      polished: false,
      pasted: false,
      appName: null,
      audioSeconds: 38.6,
      failed: false,
      audioFile: null,
      leet: false,
    },
    {
      id: "demo-3",
      date: at(1, 18, 30),
      text: "7h4nk5 f0r 7h3 qu1ck 7urn4r0und — 7h3 numb3r5 l1n3 up w17h wh47 f1n4nc3 53n7 l457 w33k.",
      raw: "Thanks for the quick turnaround — the numbers line up with what finance sent last week.",
      polished: false,
      pasted: true,
      appName: "Mail",
      audioSeconds: 7.8,
      failed: false,
      audioFile: null,
      leet: true,
    },
    {
      id: "demo-4",
      date: at(1, 11, 5),
      text: "",
      raw: "",
      polished: false,
      pasted: false,
      appName: null,
      audioSeconds: 12.4,
      failed: true,
      audioFile: "demo.wav",
      leet: false,
    },
    {
      id: "demo-5",
      date: at(3, 15, 22),
      text: "Let's ship the beta on the 18th and keep the rollout to ten people.",
      raw: "Let's ship the beta on the 18th and keep the rollout to ten people.",
      polished: false,
      pasted: true,
      appName: "Slack",
      audioSeconds: 6.3,
      failed: false,
      audioFile: null,
      leet: false,
    },
  ];
}

async function loadData(state) {
  if (state) {
    return {
      history: state === "history-empty" ? [] : demoItems(),
      prefs: { dictationKey: "AltLeft" },
    };
  }
  const safe = (command) => invoke(command).catch(() => null);
  const [history, prefs] = await Promise.all([safe("get_history"), safe("get_prefs")]);
  return {
    history: Array.isArray(history) ? history : [],
    prefs,
  };
}

function formatTime(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" }).format(date);
}

function startOfDay(date) {
  const copy = new Date(date);
  copy.setHours(0, 0, 0, 0);
  return copy;
}

function dayLabel(date) {
  const today = startOfDay(new Date());
  const day = startOfDay(date);
  const difference = Math.round((today.getTime() - day.getTime()) / 86_400_000);
  if (difference === 0) {
    return t("today");
  }
  if (difference === 1) {
    return t("yesterday");
  }
  const raw = new Intl.DateTimeFormat(locale, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(date);
  return raw.charAt(0).toUpperCase() + raw.slice(1);
}

function wordCount(text) {
  return String(text || "").split(/\s+/).filter(Boolean).length;
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
    parts.push(t("count_words", { count: wordCount(item.text) }));
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

async function copyText(text) {
  try {
    await invoke("copy_text", { text });
  } catch (error) {
    console.warn("copie impossible :", error);
  }
}

function copyButton(item) {
  const button = el("button", "row-action");
  button.type = "button";
  button.title = t("copy");
  button.append(icon("copy"));
  button.addEventListener("click", async (event) => {
    event.stopPropagation();
    await copyText(item.text);
    button.classList.add("is-done");
    button.textContent = "";
    button.append(icon("check"));
    await hydrateIcons(button);
    setTimeout(() => {
      button.classList.remove("is-done");
      button.textContent = "";
      button.append(icon("copy"));
      hydrateIcons(button);
    }, 1200);
  });
  return button;
}

function replaceItem(updated) {
  items = items.map((item) => (item.id === updated.id ? updated : item));
}

async function retryItem(item, button = null) {
  if (isDemo) {
    return;
  }
  if (button) {
    button.disabled = true;
  }
  try {
    const updated = await invoke("retry_dictation", { id: item.id });
    replaceItem(updated);
    renderList();
  } catch (error) {
    console.warn("nouvel essai impossible :", error);
    if (button) {
      button.disabled = false;
    }
  }
}

async function deleteItem(item) {
  if (!isDemo) {
    try {
      await invoke("delete_dictation", { id: item.id });
    } catch (error) {
      console.warn("suppression impossible :", error);
      return;
    }
  }
  items = items.filter((entry) => entry.id !== item.id);
  renderList();
}

async function toggleLeet(item) {
  const toLeet = !item.leet;
  if (isDemo) {
    replaceItem({ ...item, leet: toLeet });
    renderList();
    return;
  }
  try {
    const updated = await invoke("rewrite_dictation", { id: item.id, toLeet });
    replaceItem(updated);
    renderList();
  } catch (error) {
    console.warn("réécriture impossible :", error);
  }
}

function closeMenu() {
  if (menuListener) {
    document.removeEventListener("pointerdown", menuListener, true);
    menuListener = null;
  }
  if (menuKeyListener) {
    document.removeEventListener("keydown", menuKeyListener);
    menuKeyListener = null;
  }
  menuEl?.remove();
  menuEl = null;
}

function openMenu(item, x, y) {
  closeMenu();
  const menu = el("div", "context-menu");
  menu.setAttribute("role", "menu");

  const addItem = (label, action, destructive = false) => {
    const button = el(
      "button",
      destructive ? "context-menu-item is-destructive" : "context-menu-item",
      label,
    );
    button.type = "button";
    button.setAttribute("role", "menuitem");
    button.addEventListener("click", async () => {
      closeMenu();
      await action();
    });
    menu.append(button);
  };
  const addSeparator = () => menu.append(el("div", "context-menu-separator"));

  if (!item.failed) {
    addItem(t("copy"), () => copyText(item.text));
    if ((item.polished || item.leet) && item.raw !== item.text) {
      addItem(t("copy_raw_text"), () => copyText(item.raw));
    }
    addSeparator();
    addItem(t("fix_a_word"), () => openFixSheet(item));
    addItem(
      item.leet ? t("back_to_normal_text") : t("rewrite_as_1337"),
      () => toggleLeet(item),
    );
    addSeparator();
  } else {
    addItem(t("retry"), () => retryItem(item));
    addSeparator();
  }
  addItem(t("delete"), () => deleteItem(item), true);

  document.body.append(menu);
  const rect = menu.getBoundingClientRect();
  menu.style.left = `${Math.max(8, Math.min(x, window.innerWidth - rect.width - 8))}px`;
  menu.style.top = `${Math.max(8, Math.min(y, window.innerHeight - rect.height - 8))}px`;
  menuEl = menu;

  const listener = (event) => {
    if (menuEl && !menuEl.contains(event.target)) {
      closeMenu();
    }
  };
  menuListener = listener;
  setTimeout(() => {
    if (menuEl === menu) {
      document.addEventListener("pointerdown", listener, true);
    }
  }, 0);

  const onKey = (event) => {
    if (event.key === "Escape") {
      closeMenu();
    }
  };
  menuKeyListener = onKey;
  document.addEventListener("keydown", onKey);
}

function showModal(overlay, onEscape) {
  document.body.append(overlay);
  const onKey = (event) => {
    if (event.key === "Escape") {
      overlay.remove();
      onEscape?.();
    }
  };
  document.addEventListener("keydown", onKey);
  const watcher = new MutationObserver(() => {
    if (!overlay.isConnected) {
      document.removeEventListener("keydown", onKey);
      watcher.disconnect();
    }
  });
  watcher.observe(document.body, { childList: true });
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      overlay.remove();
    }
  });
  return overlay;
}

function readPartner(entries, word) {
  return entries.find(
    (entry) => String(entry.word || "").toLowerCase() === word.toLowerCase(),
  );
}

async function learnCorrection(heard, wanted) {
  const entries = await invoke("get_vocabulary");
  const list = Array.isArray(entries) ? entries : [];
  const word = wanted.trim();
  const variant = heard.trim();
  let entry = readPartner(list, word);
  if (!entry) {
    entry = {
      id: globalThis.crypto?.randomUUID?.() || `vocab-${Date.now()}`,
      word,
      variants: [],
      addedAt: new Date().toISOString(),
    };
    list.push(entry);
  }
  if (!Array.isArray(entry.variants)) {
    entry.variants = [];
  }
  if (!entry.variants.some((value) => value.toLowerCase() === variant.toLowerCase())) {
    entry.variants.push(variant);
  }
  await invoke("set_vocabulary", { vocabulary: list });
}

function openFixSheet(item) {
  const overlay = el("div", "modal-overlay");
  const sheet = el("div", "fix-sheet");
  sheet.append(el("h2", "fix-title", t("fix_a_word_2")));
  sheet.append(el("p", "fix-quote", item.text));

  const form = el("div", "fix-form");
  const heardRow = el("div", "fix-row");
  const heardLabel = el("label", "fix-label", t("heard_2"));
  const heardInput = el("input", "fix-input");
  heardInput.type = "text";
  heardRow.append(heardLabel, heardInput);

  const wantedRow = el("div", "fix-row");
  const wantedLabel = el("label", "fix-label", t("correct"));
  const wantedInput = el("input", "fix-input");
  wantedInput.type = "text";
  wantedRow.append(wantedLabel, wantedInput);
  form.append(heardRow, wantedRow);
  sheet.append(form);
  sheet.append(el("p", "fix-note", t("fixed_here_and_added_to_vocabulary")));

  const actions = el("div", "fix-actions");
  const cancel = el("button", "sheet-button", t("cancel"));
  cancel.type = "button";
  const fix = el("button", "sheet-button is-default", t("fix"));
  fix.type = "button";
  fix.disabled = true;
  actions.append(cancel, fix);
  sheet.append(actions);
  overlay.append(sheet);

  const refresh = () => {
    fix.disabled = !heardInput.value.trim() || !wantedInput.value.trim();
  };
  heardInput.addEventListener("input", refresh);
  wantedInput.addEventListener("input", refresh);

  cancel.addEventListener("click", () => overlay.remove());
  fix.addEventListener("click", async () => {
    fix.disabled = true;
    if (!isDemo) {
      try {
        await learnCorrection(heardInput.value, wantedInput.value);
      } catch (error) {
        console.warn("correction non enregistrée :", error);
      }
    }
    overlay.remove();
  });

  showModal(overlay);
  requestAnimationFrame(() => heardInput.focus());
}

function openClearDialog() {
  const overlay = el("div", "modal-overlay is-centered");
  const dialog = el("div", "dialog");
  dialog.append(el("h2", "dialog-title", t("clear_all_history")));
  dialog.append(
    el(
      "p",
      "dialog-message",
      t("count_dictations_will_be_deleted", { count: items.length }),
    ),
  );

  const actions = el("div", "dialog-actions");
  const cancel = el("button", "sheet-button", t("cancel"));
  cancel.type = "button";
  const clear = el("button", "sheet-button is-destructive", t("clear_all"));
  clear.type = "button";
  actions.append(cancel, clear);
  dialog.append(actions);
  overlay.append(dialog);

  cancel.addEventListener("click", () => overlay.remove());
  clear.addEventListener("click", async () => {
    clear.disabled = true;
    if (!isDemo) {
      try {
        await invoke("clear_history");
      } catch (error) {
        console.warn("effacement impossible :", error);
        clear.disabled = false;
        return;
      }
    }
    items = [];
    overlay.remove();
    renderList();
  });

  showModal(overlay);
}

function historyRow(item) {
  const row = el("div", "history-row");
  row.dataset.id = item.id;
  const main = el("div", "history-main");

  if (item.failed) {
    main.append(
      el(
        "div",
        "history-text is-secondary",
        t("couldn_t_transcribe_seconds_s_of_audio_kept", {
          seconds: Math.round(Number(item.audioSeconds) || 0),
        }),
      ),
    );
  } else {
    const isLong = String(item.text).length > 420;
    const isExpanded = expanded.has(item.id);
    if (isLong && !isExpanded) {
      const text = el("div", "history-text is-clamped", item.text);
      text.addEventListener("click", () => {
        expanded.add(item.id);
        renderList();
      });
      const showAll = el("button", "history-link", t("show_all"));
      showAll.type = "button";
      showAll.addEventListener("click", () => {
        expanded.add(item.id);
        renderList();
      });
      main.append(text, showAll);
    } else {
      main.append(el("div", "history-text", item.text));
      if (isLong) {
        const showLess = el("button", "history-link", t("show_less"));
        showLess.type = "button";
        showLess.addEventListener("click", () => {
          expanded.delete(item.id);
          renderList();
        });
        main.append(showLess);
      }
    }
  }

  main.append(historyMeta(item));
  row.append(main);

  if (item.failed) {
    const retry = el("button", "row-retry", t("retry"));
    retry.type = "button";
    retry.addEventListener("click", () => retryItem(item, retry));
    row.append(retry);
  } else {
    row.append(copyButton(item));
  }

  row.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    openMenu(item, event.clientX, event.clientY);
  });
  return row;
}

function emptyState() {
  const box = el("div", "empty-state");
  box.append(
    icon("audio-waveform", "icon empty-icon"),
    el("div", "empty-title", t("no_dictations")),
    el(
      "div",
      "empty-detail",
      t("press_shortcut_to_start", { shortcut: shortcut || keyLabel("") }),
    ),
  );
  return box;
}

function noResults() {
  return el(
    "div",
    "no-results",
    t("no_results_for", { query: search }),
  );
}

function filteredItems() {
  const query = search.trim().toLowerCase();
  if (!query) {
    return items;
  }
  return items.filter(
    (item) =>
      String(item.text || "").toLowerCase().includes(query) ||
      String(item.appName || "").toLowerCase().includes(query),
  );
}

function renderList() {
  if (!listWrap) {
    return;
  }
  listWrap.textContent = "";
  if (clearButton) {
    clearButton.disabled = items.length === 0;
  }

  if (items.length === 0) {
    listWrap.append(emptyState());
    hydrateIcons(listWrap);
    return;
  }

  const filtered = filteredItems();
  if (filtered.length === 0) {
    listWrap.append(noResults());
    return;
  }

  const groups = new Map();
  for (const item of filtered) {
    const date = new Date(item.date);
    const key = `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
    if (!groups.has(key)) {
      groups.set(key, { date, items: [] });
    }
    groups.get(key).items.push(item);
  }

  const list = el("div", "history-list");
  for (const group of [...groups.values()].sort((a, b) => b.date - a.date)) {
    const section = el("section", "day-section");
    section.append(el("h2", "section-header", dayLabel(group.date)));
    const rows = [...group.items].sort(
      (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
    );
    for (const item of rows) {
      section.append(historyRow(item));
    }
    list.append(section);
  }
  listWrap.append(list);
  hydrateIcons(listWrap);
}

function renderPage(root) {
  root.textContent = "";
  search = "";
  expanded = new Set();

  const toolbar = el("div", "page-toolbar");
  toolbar.append(el("h1", "page-title", t("history")));
  toolbar.append(el("div", "toolbar-spacer"));

  const searchField = el("label", "search-field");
  searchField.append(searchGlyph());
  const input = el("input");
  input.type = "text";
  input.placeholder = t("search");
  input.setAttribute("aria-label", t("search"));
  input.addEventListener("input", () => {
    search = input.value;
    renderList();
  });
  searchField.append(input);
  toolbar.append(searchField);

  clearButton = el("button", "toolbar-action");
  clearButton.type = "button";
  clearButton.append(icon("trash-2"), el("span", "", t("clear_all")));
  clearButton.disabled = items.length === 0;
  clearButton.title = t("clear_all");
  clearButton.addEventListener("click", () => {
    if (items.length > 0) {
      openClearDialog();
    }
  });
  toolbar.append(clearButton);

  root.append(toolbar);
  listWrap = el("div", "history-body");
  root.append(listWrap);
  renderList();
}

function searchGlyph() {
  const namespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(namespace, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  const circle = document.createElementNS(namespace, "circle");
  circle.setAttribute("cx", "11");
  circle.setAttribute("cy", "11");
  circle.setAttribute("r", "8");
  const line = document.createElementNS(namespace, "path");
  line.setAttribute("d", "m21 21-4.3-4.3");
  svg.append(circle, line);
  const holder = el("span", "icon");
  holder.append(svg);
  return holder;
}

async function start(root, state) {
  isDemo = Boolean(state);
  const data = await loadData(state);
  items = data.history;
  shortcut = keyLabel(data.prefs?.dictationKey);
  renderPage(root);
  await hydrateIcons(root);

  if (state === "history-menu" && items.length > 0) {
    requestAnimationFrame(() => {
      const row = document.querySelector(".history-row");
      if (!row) {
        return;
      }
      const rect = row.getBoundingClientRect();
      openMenu(items[0], rect.left + 150, rect.top + 24);
    });
  }
}

function mount(demoState) {
  const root = document.querySelector(`[data-page-root="${ROOT}"]`);
  if (!root || root.dataset.mounted === "true") {
    return;
  }
  root.dataset.mounted = "true";
  const state = demoState && demoState.page === ROOT ? demoState.state : null;
  start(root, state).catch((error) => {
    console.error("historique :", error);
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
