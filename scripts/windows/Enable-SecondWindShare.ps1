# Creates (idempotently) the host's SecondWind file share:
#   - a dedicated local account (never the user's own login)
#   - a folder (default: the user profile's SecondWind folder)
#   - an SMB share restricted to that account
#
# Requires elevation; the companion launches it with a UAC prompt the first
# time. The account password arrives via -AccountPasswordFile (elevated
# processes don't inherit the caller's environment, and argv is visible in
# process listings); the file is deleted after reading. -AccountPassword
# remains for manual/support use only.
# Prints a one-line JSON result with the share name and account.

# The plain-text-to-SecureString conversion is intentional: the password
# reaches this elevated script through an owner-only, self-deleting file
# (elevated processes don't inherit env vars; argv is world-readable), and
# New-LocalUser only accepts a SecureString.
[Diagnostics.CodeAnalysis.SuppressMessageAttribute(
    "PSAvoidUsingConvertToSecureStringWithPlainText", "",
    Justification = "Password is delivered via an owner-only temp file; see docs/DECISIONS.md (BUG-012).")]
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$FolderPath,
    [string]$AccountPassword = "",
    [string]$AccountPasswordFile = "",
    [string]$ShareName = "SecondWind",
    [string]$AccountName = "SecondWindShare"
)

$ErrorActionPreference = "Stop"

function Write-Result([string]$Status, [string]$Message) {
    @{ status = $Status; message = $Message; share_name = $ShareName; account = $AccountName } |
        ConvertTo-Json -Compress
}

try {
    if ($AccountPasswordFile) {
        $AccountPassword = (Get-Content -Path $AccountPasswordFile -Raw).Trim()
        Remove-Item -Path $AccountPasswordFile -Force -ErrorAction SilentlyContinue
    }
    if (-not $AccountPassword) {
        throw "No account password was provided."
    }

    # 1. Dedicated local account (no interactive logon rights needed).
    $secure = ConvertTo-SecureString $AccountPassword -AsPlainText -Force
    $existing = Get-LocalUser -Name $AccountName -ErrorAction SilentlyContinue
    if ($existing) {
        Set-LocalUser -Name $AccountName -Password $secure
    } else {
        New-LocalUser -Name $AccountName -Password $secure -PasswordNeverExpires `
            -Description "SecondWind node file access (dedicated account)" | Out-Null
    }

    # 2. Folder.
    if (-not (Test-Path $FolderPath)) {
        New-Item -ItemType Directory -Path $FolderPath | Out-Null
    }
    $acl = Get-Acl $FolderPath
    $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
        $AccountName, "Modify", "ContainerInherit,ObjectInherit", "None", "Allow")
    $acl.SetAccessRule($rule)
    Set-Acl $FolderPath $acl

    # 3. Share, restricted to the dedicated account.
    $share = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
    if (-not $share) {
        New-SmbShare -Name $ShareName -Path $FolderPath -FullAccess $AccountName | Out-Null
    } elseif ($share.Path -ne $FolderPath) {
        throw "A different '$ShareName' share already exists at $($share.Path)."
    }

    Write-Result "ok" "SecondWind share ready."
    exit 0
} catch {
    Write-Result "error" $_.Exception.Message
    exit 1
}
