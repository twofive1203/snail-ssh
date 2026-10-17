# SnailSSH P0 开发计划（8 项）【已完成 · 仅存档】

> ⚠️ **状态：8 项全部完成，本文件只作历史记录，禁止再按此施工。**
> 文中所有「现状 / 未实现 / 改动文件 / 行号」均停留在 `main` @ `8930a08`，与当前代码已严重不符
> （例如：`sftp.rs` 的 rename/chmod/chown/touch、`transfer.rs`、`useRecentHosts.ts`、终端搜索与
> Unicode11 addon、FileTree 的 new-file/rename 菜单均**已实现**）。
> 需要当前事实请以 `README.md` 与代码为准。

| 项 | 内容 | 完成提交 |
|---|---|---|
| 3 | 终端内搜索 + 剪贴板 | `ec9e19c` |
| 7 | 快速连接 + 最近连接 | `e6bb7b5` |
| 8 | 关键字高亮规则 | `ec9e19c` → `56353c9` → `50e9cd5` |
| 5 | SFTP 操作补齐 | `ca4b192` |
| 2 | 命令历史 + 联想补全 | `cc963bc` |
| 4 | 会话日志 | `cc963bc` |
| 6 | 拖拽上传 | `cc963bc` |
| 1 | 传输暂停/取消/重试/续传 | `8466928` |

---

## 0. 【已作废】原「给新会话的开场提示词」

> 🚫 本节原有提示词会诱导新会话把整套 P0 重做一遍，已于 2026-10-17 移除。
> 后续若有同类施工计划，请在完工后立即在文件头标注完成状态并删除开工指令，避免被当作待办执行。

---

## 1. 总览

| # | 项目 | 类型 | 涉及层 | 工作量 | 依赖 |
|---|---|---|---|---|---|
| 3 | 终端内搜索 + 剪贴板体验 | 纯前端 | 前端 | S（0.5~1d） | 无 |
| 7 | 快速连接 + 最近连接 | 前端为主 | 前端 | S（0.5~1d） | 无 |
| 8 | 关键字高亮规则 | 前端 | 前端 | M（1~2d） | 无 |
| 5 | SFTP 操作补齐（重命名/chmod/chown/新建文件/路径直输/过滤） | 前后端 | 全栈 | M（1.5~2.5d） | 无 |
| 2 | 命令历史 + 联想补全 | 前后端 | 全栈 | M（1.5~2.5d） | 建议先做 3（共用终端交互层） |
| 4 | 会话日志（自动落盘 + 脱敏） | 前后端 | 全栈 | M（1.5~2d） | 无 |
| 6 | 拖拽上传（含目录） | 前后端 | 全栈 | M~L（2~3d） | 建议在 1 之后做（复用传输队列） |
| 1 | 传输暂停/取消/重试/断点续传 | 前后端 | 全栈 | L（2.5~4d） | 建议最后做（改动面最大，收益最直接） |

**推荐执行顺序：3 → 7 → 8 → 5 → 2 → 4 → 6 → 1**

理由：前 4 项几乎零架构风险、当天可见效果；中间 3 项引入新的后端命令但互不干扰；第 1 项会改动 `sftp.rs` 全部传输函数，放最后做可以复用前面沉淀的事件与状态模型。

**统一约定**
- 所有新后端命令必须走现有的 `run_blocking(...)` 包装（见 `src-tauri/src/lib.rs:18`），不要在 IPC 线程里做阻塞 IO。
- 所有新命令都要在 `lib.rs` 的 `tauri::generate_handler![...]`（`src-tauri/src/lib.rs:296` 附近）里注册。
- 前端所有新桥接函数统一加到 `src/api/tauri.ts`，并保留 `isTauri()` 降级分支（浏览器预览模式不能被弄崩）。
- 新增 TS 类型统一放 `src/types/index.ts`。
- 新命令命名沿用现有风格：snake_case，参数名在前端调用时用 camelCase（Tauri 会自动转换）。

---

## 2. 第 3 项：终端内搜索 + 剪贴板体验

### 目标 / 验收
- `Ctrl+F` 在终端内呼出搜索条，可输入关键字、上下跳转、显示命中数量。
- 支持：区分大小写、全字匹配、正则开关。
- `Ctrl+Shift+C` 复制选中、`Ctrl+Shift+V` 粘贴、`Shift+Insert` 粘贴、`Ctrl+Insert` 复制。
- 终端内右键弹出应用内菜单（复用 `ContextMenu.vue`）：复制 / 粘贴 / 全选 / 查找 / 清除屏幕。
- 关闭搜索条后 `Esc` 回到终端焦点，终端不被抢占（搜索输入框聚焦时不能把按键写进远端）。

### 竞品依据
xterm.js 生态标准做法；Tabby / WindTerm / iTerm2 / Xshell 全部具备终端内查找。

### 现状
- `src/components/TerminalView.vue:132-134` 只加载了 `@xterm/addon-fit` 和 `@xterm/addon-web-links`。
- 每个终端实例在 `createSession()`（`TerminalView.vue:222`）里创建，键盘事件目前完全交给 xterm 默认行为。
- 全局快捷键在 `src/App.vue:69x`（`setupGlobalKeybindings`）注册，并且有 `isTerminalOrEditorTarget()` 白名单会把 `Ctrl+K` 之外的组合放行 —— 注意别和这里冲突。

### 改动文件
- `package.json`（新增 addon 依赖）
- `src/components/TerminalView.vue`
- `src/components/ContextMenu.vue`（复用，无需改）
- `src/App.vue`（如需把 `Ctrl+F` 纳入全局键盘白名单判断）

### 实现方案
1. 加依赖：
   ```
   pnpm add @xterm/addon-search @xterm/addon-unicode11
   ```
   - `@xterm/addon-search` 版本需与 `@xterm/xterm@5.5.0` 同代（addon-search ≥ 0.15）。安装后先跑一次 `pnpm build` 确认类型无误。
   - `addon-unicode11` 顺手解决中日韩宽字符宽度误判（中文环境收益明显）。
   - **不建议**现在上 `addon-webgl`：Windows 上有偶发渲染问题，留到性能优化阶段单独评估。
2. `TerminalSession` 接口（`TerminalView.vue:182`）加字段 `search: SearchAddon`；在 `createSession()` 里 `term.loadAddon(search)` 并 `term.unicode.activeVersion = '11'`。
3. 新增搜索条 UI：贴在 Tab Bar 下方的一条覆盖式浮动栏（`absolute top-10 right-4 z-30`），包含：
   输入框（自动 focus）、命中计数（`onDidChangeResults`）、上一个/下一个按钮、`Aa` / `ab` / `.*` 三个开关、关闭按钮。
4. 键盘：
   - 在 `TerminalView` 挂全局 keydown 监听（仅当该实例是当前活动标签）处理 `Ctrl+F`、`Esc`。
   - 用 `term.attachCustomKeyEventHandler(...)` 接管 `Ctrl+Shift+C/V`、`Ctrl+Insert`、`Shift+Insert`，返回 `false` 阻止 xterm 继续处理。
   - 复制实现：`navigator.clipboard.writeText(term.getSelection())`，无选区时提示。
   - 粘贴实现：优先 `navigator.clipboard.readText()`，失败时回退 `writeSshSessionApi(tab.id, text)` 直接发送。
5. 右键菜单：在终端容器上加 `@contextmenu.prevent`，把当前标签也纳入 `tabMenu` 或新开一个 `termMenu` ref；菜单项调用 `term.getSelection()` / `term.selectAll()` / `term.clear()`。
6. `clearCurrentTerminal()` 当前发 `\x0c` 给远端；搜索条要"清除屏幕高亮"时用 `search.clearDecorations()`，不要混用。

### 边界与坑
- `Ctrl+L` 已被远端清屏占用（`TerminalView.vue:76` 的提示），不要在本地抢这个键。
- xterm 默认把 `Ctrl+Shift+C` 当作复制，但对 `Shift+Insert`/`Ctrl+Insert` 未必处理 —— 用 `attachCustomKeyEventHandler` 兜住。
- 搜索条聚焦时，`term.onData` 不应收到字符：搜索条是独立的 DOM 输入框，xterm 只在自身 textarea 聚焦时触发 `onData`，所以安全；但要确保打开搜索条时 `term.blur()`、关闭时 `term.focus()`。
- 浏览器预览模式（非 Tauri）同样要能搜索，不要只在 Tauri 分支实现。

### 验证步骤
1. `pnpm build` 类型检查通过。
2. `pnpm dev` 浏览器预览：终端打印多行内容 → `Ctrl+F` 搜索 → 上下跳转、开关正则 → `Esc` 关闭。
3. `pnpm tauri dev`：真实连一台机器，`ls -la` 后搜索文件名；选中文字 `Ctrl+Shift+C`，再 `Ctrl+Shift+V` 粘贴到终端能执行。

---

## 3. 第 7 项：快速连接 + 最近连接

### 目标 / 验收
- 存在一个"快速连接"入口（不写入 `~/.snail-ssh/hosts.json`），填 host/port/user/认证后直接开会话。
- `Ctrl+Shift+N` 呼出快速连接；`Ctrl+N` 保持"新建服务器"语义不变。
- 侧边栏"最近"区域、命令面板空输入时都展示真实的最近连接（按最近使用时间倒序，最多 5 条）。
- 点击最近连接可直接连上（主机仍在库中时）。

### 竞品依据
Termius / Xshell / MobaXterm / WindTerm 全部有 Quick Connect + 最近连接列表。

### 现状
- `src/components/HostModal.vue` 已是完整表单（含认证方式、连通性测试 `testSshConnectionApi`），只需加"临时会话"模式。
- `src/App.vue:33` 侧边栏最近连接是硬编码：`:recent-hosts="hosts.slice(0, 2)"` —— 这是假的，要替换。
- `src/App.vue:56` HostModal 只接收 `:host-data`，`@save` 里无条件走 `saveHostApi`。
- `src/components/CommandPalette.vue` 已接收 `hosts`，只需加"最近"分组。

### 改动文件
- `src/components/HostModal.vue`
- `src/components/CommandPalette.vue`
- `src/App.vue`
- `src/components/Sidebar.vue`（如需调整最近区域的渲染）
- `src/composables/useRecentHosts.ts`（新建）

### 实现方案
1. 新建 `src/composables/useRecentHosts.ts`：
   ```ts
   // 结构：[{ hostId, name, host, port, username, lastUsedAt }]
   // 存储：localStorage key = 'snail_ssh_recent'，最多 20 条，读取时取前 N 条
   export function useRecentHosts() {
     return { recents, touchRecent, removeRecent, clearRecents }
   }
   ```
   本项**先用 localStorage 做 MVP**，不动后端。理由：最近连接是纯 UI 辅助信息，丢失可接受；等做"设置中心/配置中心"时再统一挪到 `~/.snail-ssh/`。
2. `HostModal.vue` 加 `mode?: 'edit' | 'new' | 'quick'`：
   - `quick` 模式下标题改为"快速连接"、隐藏分组/标签字段、主按钮文案改为"连接"。
   - 提交时 `emit('connect-quick', host)`，**不调用 `saveHostApi`**。
3. `App.vue` 加 `quickConnectOpen` ref + `editingHost = null` 复用同一个弹窗；新增 `handleQuickConnect(host)`：
   - 构造临时 HostItem（`id = 'quick-' + Date.now()`）→ 直接调 `openSession(host)` → 走现有会话创建逻辑。
   - 由于 `handleConnectSsh` 会按 `hostId` 去重，临时主机不会被误判为已存在。
4. 在 `handleConnectSsh` 成功创建会话后调用 `touchRecent(host)`；`handleCloseSession` 时保留 recent 记录。
5. `App.vue:33` 的 `:recent-hosts` 改为 `recents`（来自 composable），Sidebar 用真实数据渲染。
6. `CommandPalette.vue`：当输入为空时渲染"最近连接"分组，选中即 `emit('select-host', host)`（找不到对应 host 对象则跳过并提示"主机已删除"）。
7. 快捷键：`App.vue` 的 `setupGlobalKeybindings()` 里加 `Ctrl+Shift+N`。

### 边界与坑
- `isTerminalOrEditorTarget()` 白名单会让终端聚焦时的全局快捷键失效 —— 这是现有设计，快速连接键也遵循该规则（避免用户在 vim 里按键被抢），不要改。
- 临时会话的 host 对象没有走钥匙串，密码只存在于内存 —— 明确写进 UI 提示（"临时会话不会保存到资产列表"）。
- 最近连接里若主机已被删除，条目要过滤掉（用 `hosts.find()` 校验）。

### 验证步骤
1. `pnpm build` 通过。
2. 浏览器预览：快速连接弹窗可打开、可提交（mock 链路）；最近连接列表随连接动作更新、刷新页面后仍在。
3. `pnpm tauri dev`：快速连接真实主机成功；确认 `~/.snail-ssh/hosts.json` **没有**新增记录。

---

## 4. 第 8 项：关键字高亮规则

### 目标 / 验收
- 终端输出中的 `ERROR` / `FATAL` / `WARN` / 失败类关键词自动着色。
- 时间戳、IPv4、`file:line` 弱化或强调（可配置）。
- 高亮不破坏 ANSI 色彩，不导致滚动卡顿（10 万行日志仍可用）。
- 提供开关（终端顶栏一个灯泡图标即可），默认开。

### 竞品依据
WindTerm 最被称道的卖点之一就是关键字着色；FinalShell 也有错误高亮。

### 现状
- `TerminalView.vue` 里 stdout 是直接 `session.term.write(chunk)`（`bindSessionEvents`，`TerminalView.vue:31x`）。
- `allowProposedApi: true` 已在 `createSession()` 设置 —— xterm 的 marker / decoration API 可直接用。
- 已有 `WebLinksAddon` 处理 URL 点击。

### 改动文件
- `src/composables/useTerminalHighlight.ts`（新建）
- `src/components/TerminalView.vue`
- `src/types/index.ts`（如新增规则类型）

### 实现方案（**不要**改 chunk 文本）
在写入前对 chunk 做字符串替换去插 ANSI 转义是错误方案：chunk 边界可能把一条 ANSI 序列或多字节字符切断，会产生乱码/错色。采用 xterm 官方的 marker + decoration 后置扫描方案：

1. `useTerminalHighlight(term: Xterm)` 内部：
   - 维护 `lastScannedAbsoluteLine: number`。
   - 在 `term.onWriteParsed(...)` 回调里（若该 API 在当前 5.5 版本不可用，退化为包装 write：`const origWrite = term.write.bind(term)`，写入后 `queueMicrotask(scan)`）触发一次节流扫描（`requestAnimationFrame` 或 80ms 节流）。
   - 扫描范围只取 `[lastScannedAbsoluteLine, term.buffer.active.baseY + term.buffer.active.cursorY]`，每次最多扫 `MAX_SCAN_LINES_PER_TICK = 200` 行，剩余留给下一 tick，避免大日志一次性卡住 UI。
   - 用 `term.buffer.active.getLine(i)?.translateToString(true)` 取纯文本（已剥离 ANSI 控制码）。
   - 命中规则后：`const marker = term.registerMarker(i - (term.buffer.active.baseY + term.buffer.active.cursorY))`，再 `term.registerDecoration({ marker, backgroundColor, layer: 'top' })`。
   - 删行/缓冲区回收时要 `decoration.dispose()`（监听 marker 的 `onDispose`）。
2. 内置规则集（第一条实现即可，规则抽成常量数组便于后续做成设置项）：
   | 规则 | 正则（示意） | 样式 |
   |---|---|---|
   | 错误 | `\b(ERROR|FATAL|FAIL(ED|URE)?|PANIC|Exception)\b` | 红底/红字 |
   | 警告 | `\b(WARN(ING)?|DEPRECAT(ED|ION))\b` | 琥珀字 |
   | 成功 | `\b(OK|SUCCESS|PASSED|Done)\b` | 绿字 |
   | IPv4 | `\b\d{1,3}(\.\d{1,3}){3}\b` | 弱化 + 可点击复制 |
   | 文件行号 | `[\w./-]+\.\w+:\d+(:\d+)?` | 青色 + 可点击 |
3. IP / `file:line` 可点击：用 `term.registerLinkProvider(...)`（xterm 原生 API）注册一个 provider，返回 `{ range, text, activate }`；`activate` 里执行复制到剪贴板 + Toast。**不要去改 `WebLinksAddon`。**
4. 与第 3 项的搜索高亮冲突：搜索装饰用默认层，本项统一用 `layer: 'top'`；搜索结束后 `clearDecorations()` 不应清掉关键字高亮（两者都用 decoration，测试时注意）。若确实互相干扰，把关键字高亮改为"只设置文字色"而非背景色。
5. 开关状态存 `localStorage`（`snail_ssh_highlight_enabled`），顶栏加一个 `Highlighter` 图标切换按钮；关闭时 dispose 所有说明符并停止扫描。

### 边界与坑
- **性能红线**：必须做"只扫新增行 + 每 tick 上限 + 节流"。直接 `onWriteParsed` 全量扫 buffer 会在 `tail -f` 大日志时卡死。
- marker 是相对位置，不要用绝对行号创建 decoration（滚动后错位）。
- 高亮行被滚出 scrollback 时 marker 会自动 dispose，注意不要重复创建同一行（用 `lastScannedAbsoluteLine` + 已标记行集合去重）。
- 不要高亮提示符行本身（当前输入行），否则用户打字时闪烁。跳过 `baseY + cursorY` 所在行。

### 验证步骤
1. `pnpm build` 通过。
2. `pnpm tauri dev` 连真实主机：
   - `journalctl -n 200` / `cat /var/log/syslog` 观察高亮与滚动流畅度。
   - `for i in $(seq 1 200000); do echo "ERROR line $i"; done` 压测，UI 不应卡死（CPU 可接受，滚动仍顺）。
   - 关闭开关后高亮消失。

---

## 5. 第 5 项：SFTP 操作补齐

### 目标 / 验收
- 重命名（文件+目录）、修改权限（八进制输入）、修改属主/属组、新建空白文件。
- 路径直输跳转：点击/输入框粘贴绝对路径回车即可跳转（列表视图和树视图都要）。
- 当前目录按名称过滤（前端过滤已加载列表即可）。
- 属性查看（名称/路径/大小/权限/属主/时间）。
- **修掉两个死菜单**：`FileTree.vue` 的"新建空白文件"和"重命名"菜单项目前点了没反应。

### 竞品依据
FileZilla / WinSCP / Termius 的最低门槛功能；WinSCP 的属性对话框是标配。

### 现状（重要）
- `src/components/FileTree.vue:423` 有 `new-file` 菜单项，`:447` 有 `rename` 菜单项，但 `handleContextMenuSelect()`（`FileTree.vue:662-729`）的 switch **没有这两个 case**，是死菜单。
- `src/components/SftpView.vue`（列表视图）**完全没有右键菜单**，只有行内三个小按钮（编辑/下载/删除）。
- 后端 `src-tauri/src/sftp.rs` 已有：`read_dir` / `read_file` / `write_file` / `mkdir` / `remove_file`(递归) / `home_dir` / `upload_file` / `download_file` / `download_dir`。**没有** rename / chmod / chown / touch。
- `SftpFileItem` 里 `owner` / `group` 目前是**数字 uid/gid 的字符串**（`sftp.rs:157-158`），不是用户名 —— 属性编辑要按数字处理，UI 上要说明。

### 改动文件
- `src-tauri/src/sftp.rs`（新增 4~5 个函数）
- `src-tauri/src/lib.rs`（新增 4~5 个 command + 注册）
- `src/api/tauri.ts`
- `src/components/FileTree.vue`
- `src/components/SftpView.vue`
- `src/components/ContextMenu.vue`（复用）
- `src/types/index.ts`（可选的属性类型）

### 新增后端函数（签名）

```rust
// src-tauri/src/sftp.rs

/// 重命名 / 移动。overwrite=true 时覆盖同名目标。
pub fn sftp_rename(
    session_arc: Arc<Mutex<ssh2::Session>>,
    from: &str,
    to: &str,
    overwrite: bool,
) -> Result<(), String>;
// 实现：sftp.rename(Path::new(from), Path::new(to), if overwrite { RenameFlags::OVERWRITE } else { RenameFlags::empty() })
//       Err(_) 时拼一层中文错误信息："重命名失败: ..."

/// 修改权限位（八进制，例如 0o755）。仅当文件存在时生效。
pub fn sftp_chmod(
    session_arc: Arc<Mutex<ssh2::Session>>,
    path: &str,
    mode: u32,
) -> Result<(), String>;
// 实现：sftp.set_metadata(Path::new(path), &ssh2::FileStat { perm: Some(mode), ..Default::default() })
//       注意 mode 只取低 12 位（含 setuid/setgid/sticky），前端做校验。

/// 修改属主 / 属组（数字 id，None 表示不改该项）
pub fn sftp_chown(
    session_arc: Arc<Mutex<ssh2::Session>>,
    path: &str,
    uid: Option<u32>,
    gid: Option<u32>,
) -> Result<(), String>;
// 实现：同 set_metadata，填 uid / gid 字段。非 root 会返回权限错误，需要透传远端原始报错。

/// 新建空白文件（已存在则直接报错，不要静默覆盖）
pub fn sftp_touch(
    session_arc: Arc<Mutex<ssh2::Session>>,
    path: &str,
) -> Result<(), String>;
// 实现：先 sftp.lstat(path) 成功 => Err("目标已存在")；失败 => sftp.create(path) 后立即 drop。
//       必须 drop 文件句柄，否则某些老 sshd 上文件句柄会泄漏到会话结束。

/// 单条目属性（供属性弹窗与编辑器冲突检测复用；可选实现）
pub fn sftp_stat(
    session_arc: Arc<Mutex<ssh2::Session>>,
    path: &str,
) -> Result<SftpFileItem, String>;
// 复用 sftp_read_dir 里构造 SftpFileItem 的那段逻辑，抽成 fn build_file_item(name, &stat) -> SftpFileItem
```

`lib.rs` 里照抄现有 `sftp_mkdir` / `sftp_remove_file` 的骨架（`State<'_, SshManager>` + `run_blocking` + `manager.get_sftp_session(&session_id)?`），新增：
`sftp_rename` / `sftp_chmod` / `sftp_chown` / `sftp_touch` / `sftp_stat`，并全部注册进 `generate_handler!`。

### 前端改动

`src/api/tauri.ts` 新增：
```ts
export async function sftpRenameApi(sessionId: string, from: string, to: string, overwrite = false): Promise<void>
export async function sftpChmodApi(sessionId: string, path: string, mode: number): Promise<void>
export async function sftpChownApi(sessionId: string, path: string, uid?: number, gid?: number): Promise<void>
export async function sftpTouchApi(sessionId: string, path: string): Promise<void>
export async function sftpStatApi(sessionId: string, path: string): Promise<SftpFileItem>
```
每个都按现有模式写：`if (isTauri()) { await invoke(...) }`，非 Tauri 环境静默返回（或抛明确错误）。

`FileTree.vue`：
- 在 `handleContextMenuSelect` 的 switch 里补 `new-file` → 用 `promptDialog` 输入文件名 → `sftpTouchApi`；补 `rename` → `promptDialog`（默认值 `node.name`）→ `sftpRenameApi(node.path, joinPath(parentDir, newName))`。
- 菜单里新增 `chmod`（"修改权限"）、`chown`（"修改属主"）、`props`（"属性"）。
- 校验：文件名不得包含 `/`，不得为空；重命名目标已存在时先 `confirmDialog` 询问覆盖。
- `chmod`：`promptDialog` 输入三位/四位八进制，前端校验 `/^[0-7]{3,4}$/`，`parseInt(v, 8)`。
- 参考 `createNewFolder()`（`FileTree.vue:61x`）的写法保持风格一致。

`SftpView.vue`：
- 新增右键菜单（`ContextMenu.vue` + `@contextmenu.prevent`）：下载 / 编辑 / 重命名 / 权限 / 属主 / 复制路径 / 属性 / 删除。
- 工具栏面包屑右侧加"路径输入"：一个小铅笔图标切换成 `input`，回车 `navigateTo(value)`（校验以 `/` 开头，允许 `~` 自动替换为家目录）。
- 表格上方加过滤输入框 `filterText`，`visibleFiles` 追加 `.filter(f => f.name.toLowerCase().includes(filterText.toLowerCase()))`。
- 与 `selectedFileNames` 的联动：过滤后隐藏的选中项要保持选中集合不变（不要因为过滤就清空）。

### 边界与坑
- **路径拼接必须走已有的 `joinPath(dir, name)`**（`SftpView.vue:1xx` 与 `FileTree.vue` 里各有一份语义相同的实现），不要新写一套拼接逻辑；重命名时目标是"父目录 + 新名字"，注意从 `node.path` 反推父目录要用 `lastIndexOf('/')` 而不是字符串 replace。
- `SftpFileItem.owner/group` 是数字 id 字符串，属性弹窗要显示成 `uid: 1000`，不要假装是用户名。
- `..` 伪条目不能参与重命名/属性操作（现有代码用 `file.name !== '..'` 过滤，保持一致）。
- 树视图节点 (`FileTreeNode`) 和列表视图 (`SftpFileItem`) 是两套结构，属性弹窗要做一次字段映射（参考 `FileTree.vue:670-683` 已有的映射写法）。
- 操作成功后必须刷新：树视图 `refreshCurrent()`，列表视图 `loadFiles()`。

### 验证步骤
1. `cd src-tauri && cargo check` 通过；`pnpm build` 通过。
2. `pnpm tauri dev` 连真实主机（**用测试账号或 /tmp 目录验证，不要在生产目录上试 chmod/chown**）：
   - 新建空白文件 → 重命名 → 改 0644 → 改属主（非 root 时应看到远端权限报错原文）→ 删除。
   - 路径输入框粘贴 `/etc/nginx` 回车能跳转；树视图同样能跳。
   - 过滤框输入 `conf` 只显示匹配项，`选中 3 项 → 开始过滤 → 取消过滤` 后选中项仍在。

---

## 6. 第 2 项：命令历史 + 联想补全

### 目标 / 验收
- 输入时在终端底部（或光标附近）出现候选列表，来源：本主机历史命令（按使用频次 + 最近使用排序）。
- `Tab` 接受当前候选，`↑/↓` 切换候选，`Esc` 关闭；候选条不抢终端按键。
- `↑/↓` 在空输入行时翻历史命令（类似 bash），这是补全之外的独立诉求，一并做掉。
- 历史跨会话持久化到 `~/.snail-ssh/history.json`，每主机上限 500 条，自动去重并累计 `count`。
- 首次连接后可选择性拉取远端 `~/.bash_history` 作为词库（可关）。

### 竞品依据
FinalShell（命令自动提示/智能匹配）、WindTerm、Xterminal（AI 命令推荐）、Fish/Zsh 的灰字内联提示。

### 现状
- `TerminalView.vue:26x` `term.onData` 已拦截输入，并且已经有 `emit('terminal-enter', tab.id)` 在遇到 `\r` 时触发 —— 这是"命令提交"的天然钩子。
- 没有输入行缓冲（只有 mock 模式下的 `session.inputBuffer`）。
- 后端没有"执行命令并取回输出"的通用命令；`ssh.rs` 里有 `run_probe_command()`（私有，`ssh.rs:407`），可包装成公开能力。

### 改动文件
- `src-tauri/src/history.rs`（新建）
- `src-tauri/src/lib.rs`（`mod history;` + 3~4 个新 command）
- `src-tauri/src/ssh.rs`（把 `run_probe_command` 能力提升为公开的 `exec_command`）
- `src/api/tauri.ts`
- `src/components/TerminalView.vue`
- `src/components/CommandSuggestBar.vue`（新建，候选条组件）
- `src/types/index.ts`

### 后端设计

```rust
// src-tauri/src/history.rs  —— 完全模仿 storage.rs 的写法（serde + serde_json + fs + dirs::home_dir）
// 文件：~/.snail-ssh/history.json
// 结构：{ "<host_key>": [ { "cmd": String, "count": u32, "lastUsedAt": u64 } ] }
//   host_key 用 "user@host:port"（比 hostId 稳定：临时会话、重新导入资产后历史不丢）

pub fn load_history(host_key: &str) -> Result<Vec<HistoryEntry>, String>;
pub fn push_history(host_key: &str, cmd: &str) -> Result<(), String>;
pub fn clear_history(host_key: Option<&str>) -> Result<(), String>;
// 规则：忽略长度 > 4096 的命令；忽略纯空白；忽略以空格开头的命令（bash 的 HISTCONTROL=ignorespace 约定）；
//       已存在则 count += 1 并更新 lastUsedAt；超 500 条时按 (count desc, lastUsedAt desc) 裁剪，保留前 500 的集合。
```

```rust
// src-tauri/src/ssh.rs —— 把一个已存在的私有能力公开（签名风格参照 get_ssh_session_cwd）
pub fn exec_command(session_arc: &ActiveSession, command: &str) -> Result<String, String>;
// 内部就是现成的 run_probe_command(sess, command)，注意它默认在非阻塞会话上按 EAGAIN 轮询。
```

`lib.rs` 新增 command：
- `get_command_history(host_key: String) -> Vec<HistoryEntry>`
- `push_command_history(host_key: String, cmd: String) -> Result<(), String>`
- `clear_command_history(host_key: Option<String>) -> Result<(), String>`
- `exec_ssh_command(session_id: String, command: String) -> Result<String, String>`（走 `manager.get_active(&session_id)` 拿连接，再 `exec_command`）

> `exec_ssh_command` 是本项唯一的"顺带收益"：它同时为将来的脚本执行、批量运维、`设置中心-探测` 打底。请务必加，不要为了省事先只读本地历史。

### 前端设计

`src/components/CommandSuggestBar.vue`（新组件）：
- props：`candidates: string[]`、`activeIndex: number`、`visible: boolean`、`anchorRect?`
- 只负责渲染，`teleport` 到终端容器内，绝对定位于终端底部（**不追求像素级跟随光标**）。
  > 用 xterm 内部坐标 API（`term._core`）算光标像素位置不稳、跨版本会坏；底部浮动条是可靠做法，竞品也有用底部条的。
- `Tab` 接受、`↑/↓` 切换、`Esc` 关闭、点击接受。

`TerminalView.vue`：
1. `TerminalSession` 加 `lineBuffer: string` 与 `historyCursor: number`。
2. 在 `term.onData` 里维护 `lineBuffer`：
   - 普通字符 → 追加
   - `\x7f`（退格）→ 去掉末尾字符
   - `\x15`（Ctrl+U）→ 清空
   - `\x03`（Ctrl+C）→ 清空并重置历史游标
   - `\r` → 提交：非空且通过过滤规则则 `push_command_history(hostKey, cmd)`，然后清空
   - `\x1b[A` / `\x1b[B` → 由本组件拦截，走"翻历史"而非透传（注意：**只有在当前行为空时才拦截**，否则会破坏 vim/less 的上下键；更稳妥的做法是仅在 `lineBuffer` 非空时把历史当成补全候选，不接管裸 `↑/↓`）
3. 候选计算：
   - 取 `get_command_history(hostKey)` 结果（会话建立时加载一次并缓存在 `Map<hostKey, HistoryEntry[]>`，提交后用返回值增量更新，避免每次都 IPC）。
   - 输入 `push`（长度 ≥ 2）时：前缀匹配优先，其次子串匹配，排除与当前输入完全相同的项，去重后取前 8 条。
   - 已知子命令扩展（可选加分项）：`docker ` / `systemctl ` / `git ` 之后的候选来自对应的**内置静态词表**（不做调用远端的 `--help` 探测，避免慢）。
4. 远端历史拉取（可开关，默认开）：会话建立后调用 `exec_ssh_command(sessionId, "cat ~/.bash_history 2>/dev/null | tail -n 200")`，逐行 `push_command_history`（`count` 不累加，只做入库；需要一个 `bootstrap: bool` 参数或后端用"若已存在则不增 count"的变体 —— **建议后端新增 `import_command_history(host_key, cmds: Vec<String>)`，语义是"仅在不存在时插入，count=1"**，避免把远端历史刷成高频）。
5. 非 Tauri 环境（浏览器预览）用 localStorage 键 `snail_ssh_history` 降级，保证预览可用。

### 边界与坑
- **绝不接管裸 `↑/↓`**：一旦接管，`vim` / `less` / `man` / `mysql` 交互全废。历史翻阅只做"Tab 接受候选"和"点击候选"，把翻历史留给远端 shell 自己（readline 本来就有）。若确实想要 `↑` 翻本地历史，必须做"仅当远端无交互程序"的判定 —— 不可靠，不做。
- 敏感命令不要入库：正则排除含 `password`、`passwd`、`-p`+密码、`token=`、`--password` 的命令（列表可维护成常量）。
- `term.onData` 拿到的是按键序列，粘贴多行文本会一次进来一整段 —— 要按 `\n`/`\r` 拆开逐行处理，否则历史会记成一条巨串。
- 补全条出现时不能遮挡终端最后一行内容；底部条的容器高度要用 `absolute bottom-0` + 半透明背景，并在开关时调用 `fitActive()` 或让终端预留一行高度（**改动 xterm 容器高度必须重新 fit，否则行列数会错**）。
- IPC 频率：输入每个字符都查历史会打爆 IPC，所以历史必须在内存缓存，只有"提交/建连/清空"才走后端。

### 验证步骤
1. `cargo test`（`storage.rs` 已有单测范式，为 `history.rs` 的裁剪/去重/过滤规则补 3~4 个单测）。
2. `cargo check` + `pnpm build` 通过。
3. `pnpm tauri dev`：
   - 输入 `dock` → 候选条出现 `docker ps` → Tab 接受 → 回车执行 → 再次输入 `dock` 候选顺序变化（频次生效）。
   - 断开重连后历史仍在（**关键**：验证 `~/.snail-ssh/history.json` 内容与 `count`）。
   - 在 `vim` 中确认上下键、`Esc`、`Tab` 行为未被破坏。
   - 粘贴一段 3 行文本，确认没记成一条命令。

---

## 7. 第 4 项：会话日志

### 目标 / 验收
- 每个终端标签页可独立开关"记录会话日志"，开启时状态栏显示 `REC` 红点。
- 日志落盘到 `~/.snail-ssh/logs/<host>/<timestamp>_<sessionShortId>.log`，UTF-8 纯文本，**已脱敏**。
- 写日志不阻塞终端输出（读线程不能被磁盘 IO 拖慢）。
- 提供"打开日志目录"入口；停止记录时 Toast 显示完整文件路径。

### 竞品依据
SecureCRT / Xshell / MobaXterm 标配；运维排障刚需。

### 现状
- `src-tauri/src/ssh.rs:188` 起是后台读线程：非阻塞读 → UTF-8 边界解码 → `app.emit("ssh-stdout-{id}")`。**这里是唯一收口**。
- `ActiveSession` 结构在 `ssh.rs:130-150` 附近，含 `shell`（`Arc<Mutex<Channel>>`）与 SFTP 会话槽位，可再加一个日志槽。
- `storage.rs` 有 `get_config_file_path()` 私有函数给出 `~/.snail-ssh/` 路径（可提取为 `pub(crate) fn app_data_dir() -> PathBuf` 复用）。
- 项目**没有** `tauri-plugin-opener` / `tauri-plugin-shell`，打开目录需要自己实现。

### 改动文件
- `src-tauri/src/ssh.rs`（读线程落盘 + 会话级日志状态 + 开关方法）
- `src-tauri/src/lib.rs`（`set_session_logging` / `get_session_log_path` / `reveal_in_file_manager`）
- `src-tauri/src/storage.rs`（提取 `app_data_dir()` 为 `pub(crate)`）
- `src/api/tauri.ts`
- `src/components/TerminalView.vue`（菜单项 + REC 指示）

### 实现方案

1. **落盘位置**：新增 `pub(crate) fn app_data_dir() -> PathBuf`（`~/.snail-ssh/`，不存在则创建），日志放 `app_data_dir().join("logs").join(sanitize(host)).join(format!("{}_{}.log", ts, short_id))`。
   - `sanitize(host)`：把 `/ \ : * ? " < > |` 替换为 `_`（Windows 文件名限制）。

2. **不要在读线程里同步写磁盘**：为每个开着日志的会话建一条 `std::sync::mpsc::sync_channel::<String>(256)` + 一个 writer 线程（`std::thread::spawn`）。读线程只做 `try_send`，`Full` 或 `Disconnected` 时**丢弃日志、不阻塞**，并 `eprintln!` 告警一次（避免刷屏）。

3. `ActiveSession` 增加：
   ```rust
   log_sink: Mutex<Option<LogSink>>,   // LogSink { path: PathBuf, tx: SyncSender<String> }
   ```
   `SshManager` 增加方法：
   ```rust
   /// 开启/关闭会话日志，返回最终日志文件路径（关闭时为 None）
   pub fn set_session_logging(&self, session_id: &str, enabled: bool, host_label: &str) -> Result<Option<String>, String>;
   ```
4. 读线程里，在成功解码出一段 `String` 之后（`ssh.rs:211` UTF-8 解码那段之后）：
   - 先做前端 emit（保持原有行为不变，顺序不要调）。
   - 再 `if let Some(sink) = ... { let _ = sink.tx.try_send(redact(&text)); }`。
5. **脱敏函数**（放 `ssh.rs` 或新 `log_sink.rs`），落盘前执行：
   ```rust
   // 覆盖场景：password=xxx、PASSWORD: xxx、--password xxx、-pXXXX、token=/secret=/api_key= 等
   // 统一替换为 $1=***，并在文件头写入一行说明：
   //   # SnailSSH session log (credentials redacted): <host> <started at>
   ```
   **必须写清"日志已脱敏但与屏幕内容可能不一致"**，放在 README 与 Toast 里。
6. 关闭会话时（`close_session`，`ssh.rs:313`）要显式 drop `log_sink` 让 writer 线程退出并 flush。
7. `reveal_in_file_manager` 实现（避免引入新插件）：
   ```rust
   // Windows: Command::new("explorer").arg(path)
   // macOS:   Command::new("open").arg(path)
   // Linux:   Command::new("xdg-open").arg(path)
   // 先校验 path 存在且位于 app_data_dir 之下，防止被前端任意传参调用成"打开任意路径"
   ```
   > 安全约束很重要：这个 command 只允许打开 `~/.snail-ssh/` 下的路径。

### 前端改动
- `TerminalView.vue` 的标签右键菜单加一项：`开始/停止记录会话日志`（按当前状态切换文案）。
- 顶栏右侧加 `REC` 指示：当前活动标签在录制时显示一个闪烁红点 + 文件名 tooltip。
- 状态栏（`TerminalView.vue:1xx` 底部那条）加一项 `<日志: 记录中 / 已关闭>`。
- 菜单里再加一项"打开日志目录" → `revealInFileManagerApi(appDataLogDir)`。
  - 需要后端提供日志根目录：复用 `get_session_log_path` 返回的路径取父目录，或新增 `get_log_dir()`。**建议 `set_session_logging` 直接返回路径，`reveal_in_file_manager` 接受该路径的父目录**，减少新命令。
- 默认关闭（不要默认开，避免磁盘悄悄被写满），开关状态记 `localStorage` 用于"下次自动开启"的记忆。

### 边界与坑
- 二进制输出（`cat` 一个二进制文件、`sz` 传输）会让日志变成乱码：落盘前对不可打印字符做转义或直接跳过该段（建议：含 `\0` 的段跳过不写），并记一行说明。
- 日志会包含远端主机名、路径、可能的业务数据 —— README 要写明"日志为明文，注意保管"，并在默认情况下关闭。
- 日志文件不设自动清理（本项不做轮转），但要在 UI 上显示当前日志大小或提供"打开目录"让用户自己清理。
- `log_sink` 必须用 `parking_lot::Mutex` 且**不要在持锁期间做 send**（`try_send` 无阻塞，但仍建议先 clone 出 `tx`）。
- 读线程已经有 5ms 轮询节奏（`IO_POLL_INTERVAL`），不要在它内部做任何 O(n) 的死循环或正则回溯风险高的匹配（脱敏正则要写成非贪婪、有长度上限）。

### 验证步骤
1. `cargo check` + `pnpm build` 通过。
2. `pnpm tauri dev`：
   - 开启日志 → 执行 `echo hello; export DB_PASSWORD=supersecret; echo done` → 停止日志 → 打开日志文件确认包含 `hello` / `done`，且 `DB_PASSWORD=***`。
   - `cat /dev/urandom | head -c 1000` 不导致终端卡顿、日志不变成天文数字体积。
   - `for i in $(seq 1 100000); do echo line $i; done` 期间终端输出不卡（验证 try_send 不阻塞）。
   - 关闭会话后确认文件已 flush（末尾内容完整）。
   - 尝试传一个非日志目录路径给 `reveal_in_file_manager`，应被拒绝。

---

## 8. 第 6 项：拖拽上传

### 目标 / 验收
- 从 Windows 资源管理器拖文件到 SFTP 区域 → 出现"释放以上传到 /path"高亮遮罩 → 释放后进入现有传输队列。
- 支持多文件；支持拖入目录（递归上传，保留目录结构）。
- 拖入位置不在 SFTP/文件树区域时给出明确提示，不静默失败。
- **明确写入限制**：不支持把远端文件拖出到资源管理器（Tauri 无此能力，竞品也没有）。

### 竞品依据
Termius / WinSCP / FileZilla 的招牌交互；当前项目只能走"点击上传 → 系统对话框"，多一步。

### 现状
- `src/App.vue` 的 `handleUploadTrigger(session, remoteDir)`（`App.vue:6xx`）已经封装了"选文件 → 建任务 → 调 `sftpUploadFileApi`"，拖拽只需换一个"文件来源"。
- `src/api/tauri.ts` 的 `pickLocalFile()` 目前 `multiple: false`。
- `src-tauri/src/sftp.rs` 只有 `sftp_upload_file`（单文件），**没有** `upload_dir`；`sftp_download_dir` + `collect_download_entries` 可作为镜像模板。
- 项目未使用任何 Tauri 拖拽 API。

### 改动文件
- `src-tauri/src/sftp.rs`（新增 `sftp_upload_dir`）
- `src-tauri/src/lib.rs`（新增 command + 注册）
- `src/api/tauri.ts`（`onDragDrop` 封装 + `sftpUploadDirApi` + `pickLocalFiles` 多选）
- `src/App.vue`（拖拽事件总线 + 落点分发）
- `src/components/SftpView.vue`、`src/components/FileTree.vue`（加 `data-drop-target` 标记 + 高亮遮罩）
- `src/types/index.ts`

### 实现方案

1. **必须用 Tauri 的拖拽事件，不能用 HTML5 `drop`**：Tauri webview 默认拦截 OS 级文件拖放（`dragDropEnabled` 默认 true），HTML5 `dragover/drop` 收不到文件路径。
   ```ts
   // src/api/tauri.ts
   import { getCurrentWebview } from '@tauri-apps/api/webview'
   export async function onFileDragDrop(handler: (e: {
     type: 'enter' | 'over' | 'drop' | 'leave'
     paths: string[]
     x: number   // CSS 像素
     y: number
   }) => void): Promise<UnlistenFn>
   // 内部：getCurrentWebview().onDragDropEvent(evt => {
   //   const p = evt.payload
   //   handler({ type: p.type, paths: p.paths ?? [], x: p.position.x / devicePixelRatio, y: p.position.y / devicePixelRatio })
   // })
   ```
   > `position` 是**物理像素**，必须除以 `window.devicePixelRatio` 才能和 `elementFromPoint` 对齐；高 DPI 屏最容易在这里出错，务必实测。

2. `App.vue` 在 `onMounted` 注册 `onFileDragDrop`，用一个 `dragState = ref({ active: false, paths: [], x: 0, y: 0, targetDir: '' | null, targetSessionId: '' })`：
   - `over`：`const el = document.elementFromPoint(x, y)?.closest('[data-drop-target="sftp"]')` → 命中时读该元素上的 `data-remote-path` 与所在会话 id，写入 `targetDir`，`active = true`。
   - `drop`：`active = false`，若 `targetDir` 为 null → `triggerToast('请拖拽到左侧文件树或 SFTP 列表区域', 'warning')`；否则对每个路径判定"文件/目录"（Rust 侧用 `std::fs::metadata`）→ 分别走 `sftpUploadFileApi` / `sftpUploadDirApi` 建任务。
   - `leave`/`enter`：维护 `active`，`enter` 时若落在目标区域内可先高亮。
3. 落点标记：`SftpView.vue` 的根节点加 `data-drop-target="sftp"` + `:data-remote-path="currentPath"`；`FileTree.vue` 的树容器加同样属性 + 当前路径。用 `getAttribute` 读取即可，无需 props 上抛。
4. 高亮遮罩：在 `App.vue` 里渲染一个全屏 `pointer-events-none` 的 overlay，或者由两个子组件根据 `dragActive` prop 各自渲染。**建议 App 级统一渲染一个跟随鼠标的小标签**（"释放以上传到 /etc/nginx"），成本最低、不会遮挡拖拽判定。
   > 注意：遮罩如果是 `pointer-events: none` 就不会干扰 `elementFromPoint`；如果忘了加，`elementFromPoint` 会返回遮罩本身导致判定失败 —— 这是最常见的坑。
5. 多文件：`pickLocalFile` 增加 `pickLocalFiles(): Promise<string[]>`（`multiple: true`），并把"上传文件"按钮改为支持多选（顺带解决现有只能传单个文件的问题）。

### 新增后端函数

```rust
// src-tauri/src/sftp.rs —— 镜像 sftp_download_dir 的结构
pub fn sftp_upload_dir(
    session_arc: Arc<Mutex<ssh2::Session>>,
    local_path: &str,
    remote_dir: &str,     // 目标父目录，最终会创建 remote_dir/<basename(local_path)>/
    task_id: &str,
    app_handle: AppHandle,
) -> Result<(), String>;
```
要点（照抄 `download_dir` 的两遍式设计）：
- 第一遍 `std::fs::read_dir` 递归枚举本地文件并统计总字节数（进度条需要百分比）。
- 第二遍逐文件上传，远端目录不存在时先 `sftp.mkdir(path, 0o755)` 逐级创建（**注意 ssh2 没有 mkdir -p，要自己按 `/` 逐段创建并忽略 "already exists" 错误**）。
- 软链接策略与 `collect_download_entries` 保持对称：只跟随指向普通文件的软链接，目录软链接跳过（防成环）。
- 复用 `ProgressReporter`，不要新写一套进度逻辑。

### 边界与坑
- `dragDropEnabled` 打开时会**吞掉 HTML5 拖放事件**：将来若要做"标签页拖拽排序"或"树节点拖拽移动"，会与它冲突。目前项目没有这些功能，先在 README/注释里记一笔风险。
- Windows 上拖拽期间鼠标经过子元素会频繁触发 `over`，要做节流（每 50ms 处理一次）避免 `elementFromPoint` 抖动。
- 拖入的是**目录**时，`paths` 里给的是目录路径，前端无法判断类型 —— 要么后端提供 `local_path_is_dir(path)` 命令，要么直接在 `sftpUploadDirApi` 里让后端自行判定（**推荐后者**：新增 `sftp_upload_path` 统一入口，内部按 metadata 分派文件/目录，前端不用判断）。这是更省事的终态设计，建议直接做成 `sftp_upload_path`。
- 大目录（几千文件）的第一遍枚举可能耗时可观：要在枚举阶段就发一条 `status: "scanning"` 的进度事件，否则用户以为卡死。

### 验证步骤
1. `cargo check` + `pnpm build` 通过。
2. `pnpm tauri dev`：
   - 拖 3 个文件到 SFTP 列表 → 遮罩出现 → 释放 → 队列出现 3 条任务 → 目录自动刷新可见。
   - 拖一个含子目录的文件夹 → 远端结构一致、文件内容正确（用 `md5sum` 对比 2~3 个文件）。
   - 拖到终端区域或标题栏 → 出现"请拖拽到文件区域"提示。
   - 150% / 200% 缩放屏幕上重复上面步骤（验证 `devicePixelRatio` 换算）。

---

## 9. 第 1 项：传输暂停 / 取消 / 重试 / 断点续传

### 目标 / 验收
- 传输任务支持：暂停、继续、取消、失败后重试。
- 断点续传：暂停后继续、取消后重试、**应用重启后对同一路径重新发起**都能从已传字节继续。
- 队列卡片上有对应按钮与状态文案（进行中 / 已暂停 / 已完成 / 已失败 / 已取消）。
- 取消的任务不标记为"失败"，显示为"已取消"。
- 任务失败/取消后不删除本地已下载的部分文件（续传依赖它）。

### 竞品依据
WinSCP 的传输队列（暂停/继续/后台传输/断点续传/冲突策略）是 SFTP 侧的行业基准；当前 README 已把"无暂停/取消/续传"列为已知限制。

### 现状（关键）
- `src-tauri/src/sftp.rs` 四个传输函数结构完全一致：`loop { read → write_all → reporter.advance → reporter.report }`：
  - `sftp_upload_file`（`sftp.rs:290` 附近）
  - `sftp_download_file`（`sftp.rs:330` 附近）
  - `sftp_download_dir`（`sftp.rs:390` 附近内部循环）
- `ProgressReporter`（`sftp.rs:47`）有 `transferred / total / window_*`，扩展成本低。
- `TransferProgressPayload.status` 是 `String`（`sftp.rs:34`），前端类型 `SftpProgressPayload.status`（`src/types/index.ts:76`）目前是 `'running' | 'completed' | 'failed'`。
- 前端任务模型：`TransferTask`（`src/types/index.ts:84`），建任务在 `App.vue` 的 `createTransferTask()` / `runTransfer()`（`App.vue:5xx`）。
- `TransferDrawer.vue` 每张卡片目前只有"移除"，没有控制按钮。
- 每次传输**单独开一个 `spawn_blocking` 线程**（`lib.rs` 的 `run_blocking`），所以"取消"必须在后端用共享标志实现，不能靠 drop future。

### 改动文件
- `src-tauri/src/transfer.rs`（新建：控制信号注册表）
- `src-tauri/src/sftp.rs`（4 个传输函数接入信号 + 续传）
- `src-tauri/src/lib.rs`（`mod transfer;` + 2~3 个新 command + 注册 `manage()`）
- `src/api/tauri.ts`
- `src/types/index.ts`
- `src/App.vue`
- `src/components/TransferDrawer.vue`

### 后端设计

```rust
// src-tauri/src/transfer.rs（新建）
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use std::collections::HashMap;

#[derive(Default)]
pub struct TransferSignal {
    cancelled: AtomicBool,
    paused: AtomicBool,
    /// 供 UI 查询的最近一次已知进度（续传起点）
    transferred: std::sync::atomic::AtomicU64,
}

impl TransferSignal {
    pub fn is_cancelled(&self) -> bool { self.cancelled.load(Ordering::Relaxed) }
    pub fn is_paused(&self) -> bool { self.paused.load(Ordering::Relaxed) }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::Relaxed); self.paused.store(false, Ordering::Relaxed); }
    pub fn pause(&self)  { self.paused.store(true, Ordering::Relaxed); }
    pub fn resume(&self) { self.paused.store(false, Ordering::Relaxed); }
    pub fn set_transferred(&self, n: u64) { self.transferred.store(n, Ordering::Relaxed); }
    pub fn transferred(&self) -> u64 { self.transferred.load(Ordering::Relaxed) }
}

#[derive(Default, Clone)]
pub struct TransferRegistry {
    inner: Arc<Mutex<HashMap<String, Arc<TransferSignal>>>>,
}

impl TransferRegistry {
    pub fn new() -> Self { Self::default() }
    pub fn register(&self, task_id: &str) -> Arc<TransferSignal>;   // 若已存在则复用（重试场景）
    pub fn get(&self, task_id: &str) -> Option<Arc<TransferSignal>>;
    pub fn finish(&self, task_id: &str);                            // 传输结束后清理，避免 Map 泄漏
}
```

`lib.rs`：`.manage(TransferRegistry::new())`；`sftp_upload_file` / `sftp_download_file` / `sftp_download_dir`(**改为 `sftp_upload_path` / `sftp_download_path` 更合适，见下**) 各加一个 `registry: State<'_, TransferRegistry>` 参数，`let signal = registry.register(&task_id);`，把 `Arc<TransferSignal>` 传进 `sftp.rs` 的函数。

> 建议本项顺便把"单文件/目录"合并为统一入口，减少 4 份重复代码：
> ```rust
> pub fn sftp_upload_path(session, local_path, remote_dir, task_id, signal, app) -> Result<(), String>; // 内部按 metadata 分派
> pub fn sftp_download_path(session, remote_path, local_path, task_id, signal, app) -> Result<(), String>;
> ```
> 保留旧的 `sftp_upload_file` 等命令签名做薄封装，避免破坏前端调用点（**兼容优先**）。

**取消/暂停检查点**（在每次 `read` 之后、`write_all` 之前）：
```rust
if signal.is_cancelled() {
    reporter.report(&app, "cancelled", true);
    return Err("TRANSFER_CANCELLED".into());   // 前端按此字符串识别，不显示为"失败"
}
while signal.is_paused() && !signal.is_cancelled() {
    std::thread::sleep(Duration::from_millis(120));
}
if signal.is_cancelled() { /* 同上 */ }
signal.set_transferred(reporter.transferred);  // 供续传起点
```
> 注意：暂停是"阻塞在 spawn_blocking 线程上轮询"，不要用 async await（这些函数是同步的）。120ms 粒度足够，CPU 无压力。

**续传实现**：
```rust
// 上传
let mut offset = 0u64;
if let Ok(meta) = std::fs::metadata(remote_path) {      // 远端已存在的部分文件
    let local_len = local_file.metadata()?.len();
    if meta.len() < local_len { offset = meta.len(); }   // 只有"远端更短"才续传，否则从头来（避免污染）
}
local_file.seek(SeekFrom::Start(offset))?;
let mut remote_file = sftp.open_mode(Path::new(remote_path), OpenFlags::WRITE | OpenFlags::CREATE, 0o644, OpenType::File)?;
// 注意：不能用 sftp.create()，它会 TRUNC；同样不要带 OpenFlags::TRUNC
remote_file.seek(SeekFrom::Start(offset))?;
let mut reporter = ProgressReporter::new(task_id, total_size);  // total 仍是完整文件大小
reporter.advance(offset);                                        // 让进度从 offset 起算
```
```rust
// 下载
let mut offset = 0u64;
if let Ok(meta) = std::fs::metadata(local_path) {
    let remote_len = remote_file.stat()?.size.unwrap_or(0);
    if meta.len() < remote_len { offset = meta.len(); }
}
remote_file.seek(SeekFrom::Start(offset))?;
let mut local_file = std::fs::OpenOptions::new().create(true).write(true).open(local_path)?;
local_file.seek(SeekFrom::Start(offset))?;
```
```rust
// 目录下载的续传：逐文件复用上面的逻辑，跳过"本地文件已与远端等大且 mtime 不新"的条目，
// total 统计时已传部分的字节也要计入（否则进度条会回退）
```
> 需要 `use std::io::{Seek, SeekFrom}` 与 `ssh2::{OpenFlags, OpenType}`。`ssh2::File` 同时实现了 `Read + Write + Seek`，所以远端 seek 是可行的。

**冲突/一致性保护（必须有）**：
- 上传续传前，若远端 `mtime` 比本地源文件 `mtime` 新，说明期间被改过 → 放弃续传，从头传（或返回提示）。落地时至少做长度校验这一层，mtime 校验作为加分。
- 下载续传同理：本地文件 mtime 比远端新 → 说明本地被改过 → 提示"本地文件已修改，是否覆盖"。

**新增 command**：
```rust
#[tauri::command]
fn sftp_transfer_control(
    task_id: String,
    action: String,           // "pause" | "resume" | "cancel"
    registry: State<'_, TransferRegistry>,
) -> Result<(), String>;      // 找不到 task_id 时返回明确错误，不要静默成功
```
> 同步命令即可（不阻塞），不需要 `run_blocking`。

### 事件协议扩展
`TransferProgressPayload.status` 取值扩展为：`running` | `paused` | `completed` | `failed` | `cancelled`。
- `paused`：暂停生效后立即 `report(..., "paused", true)` 一次（前端切按钮状态）。
- `cancelled`：取消生效后 `report(..., "cancelled", true)`，然后返回 `Err("TRANSFER_CANCELLED")`。
- 前端 `runTransfer()` 的 `catch` 里要先判断 `String(e).includes('TRANSFER_CANCELLED')` → 设 `status = 'cancelled'`、Toast 用 info 级、**不触发** `sftpRefreshSignal`（部分文件不该被当成完成）。

### 前端改动
`src/types/index.ts`：
```ts
export type TransferStatus = 'running' | 'paused' | 'completed' | 'failed' | 'cancelled'
export interface TransferTask {
  id: string; name: string; type: 'upload' | 'download'
  progress: number; speed: string; size: string
  status: TransferStatus
  // 续传/重试所需上下文（重试时原样重放）
  context?: { sessionId: string; localPath: string; remotePath: string; isDir: boolean; upload: boolean }
}
export interface SftpProgressPayload { /* status: TransferStatus */ }
```

`src/api/tauri.ts`：`sftpTransferControlApi(taskId, action)`。

`src/App.vue`：
- `createTransferTask(...)` 增加第 4 个参数 `context`。
- `runTransfer(task, start)` 改为可从 `task.context` 重新构造调用（重试就再调一次）。
- 新增 `handlePauseTask(task)` / `handleResumeTask(task)` / `handleCancelTask(task)` / `handleRetryTask(task)`。
- `handleClearCompletedTasks()` 现在的实现是 `filter(t => t.status === 'running')` —— 它会**误删 paused 的任务**，要改成 `filter(t => t.status === 'running' || t.status === 'paused')`。
- `handleCloseWindow()` 的"运行中任务数"统计（`App.vue:4xx`）也要把 paused 计入。

`src/components/TransferDrawer.vue`：
- 卡片操作区按状态渲染按钮：进行中 → 暂停 + 取消；已暂停 → 继续 + 取消；失败/已取消 → 重试 + 移除；完成 → 移除。
- 状态徽标文案：`已完成 / 进行中 / 已暂停 / 已失败 / 已取消`（用 `v-if` 链替换现在嵌套三元表达式，现在那串三元已经很难读了）。
- 已暂停时进度条用琥珀色、已取消用灰色。

### 边界与坑
- **`ProgressReporter::new` 必须接收 total = 完整文件大小**，然后 `advance(offset)`；如果把 total 改成"剩余大小"，进度条会从 0 开始跳到 100，看起来像 bug。
- `OpenFlags` 千万不要带 `TRUNC`，否则"续传"变成"重传"且会先清空远端文件（危险！**这条最容易写出数据丢失**）。
- 取消是"协作式"的，最小响应粒度是一个 64KB 块（`TRANSFER_CHUNK_SIZE`），本地磁盘上的表现是几乎瞬时。不需要改块大小。
- 任务结束（成功/失败/取消）一定要 `registry.finish(&task_id)`，否则 Map 会随任务数增长。
- 前端 `onSftpProgress` 的 `unlisten()` 目前在 `finally` 里调用（`App.vue` `runTransfer`）—— 重试时必须重新 `listen`，因为旧的 unlisten 已经把监听摘掉了。
- 暂停期间用户可能关闭会话/退出应用：`close_session` 要能唤醒暂停中的线程（`signal.cancel()`），否则线程会永久 sleep 在 while 循环里。**在 `SshManager::close_session` 里对所有该会话的 task 调 cancel**，或在 `TransferRegistry` 增加 `cancel_by_session`（需要 registry 记录 task → session 映射，实现时顺手加上）。
- 目录下载里"已跳过 / 已续传"的文件不要让进度条回退，也不要让进度卡在 99%。

### 验证步骤
1. `cargo check` + `cargo test`（为 `TransferRegistry` 的 register/get/finish 与续传 offset 计算补单测）+ `pnpm build` 通过。
2. `pnpm tauri dev`，用一个大文件（≥ 500MB，或对 `truncate -s 1G` 造的文件）：
   1. 下载中点暂停 → 进度停在当前百分比、速度归零 → 继续 → 从原位置接着走。
   2. 下载中取消 → 状态"已取消"、本地残留部分文件 → 点重试 → **日志/进度显示从部分位置继续**（用 `ls -l` 观察本地文件大小不回退到 0）。
   3. 上传同样流程，在远端 `ls -l` 确认文件大小是递增的、不是先归零。
   4. 上传/下载一个多级目录，中途暂停再继续，完成后用 `md5sum -c` 校验若干个文件。
   5. 打开 500MB 任务，点暂停，然后关闭该会话 → 应用不卡死、线程正常退出。
3. 边界：对同一个 task_id 重复 pause/resume 不应报错；对不存在的 task_id 调 control 应返回明确错误。

---

## 10. 新增接口汇总（给新会话做交叉检查）

### 新增 Tauri command

| 命令 | 归属项 | 参数 | 说明 |
|---|---|---|---|
| `sftp_rename` | 5 | session_id, from, to, overwrite | 重命名/移动 |
| `sftp_chmod` | 5 | session_id, path, mode | 八进制权限位 |
| `sftp_chown` | 5 | session_id, path, uid?, gid? | 数字 id |
| `sftp_touch` | 5 | session_id, path | 新建空白文件，已存在报错 |
| `sftp_stat` | 5（可选） | session_id, path | 单条目属性 |
| `get_command_history` | 2 | host_key | |
| `push_command_history` | 2 | host_key, cmd | |
| `import_command_history` | 2 | host_key, cmds | 仅在不存在时插入 |
| `clear_command_history` | 2 | host_key? | None 表示清全部 |
| `exec_ssh_command` | 2 | session_id, command | 复用 `run_probe_command` |
| `set_session_logging` | 4 | session_id, enabled, host_label | 返回日志路径 |
| `reveal_in_file_manager` | 4 | path | 仅允许 `~/.snail-ssh/` 下的路径 |
| `sftp_upload_path` | 6 | session_id, local_path, remote_dir, task_id | 按 metadata 分派文件/目录 |
| `sftp_transfer_control` | 1 | task_id, action | pause/resume/cancel |

### 新增前端 API（`src/api/tauri.ts`）
`sftpRenameApi`、`sftpChmodApi`、`sftpChownApi`、`sftpTouchApi`、`sftpStatApi`、`getCommandHistoryApi`、`pushCommandHistoryApi`、`importCommandHistoryApi`、`clearCommandHistoryApi`、`execSshCommandApi`、`setSessionLoggingApi`、`revealInFileManagerApi`、`sftpUploadPathApi`、`sftpTransferControlApi`、`pickLocalFiles`、`onFileDragDrop`。

### 新增/修改的本地文件
- 新建：`~/.snail-ssh/history.json`（第 2 项）、`~/.snail-ssh/logs/`（第 4 项）
- 新增 Rust 模块：`src-tauri/src/history.rs`、`src-tauri/src/transfer.rs`（可选 `log_sink.rs`）
- 新增前端文件：`src/composables/useRecentHosts.ts`、`src/composables/useTerminalHighlight.ts`、`src/components/CommandSuggestBar.vue`
- 新增 npm 依赖：`@xterm/addon-search`、`@xterm/addon-unicode11`

---

## 11. 建议提交拆分

一项一个提交，不要混在一起。建议的 commit 信息（沿用现有 Conventional Commits 风格）：

```
feat(terminal): 支持终端内搜索与快捷键剪贴板操作
feat(host): 支持快速连接与真实最近连接列表
feat(terminal): 支持关键字高亮规则与可点击 IP/文件行号
feat(sftp): 补齐重命名/权限/属主/新建文件与路径直输、名称过滤
feat(terminal): 支持命令历史持久化与联想补全
feat(logging): 支持会话日志记录与凭证脱敏
feat(sftp): 支持拖拽多文件与目录上传
feat(sftp): 支持传输暂停/取消/重试与断点续传
docs: 更新 README 功能清单与本次新增限制说明
```

每项完成后**顺手更新 `README.md`**：把新功能从"尚未实现/已知限制"里划掉，并把新增的限制（如"不支持拖出到资源管理器"、"日志为明文"、"最近连接存于 localStorage"）写进"已知限制"。当前 README 已经落后于代码（未记录分栏工作区、树形浏览器、底部监控条），一并修正。

---

## 12. 风险与回滚

| 风险 | 影响 | 应对 |
|---|---|---|
| `sftp.rs` 传输函数大改（第 1 项） | 可能引入数据损坏/覆盖远端文件 | 严格禁止 `OpenFlags::TRUNC`；改动前备份 `sftp.rs`；先在 `/tmp` 目录用测试文件验证，再动真实路径；每步都跑 md5 校验 |
| 高亮扫描（第 8 项） | 大日志时 UI 卡死 | 强制"只扫新增行 + 每 tick 上限 + 节流"；提供开关；压测 20 万行 |
| 拖拽事件与 HTML5 拖放冲突（第 6 项） | 将来做标签拖拽排序时会踩坑 | 在 `tauri.conf.json` 或代码注释里记录；当前不做 HTML5 拖放 |
| xterm 内部 API 兼容性 | 升级 xterm 后 decoration/linkProvider 失效 | 集中在 `useTerminalHighlight.ts` / `useTerminalSearch` 里，不散落；升级时只改这一处 |
| 补全接管键盘 | 破坏 vim/less（**最容易被骂**） | 明确不接管裸 `↑/↓`；补全只响应 Tab 与点击 |
| 新增依赖被 Tauri 打包影响 | 包体积/`cargo check` 变慢 | 只加 `@xterm/addon-*`，不引入 tauri 插件 |
| 日志落盘 | 磁盘占用、信息泄露 | 默认关闭；写入前脱敏；README 明示 |

回滚方式：每项独立提交，出问题 `git revert <该提交>` 即可；Rust 侧改动前按项目约定把被修改的既有文件备份到 `backups/`。

---

## 13. 每项的完成检查清单（DoD）

每项做完后逐条确认：

- [ ] `pnpm build`（含 `vue-tsc --noEmit`）通过
- [ ] `cd src-tauri && cargo check` 通过（涉及 Rust 逻辑的项额外跑 `cargo test`）
- [ ] `pnpm dev` 浏览器预览模式不报错、降级分支可用
- [ ] `pnpm tauri dev` 真实主机手工验证本文档给出的验证步骤
- [ ] 新增 command 已注册进 `generate_handler!`
- [ ] 新增前端 API 都有 `isTauri()` 降级分支
- [ ] 新增 TS 类型放进 `src/types/index.ts`，没有散落的 `any`
- [ ] `README.md` 与"已知限制"章节同步更新
- [ ] 没有顺手重构无关代码、没有改动既有 UI 设计语言
- [ ] 未执行的验证如实说明（"已完成静态检查，未执行运行验证"）
```
