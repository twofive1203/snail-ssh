# SnailSSH 项目约定（给 AI 助手）

事实基线：Tauri v2 + Rust 后端 + Vue 3 / TypeScript / TailwindCSS。下列内容与代码一致，改动前请先核对，不要凭印象推断。

## 1. 入口与"死代码"红线

- 前端唯一入口：`index.html` → `src/main.ts` → `src/App.vue`。
- 全局样式唯一生效文件：`src/style.css`（`src/main.ts` 里 `import './style.css'` 相对解析到它）。`tailwind.config.js` 的 `content` 只扫描 `./index.html` 与 `./src/**/*`。
- `prototype/`（`index.html` / `style.css` / `app.js` / `logo.svg`）是第一版纯静态原型，**不参与构建，禁止在此改业务逻辑**。
- 第一版原型遗留的根目录 `app.js` / `style.css` 已归档到本地 `backups/legacy-prototype/`；**若在根目录再见到这两个文件，不要编辑**——看到 1300 行以上的 `app.js` 就是遗留副本，不是应用代码。

## 2. 新增后端能力的三步约定

1. Rust 侧：耗时/阻塞命令必须包在 `run_blocking(...)`（`src-tauri/src/lib.rs`）里；传输类命令走 `run_transfer(...)`；不要在 IPC 线程内直接做阻塞 IO。
2. 注册：新命令必须加进 `lib.rs` 的 `tauri::generate_handler![...]` 列表，否则前端 `invoke` 会失败。
3. 前端桥接：所有调用统一加到 `src/api/tauri.ts`，并保留 `isTauri()` 降级分支（浏览器预览不能崩）；TS 类型统一放 `src/types/index.ts`。

命名沿用现状：Rust 命令 snake_case，前端调用参数 camelCase（由 Tauri 自动转换）。

## 3. 数据落盘位置

- `~/.snail-ssh/hosts.json`：主机元数据（凭证走系统钥匙串，`storage.rs` 分片存储，失败才回退明文并告警）。
- `~/.snail-ssh/settings.json`：应用设置（终端外观、选中即复制、右键粘贴、命令片段、目录书签、主机分组）；`settings.rs` 只做整块 JSON 的原子读写，结构由前端 `AppSettings` 约束，缺字段由前端补默认值。
- `~/.snail-ssh/history.json`：命令历史（每主机上限 500 条）。
- `~/.snail-ssh/logs/<host>/<UTC 时间戳>_<会话短 ID>.log`：会话日志（明文、不轮转）。
- `localStorage`：`snail_ssh_recent`、`snail_ssh_highlight_settings_v1`、`snail_ssh_highlight_enabled`（高亮总开关，仍在用；首次无 `highlight_settings` 时用它作初值）、`snail_ssh_import_remote_history`、`snail_ssh_session_log_auto`；浏览器预览降级时 `snail_ssh_app_settings_v1` 充当 settings.json 的替身，`snail_ssh_hosts` / `snail_ssh_history` 充当 `hosts.json` / `history.json` 的替身。旧键 `snail_ssh_select_to_copy` 只在 `settings.json` 缺 `selectToCopy` 时迁移一次。

## 4. 文档纪律

- `README.md` 是当前事实说明；改功能时同步更新「已知限制」章节。
- `docs/p0-development-plan.md` 是**已完成项目的存档**（8/8 完成），其中「现状 / 行号 / 未实现」全部过期，**禁止按它施工或引用其结论**。
- 不要写没有实测依据的量化指标（例如安装包体积、内存占用）。安装包实测：Windows msi 约 4.6MB、NSIS setup 约 3.1MB。

## 5. 验证

```bash
pnpm build                      # vue-tsc --noEmit + vite build
cd src-tauri && cargo check      # Rust 侧；history.rs 有单测可跑 cargo test
```

改了既有文件前，按项目习惯先备份到 `backups/<任务名>/`（`backups/` 已在 .gitignore 中）。未执行的验证需如实说明"已完成静态检查，未执行运行验证"。

## 6. 已从占位改为可用的连通功能

跳板机/代理、SSH 密钥保险箱、端口转发隧道、OpenSSH config 导入导出已经接入。不要再把这些入口当成只弹提示的占位。

限制仍在：不执行 `ProxyCommand`，不导入 FinalShell / Termius / Xshell 专有格式，动态转发无认证。细节见 `README.md` 的已知限制。

（命令片段库、自定义分组、设置中心、终端主题/字体定制已实现，见 `SettingsModal.vue` 与 `composables/useAppSettings.ts`。）
