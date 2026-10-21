# Trousseau de l'instance de test « Cadence QA » dans le Gestionnaire d'identifiants Windows.
#
# Cadence range ses secrets sous le service `CADENCE_SECRETS_SERVICE` (`app.cadence.desktop.qa`
# pour l'instance de test). Le magasin Windows de keyring les nomme `<compte>.<service>`
# (`github.app.cadence.desktop.qa`, `plugin/gestion-projet/jeton.app.cadence.desktop.qa`) ;
# `cmdkey /list` les montre en `LegacyGeneric:target=<nom>` (« Cible : » ou « Target: »).
#
# Garde-fou : seul un service qui se termine par `.qa` est accepté, et seules les cibles qui se
# terminent par `.<service>` sont supprimées. Les entrées du service de l'app
# (`<compte>.app.cadence.desktop`) ne sont jamais touchées.
#
# Vérification sans rien supprimer : powershell -File trousseau.ps1 -Tester

param([switch]$Tester)

# Cibles du service `$Service` dans la sortie de `cmdkey /list`.
function Get-CiblesTrousseau([string[]]$Lignes, [string]$Service) {
    foreach ($ligne in $Lignes) {
        if ($ligne -match 'target=(.+)$') {
            $cible = $Matches[1].Trim()
            if ($cible.EndsWith(".$Service", [StringComparison]::Ordinal)) { $cible }
        }
    }
}

# Supprime les entrées du trousseau de l'instance de test.
function Clear-TrousseauQa([string]$Service) {
    if (-not $Service.EndsWith(".qa", [StringComparison]::Ordinal)) {
        throw "Refus : « $Service » n'est pas un service de test (.qa) ; le trousseau de l'app n'est jamais effacé."
    }
    $cibles = @(Get-CiblesTrousseau -Lignes (cmdkey /list) -Service $Service)
    if ($cibles.Count -eq 0) {
        Write-Host "Trousseau de test déjà vide ($Service)."
        return
    }
    foreach ($cible in $cibles) {
        # Les identifiants génériques s'effacent par leur nom complet (`LegacyGeneric:target=…`),
        # selon les versions de Windows par le nom seul.
        cmdkey "/delete:LegacyGeneric:target=$cible" | Out-Null
        if ($LASTEXITCODE -ne 0) { cmdkey "/delete:$cible" | Out-Null }
        if ($LASTEXITCODE -eq 0) { Write-Host "Trousseau de test effacé : $cible" }
        else { Write-Host "AVERTISSEMENT : entrée du trousseau non effacée : $cible" }
    }
}

if ($Tester) {
    $exemple = @(
        "Informations d'identification actuellement stockées :",
        "    Cible : LegacyGeneric:target=github.app.cadence.desktop",
        "    Type : Générique",
        "    Cible : LegacyGeneric:target=github.app.cadence.desktop.qa",
        "    Target: LegacyGeneric:target=plugin/gestion-projet/jeton.app.cadence.desktop.qa",
        "    Cible : LegacyGeneric:target=autre.qa.app.cadence.desktop",
        "    Cible : Domain:target=serveur"
    )
    $trouvees = @(Get-CiblesTrousseau -Lignes $exemple -Service "app.cadence.desktop.qa")
    $attendues = @("github.app.cadence.desktop.qa", "plugin/gestion-projet/jeton.app.cadence.desktop.qa")
    $refus = $false
    try { Clear-TrousseauQa -Service "app.cadence.desktop" } catch { $refus = $true }
    if ((Compare-Object $trouvees $attendues) -or -not $refus) {
        Write-Host "ÉCHEC : $($trouvees -join ', ')"
        exit 1
    }
    Write-Host "OK : seules les entrées du service de test sont visées ; le service de l'app est refusé."
}
