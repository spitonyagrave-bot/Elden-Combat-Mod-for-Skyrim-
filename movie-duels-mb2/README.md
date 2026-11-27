# MB2-Kampf für Movie Duels SP

Patch für den Movie-Duels-Code (github.com/MovieDuels/MovieDuels-SJE-SP-Rend2,
Commit 17d1c65 = Update 7). Fügt eine MB2-artige Blocklogik hinzu
(`code/game/wp_saber_mb2.cpp`). GPLv2 wie Movie Duels.

Der Workflow baut daraus `MovieDuels-gamex86_64.dll` mit MSVC und legt sie in `movie-duels-mb2/build/`.

## Update 8

`mb2-u8.patch` ist derselbe MB2-Kampf (Stand v16), umgestellt auf Movie Duels Update 8
(Commit 2ee6492, Build "Day-11.Month-11.Year-26.BuildNum-09"). Update 8 hat Funktionen und Felder
umbenannt (z. B. saberMove, MBF_HOLDINGBLOCK, Jedi_CombatDistance, PM_ReturnforQuad). Die DLL landet in `build-u8/`.
