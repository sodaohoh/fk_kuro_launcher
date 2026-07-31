param(
    [string]$ExePath,
    [switch]$Uninstall
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
if ($Uninstall) {
    Write-Host "=====================================================" -ForegroundColor Cyan
    Write-Host "   Wuthering Waves Steam Wrapper Uninstaller         " -ForegroundColor Cyan
    Write-Host "=====================================================" -ForegroundColor Cyan

    # 1. Detect Steam path from Registry
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

    # 2. Close Steam gracefully if running
    $steamProcs = Get-Process -Name "steam", "steamwebhelper" -ErrorAction SilentlyContinue
    if ($steamProcs) {
        Write-Host "[INFO] Closing Steam to clear launch options safely..." -ForegroundColor Yellow
        Stop-Process -Name "steam", "steamwebhelper" -Force -ErrorAction SilentlyContinue
        $steamProcs | ForEach-Object { try { [void]$_.WaitForExit(5000) } catch {} }
        Start-Sleep -Seconds 2
    }

    # 3. Clean localconfig.vdf files in all Steam userdata profiles
    if (Test-Path $UserDataDir) {
        $userDirs = Get-ChildItem -Path $UserDataDir -Directory
        $cleanedCount = 0

        foreach ($userDir in $userDirs) {
            $vdfPath = Join-Path $userDir.FullName "config\localconfig.vdf"
            if (Test-Path $vdfPath) {
                try {
                    $content = Get-Content -Path $vdfPath -Raw -Encoding UTF8
                    if ($content -match '"2775500"') {
                        # Create safety backup (.bak) before modifying
                        $bakPath = "$vdfPath.bak"
                        Copy-Item -Path $vdfPath -Destination $bakPath -Force

                        $content = $content -replace '(?i)("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*', '$1'
                        $content = $content -replace '(?m)^\s*"[^"]*fk_kuro_launcher[^"]*"\s+.*\r?\n', ''

                        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
                        [System.IO.File]::WriteAllText($vdfPath, $content, $utf8NoBom)
                        Write-Host "[SUCCESS] Cleared Launch Options for Steam profile: $($userDir.Name)" -ForegroundColor Green
                        $cleanedCount++
                    }
                } catch {
                    Write-Host "[ERROR] Failed to update VDF for profile $($userDir.Name): $_" -ForegroundColor Red
                }
            }
        }

        if ($cleanedCount -gt 0) {
            Write-Host "[SUCCESS] Successfully cleared Launch Options in $cleanedCount Steam user profile(s)!" -ForegroundColor Green
        } else {
            Write-Host "[INFO] No Wuthering Waves Launch Options found in Steam profiles." -ForegroundColor Yellow
        }
    } else {
        Write-Host "[WARNING] Steam userdata directory not found at $UserDataDir." -ForegroundColor Yellow
    }

    # 4. Delete installation directory
    $InstallDir = Join-Path $env:LOCALAPPDATA "fk_kuro_launcher"
    if (Test-Path $InstallDir) {
        Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "[SUCCESS] Removed installation directory: $InstallDir" -ForegroundColor Green
    } else {
        Write-Host "[INFO] Installation directory not found: $InstallDir" -ForegroundColor Yellow
    }

    # 5. Restart Steam if steam.exe exists
    if (Test-Path $SteamExe) {
        Write-Host "[INFO] Restarting Steam..." -ForegroundColor Green
        Start-Process -FilePath $SteamExe
    }

    Write-Host ""
    Write-Host "[COMPLETE] fk_kuro_launcher uninstalled successfully! Launch Options cleared." -ForegroundColor Yellow
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

# 4. Close Steam and wait for complete process termination
$steamProcs = Get-Process -Name "steam", "steamwebhelper" -ErrorAction SilentlyContinue
if ($steamProcs) {
    Write-Host "[INFO] Closing Steam safely before applying configuration..." -ForegroundColor Yellow
    Stop-Process -Name "steam", "steamwebhelper" -Force -ErrorAction SilentlyContinue
    $steamProcs | ForEach-Object { try { [void]$_.WaitForExit(5000) } catch {} }
    Start-Sleep -Seconds 2
}

function Update-SteamVdf([string]$vdfText, [string]$appId, [string]$launchOptionsVal) {
    # Clean up any corrupted lines from previous script bugs
    $cleanText = $vdfText -replace '(?m)^\s*"[^"]*fk_kuro_launcher[^"]*"\s+.*\r?\n', ''
    $cleanText = $cleanText -replace '(?m)^\s*"LaunchOptions"\s+.*"LaunchOptions".*\r?\n', ''

    # Check if AppID "2775500" block exists
    if ($cleanText -match '(?i)"2775500"\s*\{') {
        if ($cleanText -match '(?i)("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*') {
            return $cleanText -replace '(?i)("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*', ('$1' + "`n`t`t`t`t`"LaunchOptions`"`t`t`"$launchOptionsVal`"")
        } else {
            return $cleanText -replace '(?i)("2775500"\s*\{)', ('$1' + "`n`t`t`t`t`"LaunchOptions`"`t`t`"$launchOptionsVal`"")
        }
    } else {
        $newBlock = "`"2775500`"`n`t`t`t`t{`n`t`t`t`t`t`"LaunchOptions`"`t`t`"$launchOptionsVal`"`n`t`t`t`t}"
        return $cleanText -replace '(?i)("Apps"|"apps")\s*\{', ('$1' + "`n`t`t`t`t" + $newBlock)
    }
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

                $content = Update-SteamVdf -vdfText $content -appId "2775500" -launchOptionsVal $vdfLaunchOptions

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
