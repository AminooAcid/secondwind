# Windows Scripts

PowerShell helpers the SecondWind companion invokes. They are bundled with
the host installer next to the companion executable (`scripts/` folder) and
never run standalone in the product flow.

- `Connect-SecondWindDisk.ps1` — attaches the node's data disk through the
  Windows iSCSI initiator (one-way CHAP), initializes/formats the SecondWind
  disk NTFS on first use (only the disk from this session), and assigns a
  drive letter. Prints a one-line JSON result.
- `Disconnect-SecondWindDisk.ps1` — flushes every volume on the session's
  disk, then closes the iSCSI session. Safe when already disconnected.

All parameters come from the node's mTLS-protected disk API and per-node
config; nothing machine-specific is hardcoded. Development override for the
script location: `SECONDWIND_SCRIPTS_DIR`.
