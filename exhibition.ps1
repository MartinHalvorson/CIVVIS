# exhibition.ps1 - keep the civvis spectate exhibition running on the latest
# code. Loop: revive dead processes; fetch origin/main and stage a fresh
# release build when new commits land; swap binaries in the between-games
# window (winner on screen) so every new game boots on the newest code.
# Run hidden:  Start-Process powershell -WindowStyle Hidden -ArgumentList
#              "-ExecutionPolicy","Bypass","-File","exhibition.ps1"
param([int]$Port = 8765, [int]$PollSec = 20)

$repo = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repo
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"
$log = "$repo\exhibition-supervisor.log"
New-Item -ItemType Directory -Force "$repo\bin-run" | Out-Null

function Log($msg) {
    Add-Content -Encoding utf8 $log "$(Get-Date -Format 'MM-dd HH:mm:ss') $msg"
}

function Start-Gui {
    Start-Process -FilePath "$repo\bin-run\civvis-gui.exe" `
        -ArgumentList "play","--spectate","--no-open","--port","$Port" `
        -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$repo\civvis-play.log" `
        -RedirectStandardError "$repo\civvis-play.err.log"
    Log "gui launched on :$Port"
}

function Start-Evolve {
    Start-Process -FilePath "$repo\bin-run\civvis-evolve.exe" `
        -ArgumentList "evolve","--threads","12","--pop","16","--games","8","--turns","160" `
        -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$repo\evolved\evolve.log" `
        -RedirectStandardError "$repo\evolved\evolve.err.log"
    Log "evolve launched"
}

Log "supervisor started (port $Port, poll ${PollSec}s)"
while ($true) {
    try {
        # 1. revive anything that died
        $guiUp = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
        if (-not $guiUp) {
            if (Test-Path "$repo\bin-run\civvis-next.exe") {
                Copy-Item "$repo\bin-run\civvis-next.exe" "$repo\bin-run\civvis-gui.exe" -Force
            }
            if (Test-Path "$repo\bin-run\civvis-gui.exe") { Start-Gui }
        }
        $evoUp = Get-Process civvis-evolve -ErrorAction SilentlyContinue
        if (-not $evoUp -and (Test-Path "$repo\bin-run\civvis-evolve.exe")) { Start-Evolve }

        # 2. new commits upstream? pull + build + stage (skip if checkout dirty:
        #    a parallel session is mid-work; retry next round)
        git fetch -q origin main 2>$null
        $local = git rev-parse HEAD
        $remote = git rev-parse origin/main
        $dirty = git status --porcelain --untracked-files=no
        if ($local -ne $remote -and -not $dirty) {
            git pull --rebase -q 2>$null
            if ($LASTEXITCODE -ne 0) {
                git rebase --abort 2>$null
                Log "pull failed; will retry"
            } else {
                Log "pulled $((git rev-parse --short HEAD)); building"
                & $cargo build --release 2>$null | Out-Null
                if ($LASTEXITCODE -eq 0) {
                    Copy-Item "$repo\target\release\civvis.exe" "$repo\bin-run\civvis-next.exe" -Force
                    Log "staged new build"
                } else {
                    Log "build FAILED for $((git rev-parse --short HEAD))"
                }
            }
        }

        # 3. staged build + game over (restart countdown window) -> swap now,
        #    so the next game boots on the latest code
        if (Test-Path "$repo\bin-run\civvis-next.exe") {
            $st = $null
            try { $st = Invoke-RestMethod "http://localhost:$Port/state" -TimeoutSec 5 } catch {}
            $gameOver = ($null -ne $st) -and ($null -ne $st.winner)
            if ($gameOver -or ($null -eq $st)) {
                Get-Process civvis-gui -ErrorAction SilentlyContinue | Stop-Process -Force
                Get-Process civvis-evolve -ErrorAction SilentlyContinue | Stop-Process -Force
                Start-Sleep -Milliseconds 500
                Copy-Item "$repo\bin-run\civvis-next.exe" "$repo\bin-run\civvis-gui.exe" -Force
                Copy-Item "$repo\bin-run\civvis-next.exe" "$repo\bin-run\civvis-evolve.exe" -Force
                Remove-Item "$repo\bin-run\civvis-next.exe" -Force
                Start-Gui
                Start-Evolve
                Log "swapped to latest build between games"
            }
        }
    } catch {
        Log "supervisor error: $_"
    }
    Start-Sleep -Seconds $PollSec
}
