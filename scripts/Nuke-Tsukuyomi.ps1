# Tsukuyomi OS Nuke / Reinstall script
# Run as Administrator for best results

$dirs = @(
    "$env:LOCALAPPDATA\TsukuyomiOS",
    "$env:APPDATA\TsukuyomiOS"
)
foreach ($d in $dirs) {
    if (Test-Path $d) {
        Remove-Item -Recurse -Force $d
        Write-Host "Removed: $d"
    }
}

$shortcut = "$env:USERPROFILE\Desktop\Tsukuyomi OS.lnk"
if (Test-Path $shortcut) {
    Remove-Item -Force $shortcut
    Write-Host "Removed desktop shortcut"
}

& python -m pip uninstall -y tsukuyomi-os

Write-Host "Tsukuyomi OS data cleaned."
