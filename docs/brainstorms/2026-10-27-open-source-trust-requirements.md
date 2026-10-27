---
date: 2026-10-27
topic: open-source-trust
---

# 开源信任材料

## Problem Frame

SnailSSH 已经能作为本地 SSH / SFTP 客户端使用，但仓库还不能被陌生人信任地公开：没有许可证、没有英文入口、没有漏洞报告方式和可复现的检查。这次只补这些信任材料，不改产品功能。

---

## Requirements

**许可证**
- R1. 仓库根目录提供 MIT `LICENSE`，版权持有人写 `SnailSSH contributors`，年份 2026。
- R2. `package.json` 与 `src-tauri/Cargo.toml` 标明 `MIT`。`package.json` 保持 `"private": true`，避免桌面应用被误发到 npm。

**中英说明**
- R3. `README.md` 保持现有中文详细说明，只在文首增加英文入口、许可证与安全说明链接，不删改功能描述和已知限制。
- R4. 新增 `README.en.md`，让不读中文的人能判断：这是什么、技术栈、目标平台、如何构建、安全模型、影响信任的已知限制、许可证，以及去哪里报告漏洞。
- R5. 英文说明只复述仓库里已有事实。不翻译成与中文不一致的功能清单，不新增未实测指标，不把「目标支持 Windows / macOS / Linux」写成「已在三端实测通过」。
- R6. 英文说明必须明确没有这些能力：执行 `ProxyCommand`、导入 FinalShell / Termius / Xshell、自动重连、自定义快捷键、界面多语言、云同步、插件市场。

**安全说明**
- R7. 新增英文 `SECURITY.md`，文首用一行中文指向同一份说明。报告方式只用 GitHub Private Vulnerability Reporting，不编造安全邮箱，不承诺赏金。
- R8. 安全说明覆盖已实现的模型：凭证进系统钥匙串、钥匙串失败时回退明文并告警、`known_hosts` 校验、动态转发无认证且应只绑定本机、会话日志是明文且不轮转、`ProxyCommand` 不会在本机执行。

**贡献与检查**
- R9. 新增简短 `CONTRIBUTING.md`，说明如何安装依赖、跑前端构建与 `cargo check`，并写明不要改 `prototype/` 里的原型，也不要把根目录遗留的 `app.js` / `style.css` 当成应用代码。
- R10. 增加 GitHub Actions：对 `master` 的 push 和 pull request 运行 `pnpm build` 与 `cargo check`。检查不需要密钥，失败时工作流失败。
- R11. CI 只证明前端类型检查/构建和 Rust 检查能通过。文档和状态里不得把它写成完整桌面安装包或三端发布已通过。

---

## Success Criteria

- 陌生人打开仓库后，不用读中文也能知道许可证、安全边界、构建方式和不能做什么。
- 公开仓库前不需要再发明许可证、漏洞报告渠道或「CI 到底测了什么」。
- 实现者完成时，功能代码和行为保持不变；只新增或最小改动信任材料。

---

## Scope Boundaries

- 不改 SSH、SFTP、终端、设置或主机管理功能。
- 不做界面 i18n，不把 Vue 文案抽成语言包。
- 不把整份中文 README 逐条译成英文。
- 不新增行为准则、Issue 模板、发布工作流、代码签名或自动打包。
- 不引入账号、云同步或安全赏金。
- 不修改 `docs/p0-development-plan.md`。

---

## Key Decisions

- 许可证用 MIT：便于使用和再分发，与 Tabby、Electerm 同类开源客户端一致。
- 版权行用 `SnailSSH contributors`，不绑定个人真名。
- 中文 README 仍是详细事实来源；英文是判断能否采用的入口，不是第二份完整手册。
- 保持 `private: true`：开源指公开源码，不是发布到 npm。

---

## Dependencies / Assumptions

- 安全模型与已知限制以当前 `README.md` 为准；实现时若与代码不一致，以代码为准并只修正说明，不顺手改功能。
- 默认分支是 `master`。
- 本机已能运行 `pnpm build` 与 `cargo check` 的假设只用于写检查，不代表这次已经跑过。

---

## Outstanding Questions

### Deferred to Planning

- [Affects R10][Technical] CI 跑在 `windows-latest` 还是 `ubuntu-latest`。选择标准是能稳定完成 `pnpm build` 与 `cargo check`，且不要求密钥。

---

## Next Steps

-> `/ce-plan` 做实现计划；范围已足够小时也可以直接 `/ce-work`。
