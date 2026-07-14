# Tsukuyomi OS Nuke / Uninstall script
#
# Delegates data cleanup to `tsukuyomi.exe uninstall`, the single source of
# truth for which local data directories exist, then removes the install
# directory (and the exe within it) and the desktop shortcut. Running the
# exe's own uninstall first and letting it fully exit before deleting its
# containing directory avoids trying to delete a running executable's file.

$installDir = "$env:LOCALAPPDATA\TsukuyomiOS"
$exe = "$installDir\tsukuyomi.exe"

if (Test-Path $exe) {
    & $exe uninstall --yes
} else {
    Write-Host "tsukuyomi.exe not found at $exe; skipping its self-uninstall step."
}

if (Test-Path $installDir) {
    Remove-Item -Recurse -Force $installDir
    Write-Host "Removed: $installDir"
}

$shortcut = "$env:USERPROFILE\Desktop\Tsukuyomi OS.lnk"
if (Test-Path $shortcut) {
    Remove-Item -Force $shortcut
    Write-Host "Removed desktop shortcut"
}

Write-Host "Tsukuyomi OS has been removed from this machine."
