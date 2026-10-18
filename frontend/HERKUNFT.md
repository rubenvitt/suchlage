# Herkunft übernommener Dateien
Stand aus Lifeline Hub, Commit 3eb9115f2, kopiert am 16.10.2026. Fachpaletten (Sichtung, ETB,
Warnstufe, Fachebene) entfernt; Speicherschlüssel `lifeline-hub.*` → `suchlage.*`.
Betroffen: src/theme/{tokens.ts,rollen.css,sprache.css,rollen.guard.test.ts,ThemeModeProvider.tsx,dichte.ts,helligkeit.ts,schriften.ts},
src/lib/sichererSpeicher.ts, src/components/useViewport.ts, src/components/instrument/*, src/assets/fonts/*,
src/test/farbmass.ts, src/map/maplibreWorker.ts (Task F3), src/map/basemapStyle.ts (Task F3, aus basemapStil.ts).
Eigener Ersatz statt Kopie: src/test/utils.tsx (nur `renderMitProviders`, ohne Lifelines Auth).
Schriften: SIL OFL 1.1 (assets/fonts/lizenzen).

## Icons (SUL-50)
Stand aus Lifeline Hub, Commit 5f07a20de, kopiert am 18.10.2026. Icons8 „iOS 27 Outlined“/„Filled“,
Verwendung in Suchlage freigegeben am 17.10.2026 (Plan M1, Offene Punkte Nr. 9).
Betroffen: src/icons/{IconRahmen.tsx,IconRahmen.test.tsx,icons.css,index.ts},
src/icons/erzeugt.generated.ts (gekürzt auf die Rail-Icons; der Generator `scripts/icons/` bleibt in Lifeline).
