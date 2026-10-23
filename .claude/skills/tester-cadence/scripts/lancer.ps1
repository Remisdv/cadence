# Lance une instance de test ISOLÉE de Cadence (jamais celle de l'utilisateur) :
#  - identifiant app.cadence.desktop.qa → réglages, plugins et données séparés ;
#  - trousseau séparé (CADENCE_SECRETS_SERVICE) : les secrets de l'utilisateur ne sont jamais lus ;
#  - front Vite sur le port 1439 (l'app de l'utilisateur utilise 1420) ;
#  - WebView2 en débogage distant sur le port 9339 (piloté par cdp.mjs) ;
#  - build Rust dans src-tauri/target-agents (ne bloque pas le `tauri dev` de l'utilisateur).
# Journal horodaté (compilation, Vite, traces Rust de l'app et des serveurs de plugin) dans -Journal.
# Usage : powershell -NoProfile -ExecutionPolicy Bypass -File lancer.ps1 [-Reinitialiser]
#           [-Journal <fichier>] [-RustLog <filtre>] [-Minutes <n>]
param(
    [switch]$Reinitialiser,
    [string]$Journal = "$env:TEMP\cadence-qa.log",
    # Traces de l'app en debug (plugins, sessions d'IA, passerelle MCP), le reste en warn.
    [string]$RustLog = "warn,cadence_lib=debug",
    [int]$Minutes = 20,
    # Recompiler et relancer l'app quand src-tauri change. Désactivé par défaut : un autre agent
    # qui modifie le Rust redémarrerait l'instance en pleine recette.
    [switch]$Surveiller
)
# Sortie UTF-8 : sans cela, Windows PowerShell 5.1 écrit « Donn�es », « PR�T ».
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
$OutputEncoding = [Console]::OutputEncoding
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot\..\..\..\..").Path
$id = "app.cadence.desktop.qa"
$pidFile = "$env:TEMP\cadence-qa.pid"

if (Get-NetTCPConnection -LocalPort 9339 -State Listen -ErrorAction SilentlyContinue) {
    Write-Host "Une instance de test tourne déjà (port 9339). Pour repartir de zéro : arreter.ps1 puis lancer.ps1 -Reinitialiser."
    exit 0
}
if (Get-NetTCPConnection -LocalPort 1439 -State Listen -ErrorAction SilentlyContinue) {
    Write-Host "ÉCHEC : le port 1439 (Vite de l'instance de test) est déjà pris. Lancez arreter.ps1."
    exit 1
}
. "$PSScriptRoot\trousseau.ps1"
if ($Reinitialiser) {
    foreach ($d in @("$env:APPDATA\$id", "$env:LOCALAPPDATA\$id")) {
        if (Test-Path $d) { Remove-Item -Recurse -Force $d; Write-Host "Données de test effacées : $d" }
    }
    # Trousseau de l'instance de test (jeton GitHub, secrets des plugins) : sans cela, la
    # connexion de la recette précédente est reprise et l'onboarding n'est jamais « à neuf ».
    Clear-TrousseauQa -Service $id
}

# Configuration Tauri de l'instance de test, passée par fichier (pas de JSON sur la ligne de commande).
$config = "$env:TEMP\cadence-qa.tauri.json"
$json = '{"identifier":"' + $id + '","productName":"Cadence QA","build":{"devUrl":"http://localhost:1439","beforeDevCommand":"pnpm dev --port 1439 --strictPort"}}'
[IO.File]::WriteAllText($config, $json, (New-Object System.Text.UTF8Encoding $false))
[IO.File]::WriteAllText($Journal, "", (New-Object System.Text.UTF8Encoding $false))

# Variables héritées par le processus lancé (ce script tourne dans son propre powershell).
$env:CARGO_TARGET_DIR = "$repo\src-tauri\target-agents"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9339"
$env:RUST_LOG = $RustLog
$env:CADENCE_SECRETS_SERVICE = $id
$env:NO_COLOR = "1"
$env:CARGO_TERM_COLOR = "never"

$optionWatch = if ($Surveiller) { "" } else { "--no-watch " }

# Lanceur caché : chaque ligne (stdout + stderr) est horodatée dans le journal.
$wrapper = @"
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding `$false
`$enc = New-Object System.Text.UTF8Encoding `$false
Set-Location '$repo'
cmd.exe /c 'pnpm tauri dev $optionWatch--config "$config" 2>&1' | ForEach-Object {
    # Sans les codes couleur ANSI de cargo / tauri.
    `$ligne = `$_ -replace '\x1b\[[0-9;?]*[A-Za-z]', ''
    [IO.File]::AppendAllText('$Journal', ('{0:HH:mm:ss} {1}' -f (Get-Date), `$ligne) + "``r``n", `$enc)
}
[IO.File]::AppendAllText('$Journal', ('{0:HH:mm:ss} [lanceur] tauri dev terminé (code {1})' -f (Get-Date), `$LASTEXITCODE) + "``r``n", `$enc)
"@
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($wrapper))
$proc = Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", $encoded -WorkingDirectory $repo -WindowStyle Hidden -PassThru
Set-Content -Path $pidFile -Value $proc.Id -Encoding ascii
Write-Host "Compilation et lancement… (journal : $Journal, processus $($proc.Id))"

$erreurs = 'error(\[E\d+\])?:|could not compile|failed to build|ELIFECYCLE|already in use|EADDRINUSE|ERR_PNPM'
$debut = Get-Date
$limite = $debut.AddMinutes($Minutes)
$dernierPoint = $debut
while ((Get-Date) -lt $limite) {
    try {
        $pages = Invoke-RestMethod "http://127.0.0.1:9339/json/list" -TimeoutSec 2
        if ($pages | Where-Object { $_.type -eq "page" -and $_.url -like "*1439*" }) {
            $s = [int]((Get-Date) - $debut).TotalSeconds
            Write-Host "PRÊT : Cadence QA tourne ($s s), WebView2 pilotable sur le port 9339. Journal : $Journal"
            exit 0
        }
    } catch {}
    if ($proc.HasExited) {
        Write-Host "ÉCHEC : le lancement s'est arrêté avant l'ouverture de la fenêtre."
        $lignes = @(Select-String -Path $Journal -Pattern $erreurs -Context 0, 6 -Encoding utf8 | Select-Object -First 6)
        if ($lignes | Where-Object { $_.Line -match 'error\[E\d+\]|could not compile' }) {
            Write-Host "La compilation Rust a échoué (détail : $Journal) :"
        } elseif ($lignes) {
            Write-Host "Erreur au lancement (détail : $Journal) :"
        }
        if ($lignes) {
            $lignes | ForEach-Object { @($_.Line) + $_.Context.PostContext } | Select-Object -Unique -First 40
        } else {
            Write-Host "Fin du journal :"; Get-Content $Journal -Tail 40 -Encoding utf8
        }
        Remove-Item $pidFile -ErrorAction SilentlyContinue
        exit 1
    }
    if (((Get-Date) - $dernierPoint).TotalSeconds -ge 60) {
        $dernierPoint = Get-Date
        $last = Get-Content $Journal -Tail 1 -Encoding utf8
        Write-Host ("… {0} min : {1}" -f [int]((Get-Date) - $debut).TotalMinutes, $last)
    }
    Start-Sleep -Seconds 3
}
Write-Host "ÉCHEC : l'instance n'a pas démarré en $Minutes min (arreter.ps1 pour nettoyer). Fin du journal :"
Get-Content $Journal -Tail 40 -Encoding utf8
exit 1
