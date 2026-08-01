$SteamDir = "C:\Program Files (x86)\Steam"
$files = Get-ChildItem -Path $SteamDir -Recurse -Include "*.vdf", "*.acf" -ErrorAction SilentlyContinue | Where-Object { $_.FullName -notmatch "steamapps\\common" }

foreach ($file in $files) {
    try {
        $content = Get-Content -Path $file.FullName -Raw -Encoding UTF8 -ErrorAction SilentlyContinue
        if ($content -match "2775500") {
            Write-Host "FOUND 2775500 in:" $file.FullName
            if ($content -match "LaunchOptions") {
                Write-Host "  -> Contains LaunchOptions!"
                # Print lines around 2775500
                $lines = Get-Content -Path $file.FullName
                for ($i = 0; $i -lt $lines.Count; $i++) {
                    if ($lines[$i] -match "2775500") {
                        $start = [Math]::Max(0, $i - 2)
                        $end = [Math]::Min($lines.Count - 1, $i + 8)
                        Write-Host "--- Context in $($file.Name) ---"
                        for ($j = $start; $j -le $end; $j++) {
                            Write-Host $lines[$j]
                        }
                    }
                }
            }
        }
    } catch {}
}
