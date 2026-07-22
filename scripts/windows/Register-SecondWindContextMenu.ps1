# Registers Explorer context-menu entries (current user only, no
# elevation): "Convert on node", "Compress on node". Each invokes the
# SecondWind companion in job mode with the selected file.
#
#   .\Register-SecondWindContextMenu.ps1 -CompanionPath "C:\...\secondwind-companion.exe"
# Remove with -Unregister.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$CompanionPath,
    [switch]$Unregister
)

$ErrorActionPreference = "Stop"

$entries = @(
    @{ Key = "SecondWind.Convert";  Label = "Convert to MP4 on node"; Preset = "convert-video" },
    @{ Key = "SecondWind.Compress"; Label = "Compress on node";       Preset = "compress" }
)

foreach ($entry in $entries) {
    $keyPath = "HKCU:\Software\Classes\*\shell\$($entry.Key)"
    if ($Unregister) {
        Remove-Item -Path $keyPath -Recurse -ErrorAction SilentlyContinue
        continue
    }

    New-Item -Path "$keyPath\command" -Force | Out-Null
    Set-ItemProperty -Path $keyPath -Name "(default)" -Value $entry.Label
    Set-ItemProperty -Path $keyPath -Name "Icon" -Value "`"$CompanionPath`""
    Set-ItemProperty -Path "$keyPath\command" -Name "(default)" `
        -Value "`"$CompanionPath`" --job $($entry.Preset) `"%1`""
}

if ($Unregister) {
    @{ status = "ok"; message = "SecondWind context menu removed." } | ConvertTo-Json -Compress
} else {
    @{ status = "ok"; message = "SecondWind context menu registered." } | ConvertTo-Json -Compress
}
