# Security Policy

中文：漏洞请用 GitHub Private Vulnerability Reporting 私下报告，不要开公开 issue。下面是同一份说明。

## Reporting

Report vulnerabilities through GitHub Private Vulnerability Reporting on this repository. Do not open a public issue for a security report.

There is no security email and no bug bounty. If private reporting is not enabled yet, contact a repository admin privately instead of filing a public issue.

## Scope

The desktop application in this repository: SSH connection and authentication, host-key checking, credential storage, port forwarding, SFTP, and session logs.

## What this app does

- Host metadata is stored in `~/.snail-ssh/hosts.json`. Passwords, private keys, and passphrases are stored in the OS keychain when it works (Windows Credential Manager, macOS Keychain, Linux Secret Service). A private key that exceeds one keychain item is split across entries.
- If the keychain cannot be used, secrets fall back to plaintext in `hosts.json` and a warning is written to the log. A missing keychain means the default protection is gone. The app then limits `~/.snail-ssh` to the current user (Unix `0700`/`0600`; Windows via `icacls` when it succeeds). That does not replace the keychain.
- Host keys are checked against `~/.ssh/known_hosts`, including OpenSSH hashed `|1|` lines. The first connection shows a SHA256 fingerprint for you to confirm, then writes it. A key that does not match the stored record blocks the connection. Confirming a replacement removes only plain host-key lines for that hop, not `@cert-authority` or `@revoked` lines. Confirmation trusts the current hop only, not the rest of a jump chain you have not inspected.
- A host certificate is accepted only when its signature verifies, the signing key is a trusted `@cert-authority` for that host, and the certificate type, validity, and principals match. Certificates with critical options this client does not enforce (including `source-address`) are rejected. A wildcard without a port matches port 22 only.
- Imported `ProxyCommand` values are ignored. The app does not run them as local commands.
- Dynamic (SOCKS) forwarding does not authenticate clients. The app refuses to bind it to anything except loopback (`127.0.0.1`, `localhost`, or `::1`).
- Session logs in `~/.snail-ssh/logs/` are plaintext. A single file rolls at 50MB and files older than 7 days are removed on startup. Redaction of passwords, tokens, and private-key blocks is heuristic and can miss secrets split across output chunks. Log text can differ from what the terminal showed.
- Before authentication the client tries the SSH `none` method, matching OpenSSH. Some gateways accept a username with no credential.

## Out of scope

- Vulnerabilities in the operating system, sshd, or a third-party proxy you configure.
- Secrets pasted into a terminal or saved in a snippet.
