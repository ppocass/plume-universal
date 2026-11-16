// Page « Modèles » (§2.6) : la machine, les modèles locaux avec téléchargement,
// progression, suppression et choix, puis l'état du moteur.
// La liste vient de `list_models` ; la recommandation de `recommend_model`.

import { t } from "../js/i18n.js";
import { hydrateIcons } from "../js/icons.js";
import { hasTauri, invoke, listen } from "../js/tauri.js";

const CONTENT = document.getElementById("content");
const SPEEDS = ["slow", "fast", "very_fast"];
const ACCURACIES = ["good", "very_good", "excellent"];

let demo = null;
let root = null;
let models = [];
let engine = null;
let downloadingDemo = false;

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

function formatSize(sizeMb) {
  if (sizeMb >= 1000) {
    const value = (sizeMb / 1000).toFixed(1).replace(".0", "");
    return `${value} ${t("gb")}`;
  }
  return `${sizeMb} ${t("mb")}`;
}

function speedLabel(speed) {
  const index = Math.max(0, Math.min(2, speed <= 2 ? 0 : speed === 3 ? 1 : 2));
  return t(SPEEDS[index]);
}

function accuracyLabel(accuracy) {
  const index = Math.max(0, Math.min(2, accuracy <= 2 ? 0 : accuracy <= 4 ? 1 : 2));
  return t(ACCURACIES[index]);
}

function facts(view) {
  return [
    formatSize(view.sizeMb),
    t(view.languages || ""),
    t("speed_speed", { speed: speedLabel(view.speed) }),
    t("accuracy_accuracy", { accuracy: accuracyLabel(view.accuracy) }),
  ].join(" · ");
}

/** L'état du moteur, en clair (§2.7, ligne « État »). */
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

function progressNode(view, progress = view.progress || 0) {
  const wrap = el("div", "model-progress");
  const bar = el("div", "progress");
  const fill = el("i");
  fill.style.width = `${Math.round(progress * 100)}%`;
  bar.append(fill);
  const percent = el("span", "progress-value", `${Math.round(progress * 100)} %`);
  wrap.append(bar, percent);
  return wrap;
}

function actionFor(view) {
  const action = el("div", "model-action");
  const downloading = view.downloading || (downloadingDemo && view.id === "parakeet-v3");

  if (downloading) {
    // État de démonstration : une progression crédible, sans réseau.
    const progress = downloadingDemo && !view.downloading ? 0.62 : view.progress || 0;
    action.append(progressNode(view, progress));
    const cancel = el("button", "button-small", t("cancel"));
    cancel.type = "button";
    cancel.addEventListener("click", async () => {
      cancel.disabled = true;
      try {
        await invoke("cancel_download", { id: view.id });
      } catch (error) {
        console.warn("téléchargement non annulé :", error);
      }
      await refresh();
    });
    action.append(cancel);
    return action;
  }

  if (view.installed && view.active) {
    const inUse = el("span", "in-use");
    const icon = el("span");
    icon.dataset.icon = "circle-check";
    inUse.append(icon, document.createTextNode(t("in_use")));
    action.append(inUse);
    return action;
  }

  if (view.installed) {
    const use = el("button", "button-small", t("use"));
    use.type = "button";
    use.addEventListener("click", async () => {
      use.disabled = true;
      try {
        await invoke("use_model", { id: view.id });
      } catch (error) {
        console.warn("modèle non sélectionné :", error);
      }
      await refresh();
    });
    action.append(use);

    const remove = el("button", "button-icon");
    remove.type = "button";
    remove.title = t("delete");
    remove.setAttribute("aria-label", t("delete"));
    const trash = el("span");
    trash.dataset.icon = "trash-2";
    remove.append(trash);
    remove.addEventListener("click", () => confirmDelete(view));
    action.append(remove);
    return action;
  }

  const download = el("button", "button-small", t("download"));
  download.type = "button";
  download.addEventListener("click", async () => {
    download.disabled = true;
    try {
      await invoke("download_model", { id: view.id });
    } catch (error) {
      showError(view.id, String(error));
      download.disabled = false;
      return;
    }
    await refresh();
  });
  action.append(download);
  return action;
}

function showError(id, message) {
  const row = root?.querySelector(`[data-model-id="${id}"]`);
  if (!row) {
    return;
  }
  let error = row.querySelector(".model-error");
  if (!error) {
    error = el("p", "model-error");
    row.querySelector(".model-text")?.append(error);
  }
  error.textContent = message;
}

function modelRow(view) {
  const row = el("div", "form-row model-row");
  row.dataset.modelId = view.id;

  const text = el("div", "model-text");
  const title = el("div", "model-title");
  title.append(el("span", "model-name", view.name));
  if (view.recommended) {
    title.append(el("span", "tag", t("recommended_for_this_mac")));
  }
  text.append(title);
  if (view.descriptionKey) {
    text.append(el("p", "model-summary", t(view.descriptionKey)));
  }
  text.append(el("p", "model-facts", facts(view)));

  row.append(text, actionFor(view));
  return row;
}

function renderModels() {
  for (const family of ["parakeet", "whisper"]) {
    const card = root?.querySelector(`[data-family="${family}"]`);
    if (!card) {
      continue;
    }
    card.textContent = "";
    const familyModels = models.filter((view) => view.family === family);
    if (!familyModels.length) {
      card.append(el("div", "form-row", t("unavailable")));
      continue;
    }
    for (const view of familyModels) {
      card.append(modelRow(view));
    }
  }
  hydrateIcons(root);
}

function renderEngine() {
  const status = root?.querySelector("[data-engine-status]");
  const name = root?.querySelector("[data-engine-model]");
  if (status) {
    status.textContent = engineLabel(engine);
  }
  if (name) {
    const active = models.find((view) => view.active);
    name.textContent = active ? active.name : engine?.modelId || "—";
  }
}

function confirmDelete(view) {
  const layer = root?.querySelector("[data-alert]");
  if (!layer) {
    return;
  }
  layer.textContent = "";
  layer.hidden = false;

  const alert = el("div", "alert");
  alert.setAttribute("role", "alertdialog");
  const title = el("h2", "alert-title", t("delete_model", { model: view.name }));
  const message = el(
    "p",
    "alert-message",
    t("size_will_be_freed_you_can_download_it_again", { size: formatSize(view.sizeMb) }),
  );
  const buttons = el("div", "alert-buttons");
  const cancel = el("button", "", t("cancel"));
  cancel.type = "button";
  const remove = el("button", "is-destructive", t("delete"));
  remove.type = "button";

  const close = () => {
    layer.hidden = true;
    layer.textContent = "";
  };

  cancel.addEventListener("click", close);
  remove.addEventListener("click", async () => {
    close();
    try {
      await invoke("delete_model", { id: view.id });
    } catch (error) {
      console.warn("modèle non supprimé :", error);
    }
    await refresh();
  });

  buttons.append(cancel, remove);
  alert.append(title, message, buttons);
  layer.append(alert);
  cancel.focus();
}

async function loadModels() {
  try {
    models = await invoke("list_models");
  } catch (error) {
    console.warn("liste des modèles indisponible :", error);
    models = [];
  }
}

async function loadEngine() {
  try {
    engine = await invoke("engine_status");
  } catch {
    engine = null;
  }
}

async function refresh() {
  if (!root) {
    return;
  }
  await loadModels();
  await loadEngine();
  renderModels();
  renderEngine();
}

async function mount(node) {
  root = node;
  downloadingDemo = demo === "page=models-downloading";

  let prefs = null;
  try {
    prefs = hasTauri() ? await invoke("get_prefs") : null;
  } catch {
    prefs = null;
  }

  let machine = null;
  try {
    const recommendation = await invoke("recommend_model", {
      spokenLanguage: prefs?.language || "auto",
    });
    machine = recommendation?.machine ?? null;
  } catch (error) {
    console.warn("recommandation indisponible :", error);
  }

  const machineNode = root.querySelector("[data-machine]");
  if (machineNode) {
    machineNode.textContent = machine
      ? `${machine.cpu} · ${Math.round(machine.ramTotalGb)} ${t("gb")}`
      : "—";
  }

  await refresh();
}

// La progression et la fin d'un téléchargement, où que soit l'écran.
listen("plume://model-progress", (event) => {
  const payload = event.payload ?? {};
  const row = root?.querySelector(`[data-model-id="${payload.id}"]`);
  if (!row) {
    return;
  }
  const total = Number(payload.totalBytes) || 0;
  const done = Number(payload.doneBytes) || 0;
  const progress = total > 0 ? Math.min(1, done / total) : 0;
  const fill = row.querySelector(".progress > i");
  const percent = row.querySelector(".progress-value");
  if (fill) {
    fill.style.width = `${Math.round(progress * 100)}%`;
  }
  if (percent) {
    percent.textContent = `${Math.round(progress * 100)} %`;
  }
});

listen("plume://model-done", () => {
  refresh();
});

listen("plume://engine-status", (event) => {
  engine = event.payload ?? null;
  renderEngine();
});

async function ensureMounted() {
  const node = CONTENT?.querySelector('[data-page="models"]');
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
