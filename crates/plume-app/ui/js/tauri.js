// Accès minimal à Tauri : la coquille marche aussi ouverte dans un navigateur
// (les appels échouent proprement au lieu de casser la page).

export const hasTauri = () => Boolean(window.__TAURI__?.core);

export async function invoke(command, args = {}) {
  if (!hasTauri()) {
    throw new Error("Tauri indisponible");
  }
  return window.__TAURI__.core.invoke(command, args);
}

export async function listen(event, handler) {
  if (!hasTauri()) {
    return () => {};
  }
  return window.__TAURI__.event.listen(event, handler);
}

export async function closeCurrentWindow() {
  if (!hasTauri()) {
    return;
  }
  const current = window.__TAURI__.window.getCurrentWindow();
  await current.close();
}
