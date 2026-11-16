// Vocabulaire (§2.4) : la liste des mots, la fiche 440 × 440 (mot + formes
// entendues) et « Nouveau mot ». Enregistrement par `set_vocabulary` ; en
// démonstration (`PLUME_DEMO`) les données sont simulées.

import { t, currentLanguage } from "../js/i18n.js";
import { invoke } from "../js/tauri.js";
import { hydrateIcons } from "../js/icons.js";

const ROOT = "vocabulary";
const content = document.getElementById("content");

let entries = [];
let search = "";
let listWrap = null;
let isDemo = false;
let menuEl = null;
let menuListener = null;
let menuKeyListener = null;

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

function newId() {
  return globalThis.crypto?.randomUUID?.() || `vocab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function demoEntries() {
  const addedAt = new Date().toISOString();
  return [
    { id: "vocab-1", word: "Kubernetes", variants: ["coub ernetes", "kubernetes"], addedAt },
    { id: "vocab-2", word: "Parakeet", variants: ["parakit", "parakite"], addedAt },
    { id: "vocab-3", word: "Jexyllax", variants: ["jexilax", "jexilaxe"], addedAt },
    { id: "vocab-4", word: "Élodie", variants: ["élody", "hélo die"], addedAt },
    { id: "vocab-5", word: "Plume", variants: [], addedAt },
  ];
}

async function loadData(state) {
  if (state) {
    return { entries: demoEntries() };
  }
  try {
    const list = await invoke("get_vocabulary");
    return { entries: Array.isArray(list) ? list : [] };
  } catch {
    return { entries: [] };
  }
}

async function persist() {
  if (isDemo) {
    return;
  }
  await invoke("set_vocabulary", { vocabulary: entries });
}

function filteredEntries() {
  const query = search.trim().toLowerCase();
  if (!query) {
    return entries;
  }
  return entries.filter(
    (entry) =>
      String(entry.word || "").toLowerCase().includes(query) ||
      (Array.isArray(entry.variants)
        ? entry.variants.some((variant) => String(variant).toLowerCase().includes(query))
        : false),
  );
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

function openRowMenu(entry, x, y) {
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
  addItem(t("edit_2"), () => openEditor(entry));
  addItem(t("delete"), () => deleteEntry(entry), true);

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

async function deleteEntry(entry) {
  entries = entries.filter((item) => item.id !== entry.id);
  try {
    await persist();
  } catch (error) {
    console.warn("vocabulaire non enregistré :", error);
  }
  renderList();
}

function normalizeEntry(source) {
  const word = String(source.word || "").trim();
  const seen = new Set();
  const variants = [];
  for (const raw of Array.isArray(source.variants) ? source.variants : []) {
    const variant = String(raw || "").trim();
    const key = variant.toLowerCase();
    if (!variant || key === word.toLowerCase() || seen.has(key)) {
      continue;
    }
    seen.add(key);
    variants.push(variant);
  }
  return { ...source, word, variants };
}

function renderForms(card, forms, focusTarget) {
  card.textContent = "";

  forms.forEach((value, index) => {
    const row = el("div", "form-row");
    const input = el("input");
    input.type = "text";
    input.value = value;
    input.placeholder = t("e_g_dipsik");
    input.setAttribute("aria-label", t("form"));
    input.addEventListener("input", () => {
      forms[index] = input.value;
      remove.hidden = forms.length <= 1 && !input.value.trim();
    });

    const remove = el("button", "icon-button");
    remove.type = "button";
    remove.title = t("remove");
    remove.append(icon("circle-minus"));
    remove.hidden = forms.length <= 1 && !String(value).trim();
    remove.addEventListener("click", () => {
      forms.splice(index, 1);
      if (forms.length === 0) {
        forms.push("");
      }
      renderForms(card, forms, Math.max(0, Math.min(index, forms.length - 1)));
    });

    row.append(input, remove);
    card.append(row);
    if (focusTarget === index) {
      requestAnimationFrame(() => input.focus());
    }
  });

  const add = el("button", "form-add");
  add.type = "button";
  add.append(icon("plus"), el("span", "", t("add")));
  add.addEventListener("click", () => {
    const last = forms[forms.length - 1];
    if (forms.length > 0 && !String(last).trim()) {
      renderForms(card, forms, forms.length - 1);
      return;
    }
    forms.push("");
    renderForms(card, forms, forms.length - 1);
  });
  card.append(add);
  hydrateIcons(card);
}

function openEditor(entry = null) {
  const isNew = !entry;
  let word = entry?.word ?? "";
  const forms = Array.isArray(entry?.variants) && entry.variants.length > 0
    ? [...entry.variants]
    : [""];

  const overlay = el("div", "modal-overlay");
  const sheet = el("div", "sheet vocab-sheet");

  const body = el("div", "sheet-body");

  const wordSection = el("div", "form-section");
  wordSection.append(el("div", "form-section-header", t("word")));
  const wordCard = el("div", "form-card");
  const wordRow = el("div", "form-row is-word");
  const wordInput = el("input");
  wordInput.type = "text";
  wordInput.value = word;
  wordInput.placeholder = t("correct_spelling");
  wordInput.setAttribute("aria-label", t("word"));
  wordRow.append(wordInput);
  wordCard.append(wordRow);
  wordSection.append(wordCard);
  body.append(wordSection);

  const soundsSection = el("div", "form-section");
  soundsSection.append(el("div", "form-section-header", t("how_it_sounds")));
  const soundsCard = el("div", "form-card");
  soundsSection.append(soundsCard);
  body.append(soundsSection);

  const footnote = el(
    "p",
    "form-footnote",
    t("one_per_line_using_the_letters_you_actually_hear_plume_replaces_every_one_of_them_with_the_word_above"),
  );
  soundsSection.append(footnote);

  const footer = el("div", "sheet-footer");
  if (!isNew) {
    const removeWord = el("button", "sheet-button is-destructive", t("delete_word"));
    removeWord.type = "button";
    removeWord.addEventListener("click", async () => {
      await deleteEntry(entry);
      closeSheet();
    });
    footer.append(removeWord);
  }
  footer.append(el("div", "spacer"));

  const cancel = el("button", "sheet-button", t("cancel"));
  cancel.type = "button";
  const save = el("button", "sheet-button is-default", t("save"));
  save.type = "button";
  save.disabled = !word.trim();

  cancel.addEventListener("click", closeSheet);
  save.addEventListener("click", async () => {
    const candidate = normalizeEntry({
      id: entry?.id ?? newId(),
      word,
      variants: forms,
      addedAt: entry?.addedAt ?? new Date().toISOString(),
    });
    if (!candidate.word) {
      return;
    }

    const byId = entries.findIndex((item) => item.id === candidate.id);
    if (byId >= 0) {
      entries[byId] = candidate;
    } else {
      const byWord = entries.findIndex(
        (item) => String(item.word).toLowerCase() === candidate.word.toLowerCase(),
      );
      if (byWord >= 0) {
        const merged = entries[byWord];
        const variants = Array.isArray(merged.variants) ? [...merged.variants] : [];
        for (const variant of candidate.variants) {
          if (!variants.some((value) => String(value).toLowerCase() === variant.toLowerCase())) {
            variants.push(variant);
          }
        }
        entries[byWord] = normalizeEntry({ ...merged, variants });
      } else {
        entries = [candidate, ...entries];
      }
    }

    try {
      await persist();
    } catch (error) {
      console.warn("vocabulaire non enregistré :", error);
    }
    closeSheet();
    renderList();
  });

  footer.append(cancel, save);
  sheet.append(body, footer);
  overlay.append(sheet);

  function closeSheet() {
    overlay.remove();
    document.removeEventListener("keydown", onKey);
  }

  wordInput.addEventListener("input", () => {
    word = wordInput.value;
    save.disabled = !word.trim();
  });

  renderForms(soundsCard, forms, null);

  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      closeSheet();
    }
  });
  const onKey = (event) => {
    if (event.key === "Escape") {
      closeSheet();
    }
  };
  document.addEventListener("keydown", onKey);

  document.body.append(overlay);
  requestAnimationFrame(() => {
    if (isNew) {
      wordInput.focus();
    } else {
      const empty = [...soundsCard.querySelectorAll("input")].find(
        (field) => !field.value.trim(),
      );
      empty?.focus();
    }
  });
}

function vocabRow(entry) {
  const row = el("button", "vocab-row");
  row.type = "button";
  row.dataset.id = entry.id;

  const main = el("span", "vocab-main");
  main.append(el("span", "vocab-word", entry.word));
  if (Array.isArray(entry.variants) && entry.variants.length > 0) {
    main.append(
      el("span", "vocab-variants", `${t("heard")}${entry.variants.join(", ")}`),
    );
  }
  row.append(main, icon("chevron-right", "icon vocab-chevron"));

  row.addEventListener("click", () => openEditor(entry));
  row.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    openRowMenu(entry, event.clientX, event.clientY);
  });
  return row;
}

function noResults() {
  return el("div", "no-results", t("no_results_for", { query: search }));
}

function renderList() {
  if (!listWrap) {
    return;
  }
  listWrap.textContent = "";

  const filtered = filteredEntries();
  const list = el("div", "vocab-list");

  const intro = el("div", "vocab-intro");
  const newWord = el("button", "new-word-row");
  newWord.type = "button";
  newWord.append(icon("circle-plus"), el("span", "", t("new_word")));
  newWord.addEventListener("click", () => openEditor(null));
  intro.append(
    newWord,
    el(
      "p",
      "vocab-hint",
      t("write_how_it_sounds_the_way_you_hear_it_plume_picks_up_every_variant"),
    ),
  );
  list.append(intro);

  if (filtered.length === 0 && search.trim()) {
    list.append(noResults());
  } else {
    const section = el("section", "vocab-section");
    section.append(
      el("h2", "section-header", t("count_words", { count: filtered.length })),
    );
    const rows = el("div", "vocab-rows");
    for (const entry of filtered) {
      rows.append(vocabRow(entry));
    }
    section.append(rows);
    list.append(section);
  }

  listWrap.append(list);
  hydrateIcons(listWrap);
}

function renderPage(root) {
  root.textContent = "";
  search = "";

  const toolbar = el("div", "page-toolbar");
  toolbar.append(el("h1", "page-title", t("vocabulary")));
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

  const newWord = el("button", "toolbar-action");
  newWord.type = "button";
  newWord.title = t("new_word");
  newWord.append(icon("plus"), el("span", "", t("new_word")));
  newWord.addEventListener("click", () => openEditor(null));
  toolbar.append(newWord);

  root.append(toolbar);
  listWrap = el("div", "vocab-body");
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
  entries = data.entries;
  renderPage(root);
  await hydrateIcons(root);

  if (state === "vocabulary-sheet") {
    const entry = entries[2] || entries[0] || null;
    if (entry) {
      requestAnimationFrame(() => openEditor(entry));
    }
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
    console.error("vocabulaire :", error);
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
