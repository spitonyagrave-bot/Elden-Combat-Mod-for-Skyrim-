@echo off
rem Doppelklicken. Erzeugt aus deinem eigenen Elden Ring die Daten fuer die
rem Skyrim-Mod. Die Werkzeuge im Ordner "tools" stammen aus
rem ELDEN-RING-Combat-Rewrite (MIT, siehe LICENSE.md).
setlocal
cd /d "%~dp0"

rem Find the game: a few usual places, otherwise ask for eldenring.exe.
set "GAME="
for %%G in ("E:\ELDEN RING\Game" "E:\SteamLibrary\steamapps\common\ELDEN RING\Game" "E:\Steam\steamapps\common\ELDEN RING\Game" "C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game" "D:\SteamLibrary\steamapps\common\ELDEN RING\Game") do (
    if not defined GAME if exist "%%~G\oo2core_6_win64.dll" set "GAME=%%~G"
)
if not defined GAME (
    echo Ich finde Elden Ring nicht automatisch.
    echo Bitte zieh die Datei eldenring.exe mit der Maus in dieses Fenster
    echo und druecke dann Enter.
    set /p "EXE="
)
rem Dragging a file into the window pastes its path, in quotes when it has spaces.
if not defined GAME (
    for %%E in (%EXE%) do set "GAME=%%~dpE"
)
if "%GAME:~-1%"=="\" set "GAME=%GAME:~0,-1%"
if not exist "%GAME%\oo2core_6_win64.dll" (
    echo Im Ordner %GAME% fehlt oo2core_6_win64.dll. Ist das der Ordner mit eldenring.exe?
    pause
    exit /b 1
)
echo Elden Ring gefunden: %GAME%
set "ER_FILES=%~dp0er-files"
set "ER_GAME_DIR=%GAME%"
for %%F in (c0000.anibnd.dcx c0000_a00_hi.anibnd.dcx c0000_a00_lo.anibnd.dcx c0000_a00_md.anibnd.dcx c0000_a0x.anibnd.dcx c0000_a1x.anibnd.dcx c0000_a2x.anibnd.dcx c0000_a3x.anibnd.dcx c0000_a4x.anibnd.dcx c0000_a5x.anibnd.dcx) do (
    if not exist "%GAME%\chr\%%F" (
        echo Fehlt: %GAME%\chr\%%F
        echo Bitte diese Datei in UXM noch entpacken.
        pause
        exit /b 1
    )
)
where python >nul 2>nul || (
    echo Python fehlt. Bitte Python von python.org installieren und dabei
    echo "Add python.exe to PATH" anhaken. Danach diese Datei nochmal starten.
    pause
    exit /b 1
)

echo Kopiere die entpackten Dateien ...
if not exist "er-files\chr" mkdir "er-files\chr"
if not exist "er-files\regulation-bin" mkdir "er-files\regulation-bin"
copy /y "%GAME%\chr\c0000*.anibnd.dcx" "er-files\chr\" >nul
copy /y "..\*.param" "er-files\regulation-bin\" >nul

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
