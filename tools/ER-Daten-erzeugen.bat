@echo off
rem Put this file in the ELDEN-RING-Combat-Rewrite folder (next to the "tools"
rem folder) and double-click it. It generates the data from your own Elden Ring
rem and copies the two files Claude needs into E:\ER-Export.
setlocal
cd /d "%~dp0"

set "ER_FILES=E:\ELDEN RING\Game"
set "ER_GAME_DIR=E:\ELDEN RING\Game"

if not exist "tools\setup.py" (
    echo Diese Datei muss im Ordner ELDEN-RING-Combat-Rewrite liegen, neben dem Ordner "tools".
    pause
    exit /b 1
)
if not exist "%ER_FILES%\chr\c0000.anibnd.dcx" (
    echo UXM-Dateien fehlen: %ER_FILES%\chr\c0000.anibnd.dcx nicht gefunden.
    pause
    exit /b 1
)
if not exist "%ER_FILES%\regulation-bin" (
    echo WitchyBND-Ordner fehlt: %ER_FILES%\regulation-bin nicht gefunden.
    pause
    exit /b 1
)
where python >nul 2>nul || (
    echo Python ist nicht installiert. Bitte Python 3.10 oder neuer von python.org installieren
    echo und beim Installieren "Add python.exe to PATH" anhaken.
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

if not exist "E:\ER-Export" mkdir "E:\ER-Export"
copy /y "src\sim\extracted.rs" "E:\ER-Export\" >nul
copy /y "assets\player_anims.bin" "E:\ER-Export\" >nul
echo.
echo Fertig. Die Dateien liegen in E:\ER-Export. Sag Claude Bescheid.
pause
