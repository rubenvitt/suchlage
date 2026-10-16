# Herkunft übernommener Dateien
Stand aus Lifeline Hub, Commit 3eb9115f2, kopiert am 16.10.2026. Fachpaletten (Sichtung, ETB,
Warnstufe, Fachebene) entfernt; Speicherschlüssel `lifeline-hub.*` → `suchlage.*`.
Betroffen: src/theme/{tokens.ts,rollen.css,sprache.css,rollen.guard.test.ts,ThemeModeProvider.tsx,dichte.ts,helligkeit.ts,schriften.ts},
src/lib/sichererSpeicher.ts, src/components/useViewport.ts, src/components/instrument/*, src/assets/fonts/*,
src/test/farbmass.ts, src/map/maplibreWorker.ts (Task F3), src/map/basemapStyle.ts (Task F3, aus basemapStil.ts).
Eigener Ersatz statt Kopie: src/test/utils.tsx (nur `renderMitProviders`, ohne Lifelines Auth).
Schriften: SIL OFL 1.1 (assets/fonts/lizenzen). Icons8-SVGs sind NICHT übernommen (Lizenz für ein zweites Projekt ungeklärt).
