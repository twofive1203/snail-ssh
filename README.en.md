# SnailSSH

[中文](README.md) · [License](LICENSE) · [Security](SECURITY.md) · [Contributing](CONTRIBUTING.md)

Local-first SSH and SFTP desktop client. It uses Tauri v2, a Rust SSH stack (`russh`), and a Vue 3 interface. Windows, macOS, and Linux are build targets. Installer sizes below were measured for Windows packages only; they are not a claim that macOS or Linux releases were tested, and they are not a promise for future builds.

## Stack

| Layer | Choice |
| :--- | :--- |
| Desktop | Tauri v2 |
| SSH / SFTP | Rust (`russh`, `russh-sftp`) |
| UI | Vue 3, TypeScript, Tailwind CSS |
| Terminal | xterm.js |

Windows installers have been measured at about 4.6 MB (msi) and 3.1 MB (NSIS setup).

## What it does

- Host inventory with groups, tags, search, jump-host chains of saved hosts (up to 8 hops), SOCKS5 / HTTP proxies, and local, remote, and dynamic port forwarding.
- OpenSSH `ssh_config` import and export. Export does not write passwords or private key material. `Match` and `Include` are not expanded.
- A key vault for generating or importing ed25519 and RSA keys, plus a view of `known_hosts`.
- An interactive terminal with a real PTY, snippets, command history, keyword highlighting, and optional session logs.
- An SFTP browser with inline editing, drag-and-drop upload, and resumable transfers.
- Passwords and private keys go to the OS keychain when it is available.

`README.md` is the detailed Chinese feature list. This page is the adoption and trust summary, not a second full manual.

## Security model

Details and how to report a vulnerability are in [SECURITY.md](SECURITY.md).

- Passwords, private keys, and passphrases are stored in the OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service). If the keychain cannot be used, they fall back to plaintext in `~/.snail-ssh/hosts.json` and a warning is logged.
- Host keys are checked against `~/.ssh/known_hosts`. The first connection asks you to confirm the SHA256 fingerprint. A mismatch blocks the connection. Confirming a key trusts that hop only.
- Dynamic forwarding has no authentication. Bind it to `127.0.0.1` only.
- Session logs under `~/.snail-ssh/logs/` are plaintext, are not rotated, and use heuristic redaction. Do not treat them as a complete secret filter.
- `ProxyCommand` from imported OpenSSH config is ignored. This app does not execute it on your machine.

## Build

You need Node.js, pnpm, and a Rust toolchain. Desktop mode also needs the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
pnpm install
pnpm dev          # browser preview; it does not open a real SSH session
pnpm tauri dev    # desktop app
pnpm build        # vue-tsc + vite build
cd src-tauri && cargo check
```

`pnpm build` and `cargo check` do not produce or sign an installer. CI runs only those two checks.

## Not included

- Executing OpenSSH `ProxyCommand`
- Importing FinalShell, Termius, or Xshell private formats
- Automatic reconnect (reconnect is manual)
- Customizable shortcuts (the shortcut page is a read-only cheat sheet)
- UI translations (interface strings are Chinese)
- Cloud sync or accounts
- A plugin marketplace
- RDP, VNC, serial, Telnet, or a built-in X11 server

## License

[MIT](LICENSE). Copyright 2026 SnailSSH contributors.
