@echo off
:: Tsukuyomi OS Nuke / Reinstall script
:: Run as Administrator for best results

cd /d "%~dp0"
set INSTALL_DIR=%LOCALAPPDATA%\TsukuyomiOS

powershell -ExecutionPolicy Bypass -Command "
$dirs = @(
    '$env:LOCALAPPDATA\TsukuyomiOS',
    '$env:APPDATA\TsukuyomiOS'
)
foreach ($d in $dirs) {
    if (Test-Path $d) {
        Remove-Item -Recurse -Force $d
        Write-Host 'Removed: ' $d
    }
}
Get-ChildItem -Path $env:USERPROFILE\Desktop -Filter 'Tsukuyomi OS.lnk' | Remove-Item -Force
"

python -m pip uninstall -y tsukuyomi-os 2>nul

echo Tsukuyomi OS data cleaned.
pause
