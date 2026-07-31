[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

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
$steamProc = Get-Process -Name "steam" -ErrorAction SilentlyContinue
if ($steamProc) {
    Write-Host "[INFO] Closing Steam to clear launch options safely..." -ForegroundColor Yellow
    Stop-Process -Name "steam" -Force
    [void]$steamProc.WaitForExit(10000)
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

                    $content = $content -replace '("2775500"\s*\{[^{}]*?)\s*"LaunchOptions"\s+.*', '$1'

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
