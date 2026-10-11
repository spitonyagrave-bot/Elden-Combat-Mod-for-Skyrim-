@echo off
rem Doppelklicken. Erzeugt aus deinem eigenen Elden Ring die Daten fuer die
rem Skyrim-Mod. Die Werkzeuge im Ordner "tools" stammen aus
rem ELDEN-RING-Combat-Rewrite (MIT, siehe LICENSE.md).
setlocal
cd /d "%~dp0"

set "ER_FILES=E:\ELDEN RING\Game"
set "ER_GAME_DIR=E:\ELDEN RING\Game"

for %%F in (c0000.anibnd.dcx c0000_a00_hi.anibnd.dcx c0000_a00_lo.anibnd.dcx c0000_a00_md.anibnd.dcx c0000_a0x.anibnd.dcx c0000_a1x.anibnd.dcx c0000_a2x.anibnd.dcx c0000_a3x.anibnd.dcx c0000_a4x.anibnd.dcx c0000_a5x.anibnd.dcx) do (
    if not exist "%ER_FILES%\chr\%%F" (
        echo Fehlt: %ER_FILES%\chr\%%F
        echo Bitte diese Datei in UXM noch entpacken.
        pause
        exit /b 1
    )
)
if not exist "%ER_FILES%\regulation-bin" (
    echo Fehlt: der Ordner %ER_FILES%\regulation-bin
    echo Bitte regulation.bin mit der Maus auf WitchyBND.exe ziehen.
    pause
    exit /b 1
)
where python >nul 2>nul || (
    echo Python fehlt. Bitte Python von python.org installieren und dabei
    echo "Add python.exe to PATH" anhaken. Danach diese Datei nochmal starten.
    pause
    exit /b 1
)

echo Erzeuge die Daten, das dauert etwa eine Minute ...
python tools\setup.py
if errorlevel 1 (
    echo.
    echo Es gab einen Fehler. Bitte einen Screenshot von diesem Fenster an Claude schicken.
    pause
    exit /b 1
)
echo.
echo Fertig. Sag Claude Bescheid.
pause
