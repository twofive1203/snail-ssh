# Contributing

[中文](#中文) · [English](#english)

## 中文

功能行为以 [README.md](README.md) 为准。根目录 `AGENTS.md` 是给维护者的约定，不是产品说明。

```bash
pnpm install
pnpm build
cd src-tauri && cargo check
```

不要修改 `prototype/`。它是不参与构建的静态原型。如果根目录出现 `app.js` 或 `style.css`，那是遗留副本，不是应用代码。

CI 只跑 `pnpm build` 和 `cargo check`。通过不代表安装包已经打出，也不代表 Windows、macOS、Linux 三端发布已测过。推送 `v*` 标签会触发 Release 工作流，为 Windows、macOS（Intel 与 Apple 芯片）和 Linux 打安装包，并挂到该标签的草稿 Release 上。安装包没有代码签名，版本号仍来自 `src-tauri/tauri.conf.json`。

不要提交密码、私钥，或 `~/.snail-ssh/` 里的真实主机数据。

## English

[README.md](README.md) is the detailed behavior reference. [README.en.md](README.en.md) is the trust summary. `AGENTS.md` is maintainer guidance, not the product spec.

```bash
pnpm install
pnpm build
cd src-tauri && cargo check
```

Do not edit `prototype/`. It is a static prototype and is not part of the build. A root `app.js` or `style.css`, if one appears, is a leftover copy, not the app.

CI runs `pnpm build` and `cargo check` only. A green check is not a signed installer and not a three-platform release test. Pushing a `v*` tag runs the Release workflow, which builds Windows, macOS (Intel and Apple silicon), and Linux installers and attaches them to a draft Release for that tag. The packages are not code-signed, and the version still comes from `src-tauri/tauri.conf.json`.

Do not commit passwords, private keys, or real host data from `~/.snail-ssh/`.
