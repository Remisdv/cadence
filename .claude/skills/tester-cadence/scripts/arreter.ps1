# Arrête l'instance de test isolée (Cadence QA) sans toucher à l'app de l'utilisateur.
# Usage : powershell -NoProfile -ExecutionPolicy Bypass -File arreter.ps1 [-Effacer]
param([switch]$Effacer)
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
$OutputEncoding = [Console]::OutputEncoding
$repo = (Resolve-Path "$PSScriptRoot\..\..\..\..").Path
$exe = "$repo\src-tauri\target-agents\debug\cadence.exe"
$pidFile = "$env:TEMP\cadence-qa.pid"

# 1. L'arbre lancé par lancer.ps1 (powershell → cmd → pnpm → tauri/cargo/vite → cadence.exe).
if (Test-Path $pidFile) {
    $lanceur = Get-Content $pidFile | Select-Object -First 1
    if ($lanceur -and (Get-Process -Id $lanceur -ErrorAction SilentlyContinue)) {
        taskkill.exe /T /F /PID $lanceur 2>&1 | Out-Null
        Write-Host "Arrêté : lanceur et ses processus ($lanceur)"
    }
    Remove-Item $pidFile -ErrorAction SilentlyContinue
}
# 2. Rattrapage : l'exe de test et ce qui écoute encore sur les ports de test (jamais 1420).
Get-Process cadence -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | ForEach-Object {
    Stop-Process -Id $_.Id -Force; Write-Host "Arrêté : cadence.exe ($($_.Id))"
}
foreach ($port in 1439, 9339) {
    Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue | ForEach-Object {
        $p = Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue
        if ($p -and $p.ProcessName -in @("node", "msedgewebview2")) { Stop-Process -Id $p.Id -Force; Write-Host "Arrêté : $($p.ProcessName) (port $port)" }
    }
}
if ($Effacer) {
    foreach ($d in @("$env:APPDATA\app.cadence.desktop.qa", "$env:LOCALAPPDATA\app.cadence.desktop.qa")) {
        if (Test-Path $d) { Remove-Item -Recurse -Force $d; Write-Host "Données de test effacées : $d" }
    }
}
Write-Host "Instance de test arrêtée."
