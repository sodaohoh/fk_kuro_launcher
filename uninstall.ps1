param(
    [string]$ExePath
)

# Delegates to install.ps1 with -Uninstall switch
$script = Join-Path $PSScriptRoot "install.ps1"
if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot) -and (Test-Path $script)) {
    & $script -Uninstall
} else {
    & ([scriptblock]::Create((Invoke-RestMethod "https://raw.githubusercontent.com/sodaohoh/fk_kuro_launcher/master/install.ps1"))) -Uninstall
}
