// La pastille de dictée (spec §3, `PillPanel.swift`) : les 13 barres dessinées
// sur un <canvas> à la résolution de l'écran, les états ✓ / Copié / message, le
// pop-up « rien à coller », les teintes des réglages et le mouvement.
//
// Ce que le Rust envoie (`docs/ipc.md`) :
//   plume://phase  { phase, text }   idle · recording · processing · done ·
//                                    cancelled · rejected · error
//   plume://level  nombre 0…1        ~20 fois par seconde pendant l'écoute
//
// `PLUME_DEMO` (rendu par `ui_env`) joue un état de démonstration avec des
// données factices, pour les captures : "listening", "processing", "pasted",
// "popup", "copied", "message", "error".

import { applyI18n, initI18n, t } from "./i18n.js";
import { hydrateIcons } from "./icons.js";
import { initTheme } from "./theme.js";
import { hasTauri, invoke, listen } from "./tauri.js";

// Les constantes de §3.6 : 13 barres de 3 pt, espacées de 2,5 pt, dans un
// cadre de 18 pt (une barre peut déborder jusqu'à 22 pt).
const BAR_COUNT = 13;
const BAR_WIDTH = 3;
const BAR_GAP = 2.5;
const BARS_WIDTH = BAR_COUNT * BAR_WIDTH + (BAR_COUNT - 1) * BAR_GAP; // 69
const BARS_HEIGHT = 22;

// Les deux emplacements de la fenêtre de la pastille (§3.1) : la pastille
// ordinaire (320 × 64 + 26 d'ombre) et le pop-up (430 × 96 + 26).
const WINDOW_PILL = { width: 320, height: 90 };
const WINDOW_POPUP = { width: 482, height: 122 };

// Le mouvement de §3.10.
const MOTION_IN = { duration: 340, easing: "cubic-bezier(0.33, 1, 0.68, 1)" };
const MOTION_OUT = { duration: 220, easing: "cubic-bezier(0.32, 0, 0.67, 0)" };
const MOTION_SPRING = { duration: 320, easing: "cubic-bezier(0.34, 1.2, 0.64, 1)" };

// L'aperçu du pop-up : les cinq premiers mots + « … » (§3.8).
const PREVIEW_WORDS = 5;

const stage = document.querySelector(".pill-stage");
const card = document.querySelector(".pill-card");
const canvas = document.querySelector(".pill-bars");
const cancelButton = document.querySelector(".pill-cancel");
const closeButton = document.querySelector(".pill-close");
const label = document.querySelector(".pill-label");
const labelText = document.querySelector(".pill-label-text");
const labelIcon = document.querySelector(".pill-label-icon");
const popupBody = document.querySelector(".pill-popup-body");

const state = {
  /** État de démonstration (`PLUME_DEMO`), ou null. */
  demo: null,
  /** Phase affichée : hidden · recording · processing · pasted · copied ·
   *  message · error · popup. */
  phase: "hidden",
  /** `top` ou `bottom` (`Prefs.pillStyle`). */
  style: "top",
  /** Le texte complet de la dictée gardée (le clic le copie). */
  popupText: "",
  /** Les 13 niveaux du micro, le plus récent à droite (`LevelMeter`). */
  levels: new Array(BAR_COUNT).fill(0),
  /** Le niveau de démonstration, une voix calme et continue. */
  fakeLevel: 0,
  /** `requestAnimationFrame` en cours, ou 0. */
  frame: 0,
  /** L'instant de départ des animations, en secondes. */
  startedAt: 0,
  /** Le temps écoulé, en secondes, depuis le début de la phase. */
  clock: 0,
  lastTick: 0,
};

// --- Les barres -------------------------------------------------------------

const ctx = canvas.getContext("2d");

/** La couleur des barres, lue sur le canvas (`color` du CSS, qui suit la phase). */
function barColor() {
  const color = getComputedStyle(canvas).color;
  return color && color !== "transparent" ? color : "#33d6e0";
}

/** `rgba(r, g, b, a)` depuis une couleur CSS déjà calculée. */
function withAlpha(color, alpha) {
  const match = color.match(/rgba?\(([^)]+)\)/);
  if (!match) {
    return color;
  }
  const [r, g, b] = match[1].split(",").map(Number);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * La hauteur d'une barre, formule exacte de `PillPanel.swift:591-604`.
 *
 * - traitement : `4 + 10 * (sin(6t − 0,55·i) + 1) / 2` → 4…14 pt ;
 * - écoute : `max(3, 3 + 15·niveau^0,8 + 4·frémissement)` → 3…22 pt.
 */
function barHeight(index, t, thinking) {
  if (thinking) {
    const wave = Math.sin(t * 6 - index * 0.55);
    return 4 + 10 * ((wave + 1) / 2);
  }
  const level = state.levels[index] ?? 0;
  const shimmer = 0.5 + 0.5 * Math.sin(t * 7 + index * 1.3);
  return Math.max(3, 3 + 15 * Math.pow(level, 0.8) + 4 * shimmer);
}

/** Dessine les 13 barres, capsules arrondies, avec leur halo (`radius: 4`). */
function drawBars() {
  const ratio = window.devicePixelRatio || 1;
  const width = Math.round(BARS_WIDTH * ratio);
  const height = Math.round(BARS_HEIGHT * ratio);
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }

  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.clearRect(0, 0, BARS_WIDTH, BARS_HEIGHT);

  const color = barColor();
  const thinking = state.phase === "processing";
  ctx.fillStyle = color;
  // `.shadow(color.opacity(0.5), radius: 4)` : le halo doux des barres.
  ctx.shadowColor = withAlpha(color, 0.5);
  ctx.shadowBlur = 4 * ratio;

  for (let index = 0; index < BAR_COUNT; index += 1) {
    const heightPx = barHeight(index, state.clock, thinking);
    const x = index * (BAR_WIDTH + BAR_GAP);
    const y = (BARS_HEIGHT - heightPx) / 2;
    ctx.beginPath();
    ctx.roundRect(x, y, BAR_WIDTH, heightPx, BAR_WIDTH / 2);
    ctx.fill();
  }
  ctx.shadowBlur = 0;
}

/** Une image de l'animation : avance l'horloge, met à jour le niveau, dessine. */
function tick(now) {
  const seconds = now / 1000;
  state.clock = seconds - state.startedAt;
  state.lastTick = seconds;

  if (state.phase === "recording" && state.demo) {
    // Une voix calme : deux sinusoïdes qui ne tombent jamais à zéro ensemble.
    state.fakeLevel = Math.max(
      0.05,
      0.45 +
        0.3 * Math.sin(seconds * 2.1) * Math.sin(seconds * 0.7 + 1.2) +
        0.12 * Math.sin(seconds * 7.3),
    );
    state.levels.shift();
    state.levels.push(state.fakeLevel);
  }

  drawBars();
  state.frame = requestAnimationFrame(tick);
}

/** L'animation ne tourne que quand les barres sont visibles (30 fps minimum). */
function syncAnimation() {
  const wanted = state.phase === "recording" || state.phase === "processing";
  if (wanted && !state.frame) {
    state.startedAt = performance.now() / 1000;
    state.lastTick = 0;
    state.clock = 0;
    state.frame = requestAnimationFrame(tick);
  } else if (!wanted && state.frame) {
    cancelAnimationFrame(state.frame);
    state.frame = 0;
  }
}

/** Un niveau du micro (`plume://level`) : le plus récent pousse à droite. */
function pushLevel(level) {
  state.levels.shift();
  state.levels.push(Math.min(1, Math.max(0, Number(level) || 0)));
}

// --- Les états --------------------------------------------------------------

/** L'aperçu du pop-up : les cinq premiers mots + « … » (`PillPanel.swift:453`). */
export function previewOf(text) {
  const words = String(text ?? "").trim().split(/\s+/).filter(Boolean);
  if (words.length <= PREVIEW_WORDS) {
    return words.join(" ");
  }
  return `${words.slice(0, PREVIEW_WORDS).join(" ")}…`;
}

/** Ajuste la fenêtre à la carte : pastille ou pop-up (`panelSize`, §3.1). */
async function fitWindow(popup) {
  if (!hasTauri()) {
    return;
  }
  const size = popup ? WINDOW_POPUP : WINDOW_PILL;
  try {
    const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;
    await getCurrentWindow().setSize(new LogicalSize(size.width, size.height));
  } catch (error) {
    // Permission absente : la carte se resserre d'elle-même (CSS `min()`).
    console.debug("taille de la pastille :", error);
  }
}

/** Coupe les animations en cours, pour ne pas les empiler. */
function stopMotion() {
  for (const animation of card.getAnimations()) {
    animation.cancel();
  }
}

/** Applique l'état demandé, avec le mouvement de §3.10. */
function setPhase(phase, options = {}) {
  const previous = state.phase;
  const wasHidden = previous === "hidden";
  state.phase = phase;

  if (phase === "popup") {
    state.popupText = options.text ?? state.popupText;
    popupBody.textContent = previewOf(state.popupText);
  }
  if (phase === "copied") {
    labelText.textContent = t("copied");
  }
  if (phase === "message" || phase === "error") {
    labelText.textContent = options.text || t("microphone_unavailable");
    const icon = phase === "error" ? "triangle-alert" : "audio-waveform";
    if (labelIcon.dataset.icon !== icon) {
      // L'icône change de nom : il faut recharger le SVG.
      labelIcon.dataset.icon = icon;
      delete labelIcon.dataset.iconReady;
      labelIcon.innerHTML = "";
      hydrateIcons(label.parentElement);
    }
  }

  stage.dataset.phase = phase;
  stage.dataset.shown = phase === "hidden" ? "false" : "true";
  stage.dataset.popup = phase === "popup" ? "true" : "false";
  fitWindow(phase === "popup");

  stopMotion();

  if (phase === "hidden") {
    if (state.style === "top") {
      // Le style du haut remonte avant de disparaître (`slideOut`, 0.22 s).
      card
        .animate(
          [{ transform: "translateY(0)" }, { transform: "translateY(-100%)" }],
          { ...MOTION_OUT, fill: "both" },
        )
        .finished.catch(() => {});
    }
    syncAnimation();
    return;
  }

  if (state.style === "top" && wasHidden) {
    // Le haut descend du bord de l'écran, sans fondu (§3.10).
    card.animate(
      [{ transform: "translateY(-100%)" }, { transform: "translateY(0)" }],
      { ...MOTION_IN, fill: "both" },
    );
  } else if (state.style === "bottom" && wasHidden) {
    // En bas : `.scale(0.6, anchor: .bottom)` + opacité, avec le ressort.
    card.animate(
      [
        { transform: "scale(0.6)", opacity: 0, transformOrigin: "50% 100%" },
        { transform: "scale(1)", opacity: 1, transformOrigin: "50% 100%" },
      ],
      { ...MOTION_SPRING, fill: "both" },
    );
  } else if (previous !== phase) {
    // Changement de phase : le ressort intérieur (response 0.32 / damping 0.78).
    card.animate(
      [
        { transform: "scale(0.96)", opacity: 0.5 },
        { transform: "scale(1)", opacity: 1 },
      ],
      { ...MOTION_SPRING, fill: "both" },
    );
  }

  // Le canvas est prêt : on dessine tout de suite la première image.
  state.startedAt = performance.now() / 1000;
  state.lastTick = 0;
  state.clock = 0;
  drawBars();
  syncAnimation();
}

// --- Le pont avec le Rust ---------------------------------------------------

/** La phase du Rust (`plume://phase`) vers l'état de la pastille. */
function applyBackendPhase(payload) {
  const phase = payload?.phase ?? "idle";
  const text = payload?.text ?? null;
  switch (phase) {
    case "recording":
      state.levels = new Array(BAR_COUNT).fill(0);
      setPhase("recording");
      break;
    case "processing":
      setPhase("processing");
      break;
    case "done":
      // Le Rust ne dit pas encore si le texte a été collé : la pastille montre
      // le ✓ « collé » de §3.7, et le pop-up reste atteignable par la démo.
      setPhase("pasted", { text });
      break;
    case "rejected":
      setPhase("message", { text });
      break;
    case "error":
      setPhase("error", { text });
      break;
    case "cancelled":
    case "idle":
    default:
      setPhase("hidden");
      break;
  }
}

/** Ce que fait un clic sur la carte (`pillClicked`, §3.2). */
async function pillClicked() {
  if (state.phase === "recording" || state.phase === "processing") {
    try {
      await invoke("open_window", { name: "main" });
    } catch (error) {
      console.warn("ouvrir Plume :", error);
    }
    return;
  }
  if (state.phase === "popup") {
    try {
      await invoke("copy_text", { text: state.popupText });
      setPhase("copied");
    } catch (error) {
      console.warn("copier le texte :", error);
    }
  }
}

function dismiss() {
  setPhase("hidden");
}

// --- Le démarrage -----------------------------------------------------------

/** `PLUME_DEMO` : "listening", "processing", "pasted", "popup"… */
function demoPhase(demo) {
  switch ((demo ?? "").trim().toLowerCase()) {
    case "listening":
    case "recording":
      return "recording";
    case "processing":
      return "processing";
    case "pasted":
    case "done":
      return "pasted";
    case "copied":
      return "copied";
    case "message":
    case "rejected":
      return "message";
    case "error":
      return "error";
    case "popup":
      return "popup";
    default:
      return null;
  }
}

// Le texte factice de la démo : la capture du Mac montre « Je voulais te dire
// que… », donc le texte complet commence par ces cinq mots.
const DEMO_TEXT =
  "Je voulais te dire que le rendu de la pastille est enfin fidèle à la maquette.";

async function boot() {
  let prefs = null;
  let env = null;
  if (hasTauri()) {
    try {
      prefs = await invoke("get_prefs");
    } catch (error) {
      console.warn("réglages indisponibles :", error);
    }
    try {
      env = await invoke("ui_env");
    } catch (error) {
      console.warn("environnement de test indisponible :", error);
    }
  }

  // TEMP PROBE
  let probeText = "no-tauri";
  try {
    const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;
    const w = getCurrentWindow();
    await w.setSize(new LogicalSize(482, 122));
    const inner = await w.innerSize();
    probeText = "OK " + inner.width + "x" + inner.height;
  } catch (e) {
    probeText = "before " + window.innerWidth + "x" + window.innerHeight;
    window.resizeTo(482, 122);
    await new Promise((r) => setTimeout(r, 400));
    probeText += " after " + window.innerWidth + "x" + window.innerHeight;
  }
  state.demo = env?.demo ?? null;
  await initI18n(prefs);
  await initTheme(prefs);

  // Le look (haut ou bas) et les deux teintes viennent des réglages.
  state.style = prefs?.pillStyle === "bottom" ? "bottom" : "top";
  stage.dataset.style = state.style;
  if (prefs?.listenTint) {
    stage.dataset.listen = prefs.listenTint;
  }
  if (prefs?.thinkTint) {
    stage.dataset.think = prefs.thinkTint;
  }

  // Le canvas est à la résolution de l'écran (net sur un écran 2×).
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(BARS_WIDTH * ratio);
  canvas.height = Math.round(BARS_HEIGHT * ratio);
  canvas.style.width = `${BARS_WIDTH}px`;
  canvas.style.height = `${BARS_HEIGHT}px`;

  await hydrateIcons(document);
  applyI18n(stage);

  cancelButton.addEventListener("click", (event) => {
    event.stopPropagation();
    dismiss();
  });
  closeButton.addEventListener("click", (event) => {
    event.stopPropagation();
    dismiss();
  });
  card.addEventListener("click", pillClicked);

  if (hasTauri()) {
    await listen("plume://phase", (event) => {
      if (state.demo) {
        return;
      }
      applyBackendPhase(event.payload ?? {});
    });
    await listen("plume://level", (event) => {
      if (state.demo) {
        return;
      }
      pushLevel(event.payload);
    });
  }

  const demo = demoPhase(state.demo);
  if (demo) {
    // Le pop-up est plus large : la fenêtre prend sa taille avant l'affichage.
    if (demo === "popup") {
      await fitWindow(true);
    }
    setPhase(demo, { text: demo === "popup" ? DEMO_TEXT : undefined });
    if (demo === "popup") { popupBody.textContent = probeText; }
  } else {
    stage.dataset.phase = "hidden";
    stage.dataset.shown = "false";
  }
  stage.dataset.ready = "true";
}

boot().catch((error) => console.error("pastille :", error));
