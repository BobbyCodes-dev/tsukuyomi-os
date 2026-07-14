@echo off
:: Tsukuyomi OS Nuke / Uninstall script
cd /d "%~dp0"
powershell -ExecutionPolicy Bypass -File "%~dp0Nuke-Tsukuyomi.ps1"
pause
