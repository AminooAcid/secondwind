# Detaches the SecondWind node disk: flush volumes first, then close the
# iSCSI session. Safe to run when the session is already gone.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$TargetIqn
)

$ErrorActionPreference = "Stop"

function Write-Result([string]$Status, [string]$Message) {
    @{ status = $Status; message = $Message } | ConvertTo-Json -Compress
}

try {
    $session = Get-IscsiSession -ErrorAction SilentlyContinue |
        Where-Object { $_.TargetNodeAddress -eq $TargetIqn }

    if ($session) {
        # Flush every volume on this session's disk before disconnecting.
        $disk = $session | Get-Disk -ErrorAction SilentlyContinue
        if ($disk) {
            Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue |
                ForEach-Object {
                    if ($_.DriveLetter) {
                        Write-VolumeCache -DriveLetter $_.DriveLetter -ErrorAction SilentlyContinue
                    }
                }
        }

        Disconnect-IscsiTarget -NodeAddress $TargetIqn -Confirm:$false
    }

    Write-Result "ok" "SecondWind disk detached."
    exit 0
} catch {
    Write-Result "error" $_.Exception.Message
    exit 1
}
