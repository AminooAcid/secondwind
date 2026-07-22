# Third-party binaries bundled by the host installer

Populate `staging\` before building `secondwind.iss`. Every entry must be
pinned (version + SHA-256) in `docs/UPSTREAM.md` at release time; licenses
ship inside each staging folder.

| Staging path | Contents | Upstream |
|---|---|---|
| `staging\secondwind-companion.exe` | release build of `companion` (`cargo tauri build`) | first-party |
| `staging\apollo\apollo-installer.exe` | Apollo Windows installer (GPL-3.0) | github.com/ClassicOldSong/Apollo |
| `staging\usbip\` | usbip-win2 client + drivers + `classic_setup.ps1` (GPL-2.0) | github.com/vadimgrn/usbip-win2 |
| `staging\xpra\` | xpra Windows client, minimal (GPL-2.0+) | xpra.org |

Rules:

- Upstream binaries are invoked as separate processes only — never linked.
- Do not rehost modified upstream binaries; bundle official artifacts.
- The installer requires elevation once; the companion itself never does
  (except the first-time share-folder setup, which triggers UAC on demand).
