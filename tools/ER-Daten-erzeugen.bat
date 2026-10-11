@echo off
rem Doppelklicken. Erzeugt aus deinem eigenen Elden Ring die Daten fuer die
rem Skyrim-Mod. Die Werkzeuge im Ordner "tools" stammen aus
rem ELDEN-RING-Combat-Rewrite (MIT, siehe LICENSE.md).
setlocal
cd /d "%~dp0"

rem Find the game's Oodle DLL and the unpacked chr folder anywhere under the
rem Elden Ring folder (UXM keeps a copy of the original files in _backup).
set "ROOT="
for %%R in ("C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING" "C:\Program Files\Steam\steamapps\common\ELDEN RING" "C:\SteamLibrary\steamapps\common\ELDEN RING" "C:\Steam\steamapps\common\ELDEN RING" "C:\ELDEN RING") do if not defined ROOT if exist "%%~R" set "ROOT=%%~R"
if not defined ROOT (
    echo Ich finde Elden Ring auf C nicht.
    echo Bitte zieh den Ordner ELDEN RING von C mit der Maus in dieses Fenster
    echo und druecke dann Enter.
    set /p "ROOT="
)
set "ROOT=%ROOT:"=%"
set "GAME="
set "CHR="
echo Suche in %ROOT% ...
rem Any Oodle 2 core will do; the tools expect it under the name oo2core_6.
set "OODLE="
for /f "delims=" %%D in ('dir /s /b "%ROOT%\oo2core_*_win64.dll" 2^>nul') do if not defined OODLE set "OODLE=%%D"
if defined OODLE (
    if not exist "oodle" mkdir "oodle"
    copy /y "%OODLE%" "oodle\oo2core_6_win64.dll" >nul
    set "GAME=%~dp0oodle\"
)
for /f "delims=" %%C in ('dir /s /b "%ROOT%\c0000.anibnd.dcx" 2^>nul ^| findstr /v /i "\\_backup\\"') do if not defined CHR set "CHR=%%~dpC"
rem The params WitchyBND unpacked from this install's own regulation.bin.
set "PARAMS="
for /f "delims=" %%P in ('dir /s /b "%ROOT%\EquipParamWeapon.param" 2^>nul ^| findstr /v /i "\\_backup\\"') do if not defined PARAMS set "PARAMS=%%~dpP"
if not defined GAME (
    echo Keine oo2core_..._win64.dll unter %ROOT% gefunden.
    pause
    exit /b 1
)
if not defined CHR (
    echo c0000.anibnd.dcx nicht gefunden unter %ROOT%. Bitte in UXM entpacken.
    pause
    exit /b 1
)
if not defined PARAMS (
    echo Die regulation.bin von %ROOT% ist noch nicht entpackt.
    echo Bitte die regulation.bin aus dem Game Ordner auf die WitchyBND.exe ziehen,
    echo danach diese Datei nochmal starten.
    pause
    exit /b 1
)
set "GAME=%GAME:~0,-1%"
set "CHR=%CHR:~0,-1%"
echo Oodle:   %OODLE%
echo Dateien: %CHR%
echo Params:  %PARAMS%
set "ER_FILES=%~dp0er-files"
set "ER_GAME_DIR=%GAME%"
for %%F in (c0000.anibnd.dcx c0000_a00_hi.anibnd.dcx c0000_a00_lo.anibnd.dcx c0000_a00_md.anibnd.dcx c0000_a0x.anibnd.dcx c0000_a1x.anibnd.dcx c0000_a2x.anibnd.dcx c0000_a3x.anibnd.dcx c0000_a4x.anibnd.dcx c0000_a5x.anibnd.dcx) do (
    if not exist "%CHR%\%%F" (
        echo Fehlt: %CHR%\%%F
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
del /q "er-files\chr\*" "er-files\regulation-bin\*" >nul 2>nul
copy /y "%CHR%\c0000*.anibnd.dcx" "er-files\chr\" >nul
copy /y "%PARAMS%*.param" "er-files\regulation-bin\" >nul

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
