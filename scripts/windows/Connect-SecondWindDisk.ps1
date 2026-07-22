# Attaches the SecondWind node disk via the Windows iSCSI initiator.
# Invoked by the SecondWind companion; parameters all come from the node's
# mTLS-protected disk API — nothing here is machine-specific.
#
# On first attach the SecondWind disk is initialized and formatted NTFS.
# Only the disk belonging to this iSCSI session is ever touched.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$TargetAddress,
    [Parameter(Mandatory = $true)][string]$TargetIqn,
    [int]$TargetPort = 3260,
    [Parameter(Mandatory = $true)][string]$ChapUser,
    [Parameter(Mandatory = $true)][string]$ChapSecret,
    [string]$DriveLetter = ""
)

$ErrorActionPreference = "Stop"

function Write-Result([string]$Status, [string]$Message, [string]$Letter = "") {
    @{ status = $Status; message = $Message; drive_letter = $Letter } |
        ConvertTo-Json -Compress
}

try {
    # 1. Windows iSCSI service must be running (and stay available).
    Set-Service -Name MSiSCSI -StartupType Automatic
    Start-Service -Name MSiSCSI

    # 2. Register the portal and connect with one-way CHAP.
    $portal = Get-IscsiTargetPortal -TargetPortalAddress $TargetAddress -ErrorAction SilentlyContinue
    if (-not $portal) {
        New-IscsiTargetPortal -TargetPortalAddress $TargetAddress -TargetPortalPortNumber $TargetPort | Out-Null
    }

    $session = Get-IscsiSession -ErrorAction SilentlyContinue |
        Where-Object { $_.TargetNodeAddress -eq $TargetIqn }
    if (-not $session) {
        Connect-IscsiTarget -NodeAddress $TargetIqn `
            -TargetPortalAddress $TargetAddress -TargetPortalPortNumber $TargetPort `
            -AuthenticationType ONEWAYCHAP -ChapUsername $ChapUser -ChapSecret $ChapSecret `
            -IsPersistent $false | Out-Null
        $session = Get-IscsiSession | Where-Object { $_.TargetNodeAddress -eq $TargetIqn }
    }
    if (-not $session) { throw "The SecondWind disk connection was not established." }

    # 3. Find the disk behind this session (retry while Windows enumerates).
    $disk = $null
    for ($attempt = 0; $attempt -lt 20 -and -not $disk; $attempt++) {
        Start-Sleep -Milliseconds 500
        $disk = $session | Get-Disk -ErrorAction SilentlyContinue
    }
    if (-not $disk) { throw "The SecondWind disk did not appear." }

    if ($disk.IsOffline) { Set-Disk -Number $disk.Number -IsOffline $false }
    if ($disk.IsReadOnly) { Set-Disk -Number $disk.Number -IsReadOnly $false }

    # 4. First use: initialize + format the SecondWind disk (this session's
    # disk only). The node-side export is a partition SecondWind owns.
    if ($disk.PartitionStyle -eq "RAW") {
        Initialize-Disk -Number $disk.Number -PartitionStyle GPT
        $partition = New-Partition -DiskNumber $disk.Number -UseMaximumSize
        Format-Volume -Partition $partition -FileSystem NTFS `
            -NewFileSystemLabel "SecondWind" -Confirm:$false | Out-Null
    }

    # 5. Ensure a drive letter (requested one when free, else first free).
    $partition = Get-Partition -DiskNumber $disk.Number |
        Where-Object { $_.Type -ne "Reserved" } | Select-Object -First 1
    if (-not $partition.DriveLetter) {
        if ($DriveLetter -and -not (Get-PSDrive -Name $DriveLetter -ErrorAction SilentlyContinue)) {
            Set-Partition -DiskNumber $disk.Number -PartitionNumber $partition.PartitionNumber `
                -NewDriveLetter $DriveLetter
        } else {
            $partition | Add-PartitionAccessPath -AssignDriveLetter | Out-Null
        }
        $partition = Get-Partition -DiskNumber $disk.Number -PartitionNumber $partition.PartitionNumber
    }

    Write-Result "ok" "SecondWind disk attached." ([string]$partition.DriveLetter)
    exit 0
} catch {
    Write-Result "error" $_.Exception.Message
    exit 1
}
