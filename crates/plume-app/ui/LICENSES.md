# Licences des ressources embarquées

Plume embarque deux ressources tierces, avec leurs licences complètes à côté
d'elles.

## Inter (police)

- Fichier : `ui/fonts/InterVariable.woff2` (police variable, graisses 100–900).
- Version : Inter 4.1 (release `v4.1`, <https://github.com/rsms/inter/releases/tag/v4.1>).
- Licence : SIL Open Font License 1.1 — texte complet dans
  `ui/fonts/LICENSE.txt`.
- Usage : `font-family: "Inter", system-ui, sans-serif` (voir
  `ui/css/tokens.css`), `font-feature-settings` neutre.

## Lucide (icônes)

- Dossier : `ui/icons/*.svg`, icônes Lucide (`lucide-static` 1.73.0,
  <https://registry.npmjs.org/lucide-static/-/lucide-static-1.73.0.tgz>).
- Licence : ISC — texte complet dans `ui/icons/LICENSE`.
- Usage : SVG inline via `ui/js/icons.js` ; le trait prend `currentColor` et la
  taille vient du CSS.
- Icônes embarquées : uniquement celles nommées dans la table §1.9 de
  `docs/design-spec.md` et présentes dans `lucide-static` 1.73.0. Deux entrées
  de la table n'existent plus sous ce nom dans Lucide (`rectangle-x`,
  `circle-half`) : elles ne sont ni dessinées ni embarquées.
