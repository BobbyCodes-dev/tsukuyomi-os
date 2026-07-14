@echo off
:: Tsukuyomi OS Windows launcher
:: Downloads Python if missing, installs Tsukuyomi OS, and runs it in a terminal

cd /d "%~dp0"
set INSTALL_DIR=%LOCALAPPDATA%\TsukuyomiOS
set PYTHON_DIR=%LOCALAPPDATA%\Programs\Python\Python311
set PYTHON=%PYTHON_DIR%\python.exe

echo Checking for Python...
if not exist "%PYTHON%" (
    echo Python not found. Downloading Python 3.11...
    curl -L -o "%TEMP%\python-installer.exe" https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe
    "%TEMP%\python-installer.exe" /quiet InstallAllUsers=0 PrependPath=1 Include_launcher=1
    set PYTHON=%LOCALAPPDATA%\Programs\Python\Python311\python.exe
)

if not exist "%INSTALL_DIR%" (
    echo Installing Tsukuyomi OS...
    "%PYTHON%" -m pip install --user tsukuyomi-os
    md "%INSTALL_DIR%"
)

echo Starting Tsukuyomi OS...
"%PYTHON%" -m ironos.tui

pause
