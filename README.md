# SnailSSH - 现代化跨平台 SSH & SFTP 桌面客户端

> **下一代现代化、高性能、轻量级 SSH 终端与 SFTP 传输套件。**
> 基于 **Tauri v2 + Rust 后端 + Vue 3 / TypeScript 前端** 架构，适配 Windows、macOS 与 Linux 三大主流操作系统。

---

## 🚀 架构与技术选型

| 层面 | 选型 | 特性与优势 |
| :--- | :--- | :--- |
| **桌面框架** | **Tauri v2** | 原生轻量化（安装包仅约 15MB，常驻内存仅 30~50MB，远优于 Electron 的 300MB+） |
| **后端核心** | **Rust (`ssh2`, `tokio`, `tauri 2.0`)** | 原生多线程非阻塞 I/O，高性能流式 SFTP 传输与 PTY 进程管理，安全可控 |
| **前端体系** | **Vue 3 + Vite + TypeScript** | 现代 Composition API，极速 HMR 热重载与强类型支持 |
| **终端引擎** | **`@xterm/xterm` + `addon-fit`** | 行业标准级 xterm.js ANSI 彩色终端渲染与平滑交互 |
| **样式与设计** | **Tailwind CSS + Lucide Icons** | Warp / Raycast / Linear 暗黑风格设计系统，毛玻璃磨砂与发光微质感 |

---

## ✨ 第一版核心功能实现

### 1. 🖥️ 服务器资产管理 (Host Manager)
* **卡片网格 (Grid) / 紧凑表格 (List)** 自由切换。
* **真实监控指标**：连接成功后自动执行远端探测脚本，回填系统发行版、CPU/内存/磁盘利用率与 TCP+SSH 握手延迟；未探测过的主机不再展示伪造数值。
* **分组与标签检索**：按生产集群 (Prod)、测试环境 (Stage)、个人实验机 (Cloud) 及自定义 Tag 即时过滤。
* **快捷直连入口**：卡片上提供一键 `连接 SSH` 与 `SFTP`。
* **新建/编辑服务器弹窗 (Modal)**：支持主机别名、分组、IP、端口、认证方式（密码 / 私钥 PEM 或路径 / SSH Agent）与真实连通性测试。
* **资产持久化存储**：主机元数据存于 `~/.snail-ssh/hosts.json`，密码、私钥与口令存入系统钥匙串（Windows 凭据管理器 / macOS Keychain / Linux Secret Service），配置文件中不落明文凭据。私钥体积超单条凭据上限时自动分片存储。

### 2. 💻 SSH 现代终端 (Terminal)
* **多会话标签页 (Tab Bar)**：支持多会话并行切换、动态新建终端、独立关闭会话。
* **真实 PTY 交互通道**：Rust 后端通过 libssh2 分配真实 `xterm-256color` PTY 并启动 Shell，后台线程实时通过 Tauri 事件向前端广播 stdout/stderr，支持双向交互输入与窗口尺寸 Resize。
* **常用指令代码片段 (Snippets)**：一键向终端发送常用命令（`docker ps`, `free -h`, `df -h`, `systemctl status nginx`, `htop` 等）。
* **终端与 SFTP 快速联动**：终端顶栏提供“打开 SFTP”按钮，一键打开对应主机的文件管理器。
* **非阻塞双向通道**：Shell 会话以 libssh2 非阻塞模式读写，读线程不再长时间持锁，输入/回显互不阻塞；输出按 UTF-8 边界增量解码，避免中文被分片截断。
* **主机密钥校验**：复用 `~/.ssh/known_hosts`，首次连接展示 SHA256 指纹由用户确认（TOFU）后写入；密钥与记录不一致时直接阻断连接，防止中间人攻击。
* **终端内搜索 (Ctrl+F)**：不用离开终端即可查找回显内容，支持上一个/下一个跳转、命中计数，以及区分大小写 / 全字匹配 / 正则三个开关。
* **终端剪贴板快捷键**：`Ctrl+Shift+C` / `Ctrl+Insert` 复制选中内容，`Ctrl+Shift+V` / `Shift+Insert` 粘贴；终端内右键菜单提供复制 / 粘贴 / 全选 / 查找 / 清除屏幕。
* **关键字高亮**：自动着色 `ERROR` / `FATAL` / `FAILED` / `WARN` / `SUCCESS` 等关键字，纯前端 marker + decoration 增量扫描（只扫新增行、每帧限流、默认跳过 alt buffer 与当前输入行），大日志滚动不卡顿；顶栏灯泡图标可随时开关（状态存 localStorage）。
* **可点击的 IP / 文件行号**：回显中的 IPv4 与 `file:line` 支持悬停高亮，点击即复制到剪贴板。
* **中日韩宽字符宽度修正**：加载 Unicode11 addon，中文环境下字符宽度不再错位。
* **命令历史 + 联想补全**：输入时在终端底部浮出本主机历史命令候选（前缀匹配优先、其次子串匹配，按使用频次与最近使用时间排序，最多 8 条），并对 `docker` / `systemctl` / `git` 提供内置子命令词表；`Tab` 接受、`↑/↓` 切换、`Esc` 关闭、鼠标点击即用。历史按 `user@host:port` 归属持久化到 `~/.snail-ssh/history.json`（每主机上限 500 条、自动去重累计频次、自动过滤含密码/令牌的敏感命令与超长命令）；首次连接可选拉取远端 `~/.bash_history` 作为词库（`localStorage` 键 `snail_ssh_import_remote_history` 置 `0` 可关闭）。
* **快捷清屏 (Ctrl+L 透传)**。
* **会话日志 (REC)**：标签右键菜单可对单个终端开/关会话日志，顶栏出现闪烁 `REC` 红点、状态栏显示「日志: 记录中」；日志写入 `~/.snail-ssh/logs/<主机目录>/<UTC 时间戳>_<会话短 ID>.log`（主机目录取自 `user@host:port`，文件名非法字符替换为 `_`；UTF-8 纯文本）。落盘走独立 writer 线程 + 有界 channel，读线程只做 `try_send`、写盘滞后时丢弃而不阻塞终端输出；落盘前对 `password` / `token` / `secret` / `api_key` / `-pXXX` 等做脱敏。菜单提供「打开日志目录」，停止记录时 Toast 给出完整路径；开关状态记 localStorage，下次新会话自动开启。

### 3. 📁 SFTP 远程文件管理器 (SFTP Explorer)
* **面包屑路径导航**：支持层级路径点击回溯、一键复制绝对路径。
* **精细化文件列表**：根据扩展名匹配专属高质感图标（Nginx、YAML、代码、压缩包、证书、日志等），显示文件大小、Linux 权限权限位（如 `drwxr-xr-x (755)`）、所属用户与组、修改时间。
* **在线文件编辑器 (Inline Editor)**：双击或点击编辑图标即可弹出代码编辑器，支持在线实时修改配置文件并一键回写同步至远程服务器（单文件读取上限 8MB，支持 Ctrl+S）。
* **文件上传与下载**：通过系统原生对话框选择本地文件/保存路径（上传支持多选），真实走 SFTP 分块传输，进度、瞬时速度与已传字节数由后端事件实时回传。
* **拖拽上传**：从系统资源管理器拖入文件/文件夹到左侧文件树（或 SFTP 列表）区域，出现「释放以上传到 /path」高亮与跟随鼠标的提示，释放后进入传输队列；支持多文件与目录递归上传（保留目录结构，枚举阶段先发 `scanning` 状态），拖到非文件区域会明确提示而非静默失败。拖放坐标按 `devicePixelRatio` 换算，高 DPI 屏命中正常。
* **目录递归下载**：目录与文件均支持下载，目录会保留远端目录结构；同名文件上传前会出现覆盖确认，避免误覆盖远端配置。
* **独立 SFTP 连接**：SFTP 复用同一主机的 SSH 凭据，但使用独立连接，避免与终端 Shell 抢同一 session 而互相干扰。
* **目录操作**：新建文件夹、新建空白文件、重命名（文件与目录，重名时先确认覆盖）、修改权限（八进制 `chmod`）、修改属主/属组（数字 `uid:gid`）、查看属性（名称/路径/大小/权限/属主/属组/修改时间）、复选框多选后批量下载（文件 + 目录）/ 批量删除、目录递归删除、显示/隐藏隐藏文件；进入 SFTP 视图时默认定位到远端登录用户的家目录。
* **路径直输**：点击面包屑右侧铅笔图标可直接粘贴绝对路径回车跳转（支持 `~` / `~/xxx` 自动展开为家目录）。
* **名称过滤**：列表上方输入框按名称即时过滤当前目录，过滤不会清空已勾选集合。
* **文件行右键菜单**：下载 / 在线编辑 / 重命名 / 修改权限 / 修改属主 / 复制路径 / 属性 / 删除。
* **操作二次确认**：删除文件/目录、删除服务器、上传覆盖、放弃未保存修改等均使用应用内弹窗确认（不再依赖各平台行为不一致的原生 `window.confirm` / `window.prompt`）。

### 4. 🚀 传输队列中心 (Transfer Manager)
* 右下角悬浮动态抽屉，实时显示上传与下载任务进度条、当前瞬时速度 (如 `18.4 MB/s`)、剩余预计时间与完成状态。
* 任务状态（进行中 / 已完成 / 失败）由后端进度事件驱动，可单条移除或一键清理已结束记录；传输完成后 SFTP 目录会自动刷新，上传结果立刻可见。

### 5. ⚡ 全局效率工具 (Productivity)
* **<kbd>Ctrl+K</kbd> / <kbd>⌘K</kbd> 全局命令面板**：仿 Raycast 设计，任何界面均可呼出，模糊搜索服务器秒级直连；输入为空时优先展示「最近连接」。
* **快速连接**：`Ctrl+Shift+N` 呼出（命令面板中也有入口），临时填写 host/port/user/认证方式直接开会话，**不会写入 `~/.snail-ssh/hosts.json`**，密码/私钥仅保存在内存中。
* **最近连接**：侧边栏「快捷入口」与命令面板按最近使用时间倒序展示真实最近连接（最多 5 条），主机被删除后自动从列表过滤。
* **快捷键支持**：
  * <kbd>Ctrl+K</kbd>：唤出全局搜索面板
  * <kbd>Ctrl+N</kbd>：快速打开新建服务器弹窗
  * <kbd>Ctrl+Shift+N</kbd>：快速连接（临时会话）
  * <kbd>Ctrl+T</kbd>：展开/折叠传输任务抽屉
  * <kbd>Ctrl+F</kbd>：终端内查找（终端聚焦时）
  * <kbd>ESC</kbd>：退出弹窗 / 关闭终端搜索条

---

## ⚠️ 尚未实现 / 已知限制

界面中以下入口为占位，点击仅提示「尚未实现」，请勿当作可用功能：

* 跳板机 & 代理（Jump Host / ProxyJump）
* SSH 密钥保险箱（密钥对生成与导入）
* 命令片段库（团队共享）
* 端口转发 & 隧道（Local / Remote / Dynamic）
* 主机配置导入 / 导出（SSH Config、FinalShell、Termius）
* 设置中心（终端主题、字体定制）

其它限制：

* 远端探测脚本依赖 Linux 常见命令（`/etc/os-release`、`free`、`df`、`/proc/stat`），非 Linux 主机的 CPU/内存指标可能采集不到。
* 传输任务暂不支持取消 / 暂停 / 断点续传，任务失败后需重新发起。
* 系统钥匙串不可用时会回退为明文存入 `hosts.json`，并在日志中告警。
* 最近连接列表保存在浏览器 `localStorage`（key：`snail_ssh_recent`），不落 `~/.snail-ssh/`，清空站点数据后会丢失。
* 快速连接为临时会话：不会保存到资产列表，凭据只存在于内存中，应用重启后需重新填写；同名 `hostId` 不会与已入库主机去重。
* SFTP 列表中的属主/属组显示的是远端原始**数字 uid/gid**，不是用户名（`chown` 同样接受数字 id）。
* 关键字高亮为纯前端渲染层效果，只改变显示、不改写收到的数据；IP / `file:line` 点击行为是复制到本地剪贴板，不会在远端执行任何操作。
* 命令补全条仅在可见时接管 `Tab` / `↑` / `↓` / `Esc`，不可见时这些按键完全透传给远端（`vim` / `less` / `man` 均不受影响）；因此当输入行已匹配到候选时，`↑/↓` 用于切换候选而非翻远端 shell 历史。输入行缓冲不跟踪光标左右移动，仅按行尾追加/退格模型工作。
* 命令历史按 `user@host:port` 归属（临时会话、重新导入资产后历史不丢）；远端 `~/.bash_history` 导入为可选且默认开启，zsh / fish 等未写入该文件时不会自动获得初始词库，且导入只插入不存在的项、不累加频次。
* 会话日志默认关闭（避免磁盘悄悄写满）；开启后由 localStorage 记忆，之后新建会话会自动进入记录状态。
* 会话日志为**明文**且**不自动轮转 / 清理**，请自行保管与清理；虽然落盘前对常见凭据字段脱敏，但脱敏是启发式、按输出分片进行，跨分片的密码或非常规写法仍可能漏网，且脱敏后的日志内容会与终端屏幕显示不完全一致。
* 含 `\0` 的二进制输出片段整段跳过不写日志；写入缓冲跟不上时按 `try_send` 丢弃部分内容（stderr 有一次告警）。日志文件名中的时间戳为 UTC。
* 「打开日志目录」仅允许打开 `~/.snail-ssh/` 之内的路径（后端会校验），无法被前端任意传参打开其它目录。
* 拖拽上传只支持「从系统资源管理器拖入」，**不支持把远端文件拖出到资源管理器**（Tauri 无此能力）；落点仅识别左侧文件树 / SFTP 列表区域，拖到终端或标题栏会提示。
* 拖放由 Tauri webview 的 `dragDropEnabled` 接管（默认开启），HTML5 拖放事件会被吞掉；将来若要做「标签页拖拽排序 / 树节点拖拽移动」需与它协调（已在代码注释记风险）。
* 拖拽上传与点击上传沿用同一套上传语义（同名直接覆盖，不做冲突确认）；多文件/目录会逐条入队并顺序传输，不并发混用同一条 SFTP 会话。

---

## 📂 项目结构

```text
snail-ssh/
├── prototype/              # 第一版纯静态高保真交互原型备份 (可直接浏览器双击预览)
│   ├── index.html
│   ├── style.css
│   └── app.js
├── src-tauri/              # Rust 原生后端工程 (Tauri v2)
│   ├── Cargo.toml          # Rust 依赖声明 (tauri 2.0, ssh2, tokio, serde 等)
│   ├── tauri.conf.json     # Tauri 跨平台窗口与应用配置
│   ├── capabilities/       # Tauri v2 权限策略配置
│   ├── icons/              # 跨平台标准应用图标资产 (ICO, ICNS, PNG)
│   └── src/
│       ├── main.rs         # 桌面客户端可执行入口
│       ├── lib.rs          # Tauri 命令调度与状态注入
│       ├── ssh.rs          # SSH 核心：TCP握手、认证、PTY申请、Shell双向读写流
│       ├── sftp.rs         # SFTP 核心：目录遍历、文件读写、分块流式上传下载
│       ├── history.rs      # 命令历史持久化（去重累计、频次裁剪、敏感命令过滤）
│       └── storage.rs      # 主机资产配置本地持久化管理器
├── src/                    # 前端工程 (Vue 3 + TypeScript + TailwindCSS)
│   ├── api/
│   │   └── tauri.ts        # 前后端双模桥接层 (支持 Tauri 原生调用与降级 Mock)
│   ├── components/
│   │   ├── TitleBar.vue        # 现代化窗口标题栏
│   │   ├── Sidebar.vue         # 侧边栏与分组筛选
│   │   ├── HostManager.vue     # 服务器资产卡片/列表管理器
│   │   ├── TerminalView.vue    # xterm.js 终端会话组件
│   │   ├── SftpView.vue        # SFTP 文件浏览器组件
│   │   ├── TransferDrawer.vue  # 悬浮传输任务抽屉
│   │   ├── CommandPalette.vue  # Ctrl+K 全局命令面板
│   │   ├── HostModal.vue       # 新建/编辑主机弹窗
│   │   ├── EditorModal.vue     # 在线代码/文件编辑器弹窗
│   │   └── Toast.vue           # 全局微动效提示框
│   ├── composables/
│   │   ├── useDialog.ts            # 应用内确认/输入弹窗
│   │   ├── useRecentHosts.ts       # 最近连接（localStorage）
│   │   └── useTerminalHighlight.ts # 终端关键字高亮与可点击链接
│   ├── types/
│   │   └── index.ts        # 全局 TypeScript 接口定义
│   ├── App.vue             # 主界面总线
│   ├── main.ts             # 应用入口
│   └── style.css           # 全局样式系统
├── index.html              # Vite 单页应用宿主
├── vite.config.ts          # Vite 打包配置
├── tailwind.config.js      # Tailwind CSS 主题配置
└── package.json            # 前端依赖配置
```

---

## 🛠️ 本地运行与构建

### 1. 前端快速开发预览（纯 Web 调试模式）
```bash
# 安装依赖
pnpm install

# 启动 Web 前端预览（默认端口 1420）
pnpm dev
```

### 2. 启动 Tauri 桌面客户端（原生窗口）
确保本地已安装 Rust 开发环境：
```bash
# 启动 Tauri 开发模式（自动编译 Rust 后端并热载前端）
pnpm tauri dev
```

### 3. 打包跨平台桌面安装包（Release）
```bash
# 一键编译并生成平台原生安装包 (.msi / .exe / .dmg / .deb / .AppImage)
pnpm tauri build
```

---

## 💡 原型快速查看
如果仅需快速预览 UI 交互效果，也可以进入 `prototype/` 目录，直接双击 **`prototype/index.html`** 在任意浏览器打开体验。
