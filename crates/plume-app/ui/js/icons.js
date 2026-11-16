// Les icônes Lucide (ISC, ui/icons/) en SVG inline : le trait prend la couleur
// du texte (`currentColor`) et la taille vient du CSS.

const DIR = new URL("../icons/", import.meta.url);
const cache = new Map();

async function loadIcon(name) {
  if (!cache.has(name)) {
    cache.set(
      name,
      fetch(new URL(`${name}.svg`, DIR))
        .then((response) => (response.ok ? response.text() : ""))
        .catch(() => ""),
    );
  }
  return cache.get(name);
}

/**
 * Remplace chaque `[data-icon="nom"]` par le SVG Lucide correspondant.
 * Les SVG manquants laissent l'élément vide, sans casser la page.
 */
export async function hydrateIcons(root = document) {
  const nodes = [...root.querySelectorAll("[data-icon]")];
  await Promise.all(
    nodes.map(async (node) => {
      if (node.dataset.iconReady === "true") {
        return;
      }
      const svg = await loadIcon(node.dataset.icon);
      if (!svg) {
        console.warn(`icône absente : ${node.dataset.icon}`);
        return;
      }
      node.innerHTML = svg;
      const element = node.firstElementChild;
      if (element) {
        element.setAttribute("aria-hidden", "true");
        element.setAttribute("focusable", "false");
      }
      node.dataset.iconReady = "true";
    }),
  );
}
