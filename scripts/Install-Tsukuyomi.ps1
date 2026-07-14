# Tsukuyomi OS Windows installer
# Run: powershell -ExecutionPolicy Bypass -File Install-Tsukuyomi.ps1
#
# Places the self-contained tsukuyomi.exe (no Python/runtime required) into
# %LOCALAPPDATA%\TsukuyomiOS and creates a desktop shortcut.

$installDir = "$env:LOCALAPPDATA\TsukuyomiOS"
$targetExe = "$installDir\tsukuyomi.exe"

function Find-SourceExe {
    # Prefer an exe shipped alongside this script (a release bundle); fall back
    # to a locally built one for developers running straight from the repo.
    $sideBySide = Join-Path $PSScriptRoot "tsukuyomi.exe"
    if (Test-Path $sideBySide) { return $sideBySide }

    $builtRelease = Join-Path $PSScriptRoot "..\target\release\tsukuyomi.exe"
    if (Test-Path $builtRelease) { return $builtRelease }

    throw "Could not find tsukuyomi.exe next to this script or at ..\target\release\tsukuyomi.exe. Build it first with 'cargo build --release', or place a prebuilt tsukuyomi.exe alongside this script."
}

function Install-Tsukuyomi {
    $sourceExe = Find-SourceExe
    Write-Host "Installing Tsukuyomi OS..."
    if (!(Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }
    Copy-Item -Path $sourceExe -Destination $targetExe -Force

    $desktop = [Environment]::GetFolderPath("Desktop")
    $shortcut = "$desktop\Tsukuyomi OS.lnk"
    $WshShell = New-Object -comObject WScript.Shell
    $SC = $WshShell.CreateShortcut($shortcut)
    $SC.TargetPath = $targetExe
    $SC.WorkingDirectory = $installDir
    $SC.IconLocation = "$targetExe,0"
    $SC.Save()
    Write-Host "Shortcut created on desktop."
}

Install-Tsukuyomi
Write-Host "Starting Tsukuyomi OS..."
& $targetExe
