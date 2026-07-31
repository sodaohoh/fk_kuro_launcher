param(
    [string]$ExePath,
    [switch]$Uninstall
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
if ($Uninstall) {
    $uninstallScript = $null
    if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
        $candidate = Join-Path $PSScriptRoot "uninstall.ps1"
        if (Test-Path $candidate) { $uninstallScript = $candidate }
    }
    if (-not $uninstallScript) {
        $candidate = Join-Path (Get-Location).Path "uninstall.ps1"
        if (Test-Path $candidate) { $uninstallScript = $candidate }
    }
    if ($uninstallScript) {
        & $uninstallScript
    } else {
        $webUrl = "https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/uninstall.ps1"
        Invoke-Expression (Invoke-RestMethod -Uri $webUrl -UseBasicParsing)
    }
    exit 0
}

Write-Host "=====================================================" -ForegroundColor Cyan
Write-Host "   Wuthering Waves 1-Click Steam Auto-Installer      " -ForegroundColor Cyan
Write-Host "=====================================================" -ForegroundColor Cyan

# 1. Target Installation Directory
$InstallDir = Join-Path $env:LOCALAPPDATA "fk_kuro_launcher"
$InstalledExe = Join-Path $InstallDir "fk_kuro_launcher.exe"

# 2. Binary Acquisition Logic
if (-not [string]::IsNullOrWhiteSpace($ExePath)) {
    if (Test-Path $ExePath) {
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        }
        Copy-Item -Path (Get-Item $ExePath).FullName -Destination $InstalledExe -Force
        Write-Host "[SUCCESS] Installed fk_kuro_launcher.exe to $InstalledExe" -ForegroundColor Green
    } else {
        Write-Host "[WARNING] Specified ExePath does not exist: $ExePath" -ForegroundColor Yellow
        if (-not (Test-Path $InstalledExe)) {
            Write-Host "[ERROR] Installation aborted. Executable not found." -ForegroundColor Red
            exit 1
        }
    }
} else {
    Write-Host "[INFO] Downloading latest release from GitHub..." -ForegroundColor Yellow
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    }
    $downloadUrl = "https://github.com/sodaohoh/fk_kuro_launcher/releases/latest/download/fk_kuro_launcher.exe"
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $InstalledExe -UseBasicParsing -ErrorAction Stop
        Write-Host "[SUCCESS] Installed fk_kuro_launcher.exe to $InstalledExe" -ForegroundColor Green
    } catch {
        if (Test-Path $InstalledExe) {
            Write-Host "[WARNING] Failed to download latest release from GitHub: $_" -ForegroundColor Yellow
            Write-Host "[INFO] Keeping existing executable at $InstalledExe" -ForegroundColor Green
        } else {
            Write-Host "[ERROR] Failed to download fk_kuro_launcher.exe from GitHub: $_" -ForegroundColor Red
            Write-Host "[ERROR] Installation aborted. Steam configuration was not modified." -ForegroundColor Red
            exit 1
        }
    }
}
# 3. Steam Path Registry Auto-Detection
$SteamDir = $null

try {
    $regHKCU = Get-ItemProperty -Path "HKCU:\Software\Valve\Steam" -Name "SteamPath" -ErrorAction SilentlyContinue
    if ($regHKCU -and $regHKCU.SteamPath) {
        $SteamDir = $regHKCU.SteamPath
    }
} catch {}

if (-not $SteamDir) {
    try {
        $regHKLM = Get-ItemProperty -Path "HKLM:\SOFTWARE\WOW6432Node\Valve\Steam" -Name "InstallPath" -ErrorAction SilentlyContinue
        if ($regHKLM -and $regHKLM.InstallPath) {
            $SteamDir = $regHKLM.InstallPath
        }
    } catch {}
}
if (-not $SteamDir -or -not (Test-Path $SteamDir)) {
    $SteamDir = "C:\Program Files (x86)\Steam"
}

if (Test-Path $SteamDir) {
    $SteamDir = (Get-Item $SteamDir).FullName
}
Write-Host "[INFO] Detected Steam directory: $SteamDir" -ForegroundColor Green

$SteamExe = Join-Path $SteamDir "steam.exe"
$UserDataDir = Join-Path $SteamDir "userdata"

# 4. Close Steam if running and wait for termination
$steamProc = Get-Process -Name "steam" -ErrorAction SilentlyContinue
if ($steamProc) {
    Write-Host "[INFO] Closing Steam to apply launch options safely..." -ForegroundColor Yellow
    Stop-Process -Name "steam" -Force
    [void]$steamProc.WaitForExit(10000)
    Start-Sleep -Seconds 2
}

# 5. Update localconfig.vdf with safety backups for all Steam profiles
if (Test-Path $UserDataDir) {
    $userDirs = Get-ChildItem -Path $UserDataDir -Directory
    $updatedCount = 0

    foreach ($userDir in $userDirs) {
        $vdfPath = Join-Path $userDir.FullName "config\localconfig.vdf"
        if (Test-Path $vdfPath) {
            # Create safety backup (.bak)
            $bakPath = "$vdfPath.bak"
            Copy-Item -Path $vdfPath -Destination $bakPath -Force
            Write-Host "[INFO] Safety backup created: $bakPath" -ForegroundColor Gray

            try {
                $content = Get-Content -Path $vdfPath -Raw -Encoding UTF8

                $vdfExePath = $InstalledExe.Replace('\', '\\')
                $vdfLaunchOptions = "\`"$vdfExePath\`" %command%"

                # Scoped regex matching specifically for 2775500 block (Wuthering Waves)
                if ($content -match '"2775500"') {
                    if ($content -match '("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*') {
                        $content = $content -replace '("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*', ('$1' + "`n`t`t`t`t`"LaunchOptions`"`t`t`"$vdfLaunchOptions`"")
                    } else {
                        $content = $content -replace '("2775500"\s*\{)', ('$1' + "`n`t`t`t`t`"LaunchOptions`"`t`t`"$vdfLaunchOptions`"")
                    }
                } else {
                    # Inject 2775500 block under "apps"
                    $newBlock = "`"2775500`"`n`t`t`t`t{`n`t`t`t`t`t`"LaunchOptions`"`t`t`"$vdfLaunchOptions`"`n`t`t`t`t}"
                    $content = $content -replace '("apps"\s*\{)', ('$1' + "`n`t`t`t`t$newBlock")
                }

                $utf8NoBom = New-Object System.Text.UTF8Encoding $false
                [System.IO.File]::WriteAllText($vdfPath, $content, $utf8NoBom)
                Write-Host "[SUCCESS] Configured Steam profile: $($userDir.Name)" -ForegroundColor Green
                $updatedCount++
            } catch {
                Write-Host "[ERROR] Failed to update VDF, restoring from backup..." -ForegroundColor Red
                Copy-Item -Path $bakPath -Destination $vdfPath -Force
            }
        }
    }

    if ($updatedCount -gt 0) {
        Write-Host "[SUCCESS] Successfully configured $updatedCount Steam user profile(s)!" -ForegroundColor Green
    } else {
        Write-Host "[WARNING] No Steam user config (localconfig.vdf) files found under $UserDataDir." -ForegroundColor Yellow
    }
} else {
    Write-Host "[ERROR] Steam userdata directory not found at $UserDataDir." -ForegroundColor Red
}

# 6. Restart Steam
if (Test-Path $SteamExe) {
    Write-Host "[INFO] Restarting Steam..." -ForegroundColor Green
    Start-Process -FilePath $SteamExe
}

Write-Host ""
Write-Host "[COMPLETE] Steam launch options set automatically! Launch Wuthering Waves via Steam." -ForegroundColor Yellow
