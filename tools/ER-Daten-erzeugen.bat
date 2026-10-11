@echo off
rem Doppelklicken. Erzeugt aus deinem eigenen Elden Ring die Daten fuer die
rem Skyrim-Mod. Die Werkzeuge im Ordner "tools" stammen aus
rem ELDEN-RING-Combat-Rewrite (MIT, siehe LICENSE.md).
setlocal
cd /d "%~dp0"

rem Find the game's Oodle DLL and the unpacked chr folder anywhere under the
rem Elden Ring folder (UXM keeps a copy of the original files in _backup).
set "ROOT=E:\ELDEN RING"
if not exist "%ROOT%" (
    echo Ich finde den Ordner %ROOT% nicht.
    echo Bitte zieh den Ordner ELDEN RING mit der Maus in dieses Fenster
    echo und druecke dann Enter.
    set /p "ROOT="
)
set "ROOT=%ROOT:"=%"
set "GAME="
set "CHR="
echo Suche in %ROOT% ...
for /f "delims=" %%D in ('dir /s /b "%ROOT%\oo2core_6_win64.dll" 2^>nul') do if not defined GAME set "GAME=%%~dpD"
for /f "delims=" %%C in ('dir /s /b "%ROOT%\c0000.anibnd.dcx" 2^>nul ^| findstr /v /i "\\_backup\\"') do if not defined CHR set "CHR=%%~dpC"
if not defined GAME (
    echo oo2core_6_win64.dll nicht gefunden unter %ROOT%.
    pause
    exit /b 1
)
if not defined CHR (
    echo c0000.anibnd.dcx nicht gefunden unter %ROOT%. Bitte in UXM entpacken.
    pause
    exit /b 1
)
set "GAME=%GAME:~0,-1%"
set "CHR=%CHR:~0,-1%"
echo Oodle:  %GAME%
echo Dateien: %CHR%
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
copy /y "%CHR%\c0000*.anibnd.dcx" "er-files\chr\" >nul
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
