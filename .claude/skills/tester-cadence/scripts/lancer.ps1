# Lance une instance de test ISOLÉE de Cadence (jamais celle de l'utilisateur) :
#  - identifiant app.cadence.desktop.qa → réglages, plugins et données séparés ;
#  - trousseau séparé (CADENCE_SECRETS_SERVICE) : les secrets de l'utilisateur ne sont jamais lus ;
#  - front Vite sur le port 1439 (l'app de l'utilisateur utilise 1420) ;
#  - WebView2 en débogage distant sur le port 9339 (piloté par cdp.mjs) ;
#  - build Rust dans src-tauri/target-agents (ne bloque pas le `tauri dev` de l'utilisateur).
# Usage : powershell -NoProfile -ExecutionPolicy Bypass -File lancer.ps1 [-Reinitialiser] [-Journal <fichier>]
param(
    [switch]$Reinitialiser,
    [string]$Journal = "$env:TEMP\cadence-qa.log"
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot\..\..\..\..").Path
$id = "app.cadence.desktop.qa"

if (Get-NetTCPConnection -LocalPort 9339 -State Listen -ErrorAction SilentlyContinue) {
    Write-Host "Une instance de test tourne déjà (port 9339)."
    exit 0
}
if ($Reinitialiser) {
    foreach ($d in @("$env:APPDATA\$id", "$env:LOCALAPPDATA\$id")) {
        if (Test-Path $d) { Remove-Item -Recurse -Force $d; Write-Host "Données de test effacées : $d" }
    }
}

$config = '{\"identifier\":\"' + $id + '\",\"productName\":\"Cadence QA\",\"build\":{\"devUrl\":\"http://localhost:1439\",\"beforeDevCommand\":\"pnpm dev --port 1439 --strictPort\"}}'
$cmd = "set CARGO_TARGET_DIR=$repo\src-tauri\target-agents&& set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9339&& set RUST_LOG=cadence_lib=info&& set CADENCE_SECRETS_SERVICE=$id&& pnpm tauri dev --config `"$config`""
Start-Process -FilePath "cmd.exe" -ArgumentList "/c", "$cmd > `"$Journal`" 2>&1" -WorkingDirectory $repo -WindowStyle Hidden
Write-Host "Compilation et lancement… (journal : $Journal)"

$limite = (Get-Date).AddMinutes(15)
while ((Get-Date) -lt $limite) {
    try {
        $pages = Invoke-RestMethod "http://127.0.0.1:9339/json/list" -TimeoutSec 2
        if ($pages | Where-Object { $_.type -eq "page" -and $_.url -like "*1439*" }) {
            Write-Host "PRÊT : Cadence QA tourne, WebView2 pilotable sur le port 9339."
            exit 0
        }
    } catch {}
    Start-Sleep -Seconds 3
}
Write-Host "ÉCHEC : l'instance n'a pas démarré en 15 min. Fin du journal :"
Get-Content $Journal -Tail 40
exit 1
