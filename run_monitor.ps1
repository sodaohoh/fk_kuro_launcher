[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# PowerShell Monitor Script for Wuthering Waves (Steam Version)
$DefaultLogPath = "C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves\Client\Saved\Logs\Client.log"
$DefaultGameExe = "C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe"

# Use built-in $args array to capture all positional arguments passed by Steam's %command%
if ($args -and $args.Count -gt 0) {
    $GameExe = $args[0]
    $GameArgsList = if ($args.Count -gt 1) { $args[1..($args.Count-1)] } else { @() }
} else {
    $GameExe = $DefaultGameExe
    $GameArgsList = @()
}

$LogPath = $DefaultLogPath

$lut = @{
    0xb4='['; 0xb2=']'; 0x8b='.'; 0x9f=':'; 0xc2='-'; 0xaf="`n"; 0xe2="`r"
    0x95='0'; 0xde='1'; 0x97='2'; 0xdc='3'; 0x91='4'; 0xda='5'; 0x93='6'; 0xd8='7'; 0x9d='8'; 0xd6='9'
    0xfd=' '; 0x85=' '; 0xae='A'; 0xac='C'; 0xb8='W'; 0xe9='L'; 0xa4='K'; 0x9c='S'
    0xf7='R'; 0xf1='T'; 0xa2='M'; 0xaa='E'; 0xe1='V'; 0xf5='P'; 0xed='H'; 0xeb='N'
    0x8e='a'; 0x8c='c'; 0xc1='d'; 0x8a='e'; 0xc3='f'; 0x88='g'; 0xcd='h'; 0x86='i'
    0xc9='l'; 0x82='m'; 0xcb='n'; 0x80='o'; 0xd5='p'; 0xd7='r'; 0xbc='s'; 0xd1='t'
    0x9a='u'; 0xd3='v'; 0x98='w'; 0xdd='x'; 0x96='y'
}

Write-Host "[INFO] Steam Wrapper Monitor Started." -ForegroundColor Green
Write-Host "[INFO] Executable Target: $GameExe" -ForegroundColor Gray
Write-Host "[INFO] Log Target: $LogPath" -ForegroundColor Gray

# Initialize offset to current end of log file
$offset = 0
if (Test-Path $LogPath) {
    $offset = (Get-Item $LogPath).Length
}

# Function to launch game process
function Start-GameChildProcess {
    $workDir = Split-Path $GameExe
    if ($GameArgsList.Count -gt 0) {
        return Start-Process -FilePath $GameExe -ArgumentList $GameArgsList -WorkingDirectory $workDir -PassThru
    } else {
        return Start-Process -FilePath $GameExe -WorkingDirectory $workDir -PassThru
    }
}

# Spawn initial game process as child of wrapper
$gameProc = Start-GameChildProcess
Write-Host "[INFO] Game spawned with PID: $($gameProc.Id)" -ForegroundColor Green

$isHotfixRestart = $false

while ($true) {
    # Check if game process is still alive
    if ($gameProc.HasExited) {
        if ($isHotfixRestart) {
            Write-Host "[WARN] Hotfix restart detected! Respawning game process in 3s..." -ForegroundColor Yellow
            Start-Sleep -Seconds 3
            $gameProc = Start-GameChildProcess
            Write-Host "[INFO] Game respawned with PID: $($gameProc.Id)" -ForegroundColor Green
            $isHotfixRestart = $false
            if (Test-Path $LogPath) {
                $offset = (Get-Item $LogPath).Length
            }
            Start-Sleep -Seconds 5
            continue
        } else {
            Write-Host "[INFO] Normal game exit detected. Steam Wrapper shutting down." -ForegroundColor Gray
            break
        }
    }

    # Monitor Client.log for hotfix restart triggers
    if (Test-Path $LogPath) {
        try {
            $stream = [System.IO.File]::Open($LogPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
            $length = $stream.Length

            if ($length -lt $offset) { 
                $offset = 0
            }

            if ($length -gt $offset) {
                $stream.Seek($offset, [System.IO.SeekOrigin]::Begin) | Out-Null
                $buffer = New-Object byte[] ($length - $offset)
                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -gt 0) {
                    $offset += $read
                    $chars = foreach ($b in $buffer[0..($read-1)]) {
                        if ($lut.ContainsKey([int]$b)) { $lut[[int]$b] } else { [char]$b }
                    }
                    $text = -join $chars

                    if ($text -match "Engine exit requested|NeedRestart") {
                        Write-Host "[WARN] Hotfix restart requested by game engine!" -ForegroundColor Yellow
                        $isHotfixRestart = $true
                    }
                }
            }
            $stream.Close()
        } catch {}
    }
    Start-Sleep -Seconds 1
}
