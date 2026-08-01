function Set-VdfLaunchOptions {
    param(
        [string]$VdfText,
        [string]$AppId = "2775500",
        [string]$LaunchOptionsVal
    )

    # 1. Clean up any corrupted lines from previous script bugs
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
            # Track brace depth inside AppID block
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

# Test on localconfig.vdf files
$vdf1 = "C:\Program Files (x86)\Steam\userdata\144599539\config\localconfig.vdf"
$vdf2 = "C:\Program Files (x86)\Steam\userdata\1660693104\config\localconfig.vdf"

foreach ($vdf in @($vdf1, $vdf2)) {
    if (Test-Path $vdf) {
        $txt = Get-Content -Path $vdf -Raw -Encoding UTF8
        $out = Set-VdfLaunchOptions -VdfText $txt -AppId "2775500" -LaunchOptionsVal "\"C:\\Test\\fk.exe\" %command%"
        Write-Host "Tested $vdf -> Result length: $($out.Length)"
    }
}
