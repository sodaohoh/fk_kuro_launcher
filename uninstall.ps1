param(
    [string]$ExePath
)

# Delegates to install.ps1 with -Uninstall switch
$script = Join-Path $PSScriptRoot "install.ps1"
if (Test-Path $script) {
    & $script -Uninstall
} else {
    & { $(irm https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/main/install.ps1) } -Uninstall
}
