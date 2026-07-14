@echo off
:: Tsukuyomi OS Windows installer
cd /d "%~dp0"
powershell -ExecutionPolicy Bypass -File "%~dp0Install-Tsukuyomi.ps1"
pause
