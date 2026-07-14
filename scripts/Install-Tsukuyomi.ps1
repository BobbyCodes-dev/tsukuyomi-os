# Tsukuyomi OS Windows installer
# Run: powershell -ExecutionPolicy Bypass -File Install-Tsukuyomi.ps1

$installDir = "$env:LOCALAPPDATA\TsukuyomiOS"
$pythonExe = "$env:LOCALAPPDATA\Programs\Python\Python311\python.exe"

function Ensure-Python {
    if (!(Test-Path $pythonExe)) {
        Write-Host "Python 3.11 not found. Downloading..."
        $url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe"
        $installer = "$env:TEMP\python-3.11.9-amd64.exe"
        Invoke-WebRequest -Uri $url -OutFile $installer
        Start-Process -FilePath $installer -ArgumentList "/quiet", "InstallAllUsers=0", "PrependPath=1", "Include_launcher=1" -Wait
        $pythonExe = "$env:LOCALAPPDATA\Programs\Python\Python311\python.exe"
    }
}

function Install-Tsukuyomi {
    Ensure-Python
    Write-Host "Installing Tsukuyomi OS..."
    & $pythonExe -m pip install --user --upgrade tsukuyomi-os
    if (!(Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }
    $desktop = [Environment]::GetFolderPath("Desktop")
    $shortcut = "$desktop\Tsukuyomi OS.lnk"
    $WshShell = New-Object -comObject WScript.Shell
    $SC = $WshShell.CreateShortcut($shortcut)
    $SC.TargetPath = $pythonExe
    $SC.Arguments = "-m ironos.tui"
    $SC.WorkingDirectory = $installDir
    $SC.IconLocation = "$pythonExe,0"
    $SC.Save()
    Write-Host "Shortcut created on desktop."
}

Install-Tsukuyomi
Write-Host "Starting Tsukuyomi OS..."
& $pythonExe -m ironos.tui
