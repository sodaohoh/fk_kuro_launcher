param(
    [string]$ExePath,
    [switch]$Uninstall
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls11 -bor [Net.SecurityProtocolType]::Tls
function Set-VdfLaunchOptions {
    param(
        [string]$VdfText,
        [string]$AppId = "2775500",
        [string]$LaunchOptionsVal
    )

    # Clean up any corrupted lines from previous script bugs
    $cleanLines = $VdfText -split "\r?\n" | Where-Object { $_ -notmatch '^\s*"[^"]*fk_kuro_launcher[^"]*"\s+.*' -and $_ -notmatch '^\s*"C:\\\\Users\\\\.*?' }

    $inAppBlock = $false
    $appBlockStartLine = -1
    $appBlockEndLine = -1
    $braceDepth = 0
    $foundLaunchOptionsLine = -1

    for ($i = 0; $i -lt $cleanLines.Count; $i++) {
        $line = $cleanLines[$i]

        if (-not $inAppBlock) {
            if ($line -match '(?i)^\s*"' + $AppId + '"\s*$') {
                $inAppBlock = $true
                $appBlockStartLine = $i
            }
        } else {
            if ($line -contains '{' -or $line -match '\{') {
                $braceDepth += ($line.ToCharArray() | Where-Object { $_ -eq '{' }).Count
            }
            if ($line -contains '}' -or $line -match '\}') {
                $braceDepth -= ($line.ToCharArray() | Where-Object { $_ -eq '}' }).Count
            }

            if ($line -match '^\s*"LaunchOptions"\s+') {
                $foundLaunchOptionsLine = $i
            }

            if ($braceDepth -eq 0 -and $appBlockStartLine -ge 0) {
                $appBlockEndLine = $i
                break
            }
        }
    }

    # Case A: AppID block found
    if ($appBlockStartLine -ge 0) {
        if ([string]::IsNullOrWhiteSpace($LaunchOptionsVal)) {
            # Uninstallation mode: remove LaunchOptions line if found
            if ($foundLaunchOptionsLine -ge 0) {
                $cleanLines[$foundLaunchOptionsLine] = ""
            }
            return (($cleanLines | Where-Object { $_ -ne "" }) -join "`r`n")
        }

        if ($foundLaunchOptionsLine -ge 0) {
            # Update existing LaunchOptions line
            $cleanLines[$foundLaunchOptionsLine] = "`t`t`t`t`"LaunchOptions`"`t`t`"$LaunchOptionsVal`""
        } else {
            # Insert LaunchOptions right after the opening brace '{' of AppID block
            $insertIdx = $appBlockStartLine + 1
            if ($insertIdx -lt $cleanLines.Count -and $cleanLines[$insertIdx] -match '^\s*\{') {
                $insertIdx++
            }
            $cleanLines = $cleanLines[0..($insertIdx-1)] + "`t`t`t`t`"LaunchOptions`"`t`t`"$LaunchOptionsVal`"" + $cleanLines[$insertIdx..($cleanLines.Count-1)]
        }
        return ($cleanLines -join "`r`n")
    }

    if ([string]::IsNullOrWhiteSpace($LaunchOptionsVal)) {
        return ($cleanLines -join "`r`n")
    }

    # Case B: AppID block not found -> Check if "Apps" or "apps" block exists
    $appsIdx = -1
    for ($i = 0; $i -lt $cleanLines.Count; $i++) {
        if ($cleanLines[$i] -match '(?i)^\s*"(Apps|apps)"\s*$') {
            $appsIdx = $i
            break
        }
    }

    if ($appsIdx -ge 0) {
        $insertIdx = $appsIdx + 1
        if ($insertIdx -lt $cleanLines.Count -and $cleanLines[$insertIdx] -match '^\s*\{') {
            $insertIdx++
        }
        $newAppBlock = @(
            "`t`t`t`t`"$AppId`"",
            "`t`t`t`t{",
            "`t`t`t`t`t`"LaunchOptions`"`t`t`"$LaunchOptionsVal`"",
            "`t`t`t`t}"
        )
        $cleanLines = $cleanLines[0..($insertIdx-1)] + $newAppBlock + $cleanLines[$insertIdx..($cleanLines.Count-1)]
        return ($cleanLines -join "`r`n")
    }

    # Case C: "Apps" block not found -> Check if "Steam" block exists
    $steamIdx = -1
    for ($i = 0; $i -lt $cleanLines.Count; $i++) {
        if ($cleanLines[$i] -match '(?i)^\s*"Steam"\s*$') {
            $steamIdx = $i
            break
        }
    }

    if ($steamIdx -ge 0) {
        $insertIdx = $steamIdx + 1
        if ($insertIdx -lt $cleanLines.Count -and $cleanLines[$insertIdx] -match '^\s*\{') {
            $insertIdx++
        }
        $newAppsBlock = @(
            "`t`t`t`"Apps`"",
            "`t`t`t{",
            "`t`t`t`t`"$AppId`"",
            "`t`t`t`t{",
            "`t`t`t`t`t`"LaunchOptions`"`t`t`"$LaunchOptionsVal`"",
            "`t`t`t`t}",
            "`t`t`t}"
        )
        $cleanLines = $cleanLines[0..($insertIdx-1)] + $newAppsBlock + $cleanLines[$insertIdx..($cleanLines.Count-1)]
        return ($cleanLines -join "`r`n")
    }

    return $VdfText
}
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

                        $content = Set-VdfLaunchOptions -VdfText $content -AppId "2775500" -LaunchOptionsVal ""

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
    $downloadSuccess = $false

    try {
        if (Get-Command "curl.exe" -ErrorAction SilentlyContinue) {
            & curl.exe -sSL -f -o $InstalledExe $downloadUrl
            if ($LASTEXITCODE -eq 0 -and (Test-Path $InstalledExe) -and (Get-Item $InstalledExe).Length -gt 1000) {
                $downloadSuccess = $true
            }
        }
        if (-not $downloadSuccess) {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $InstalledExe -UserAgent "fk_kuro_launcher" -UseBasicParsing -ErrorAction Stop
            if ((Test-Path $InstalledExe) -and (Get-Item $InstalledExe).Length -gt 1000) {
                $downloadSuccess = $true
            }
        }
    } catch {}

    if ($downloadSuccess) {
        Write-Host "[SUCCESS] Installed fk_kuro_launcher.exe to $InstalledExe" -ForegroundColor Green
    } else {
        $localCandidates = @(
            (Join-Path $PSScriptRoot "target\release\fk_kuro_launcher.exe"),
            (Join-Path $PSScriptRoot "fk_kuro_launcher.exe"),
            (Join-Path (Get-Location) "target\release\fk_kuro_launcher.exe"),
            (Join-Path (Get-Location) "fk_kuro_launcher.exe")
        )
        $foundLocal = $null
        foreach ($cand in $localCandidates) {
            if (-not [string]::IsNullOrWhiteSpace($cand) -and (Test-Path $cand)) {
                $foundLocal = (Get-Item $cand).FullName
                break
            }
        }

        if ($foundLocal) {
            Copy-Item -Path $foundLocal -Destination $InstalledExe -Force
            Write-Host "[SUCCESS] Installed local build executable from $foundLocal to $InstalledExe" -ForegroundColor Green
        } elseif (Test-Path $InstalledExe) {
            Write-Host "[WARNING] Could not download latest release from GitHub (Repo may be Private)." -ForegroundColor Yellow
            Write-Host "[INFO] Keeping existing executable at $InstalledExe" -ForegroundColor Green
        } else {
            Write-Host "[ERROR] Could not download from GitHub (Repo is Private or offline) and no local binary found." -ForegroundColor Red
            Write-Host "[INFO] Please build first via 'cargo build --release' or make repo Public in GitHub Settings." -ForegroundColor Yellow
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

                $content = Set-VdfLaunchOptions -VdfText $content -AppId "2775500" -LaunchOptionsVal $vdfLaunchOptions

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
