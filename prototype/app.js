/**
 * SnailSSH - 现代化跨平台 SSH & SFTP 终端套件
 * 交互原型逻辑引擎
 */

// ==========================================
// 1. 模拟数据状态源 (State Store)
// ==========================================
const AppState = {
  activeView: 'hosts', // 'hosts' | 'terminal' | 'sftp'
  activeGroup: 'all',
  hostLayout: 'grid', // 'grid' | 'list'
  searchQuery: '',
  showHiddenFiles: false,

  // 服务器资产数据
  hosts: [
    {
      id: 'prod-k8s-master-01',
      name: 'prod-k8s-master-01',
      group: 'production',
      os: 'ubuntu',
      osName: 'Ubuntu 22.04 LTS',
      host: '47.98.112.24',
      port: 22,
      user: 'root',
      auth: 'Key (id_rsa)',
      status: 'online',
      ping: '24ms',
      cpu: 28,
      mem: 64,
      disk: 42,
      tags: ['k8s', 'master', 'aliyun-hz'],
      updatedAt: '2分钟前'
    },
    {
      id: 'prod-k8s-node-02',
      name: 'prod-k8s-node-02',
      group: 'production',
      os: 'ubuntu',
      osName: 'Ubuntu 22.04 LTS',
      host: '47.98.112.25',
      port: 22,
      user: 'root',
      auth: 'Key (id_rsa)',
      status: 'online',
      ping: '26ms',
      cpu: 56,
      mem: 81,
      disk: 65,
      tags: ['k8s', 'worker', 'docker'],
      updatedAt: '1分钟前'
    },
    {
      id: 'aliyun-hk-edge',
      name: 'aliyun-hk-edge',
      group: 'staging',
      os: 'debian',
      osName: 'Debian 12 (Bookworm)',
      host: '43.154.201.89',
      port: 2222,
      user: 'deploy',
      auth: 'Password',
      status: 'online',
      ping: '48ms',
      cpu: 14,
      mem: 38,
      disk: 29,
      tags: ['nginx', 'gateway', 'hk-node'],
      updatedAt: '刚刚'
    },
    {
      id: 'staging-api-service',
      name: 'staging-api-service',
      group: 'staging',
      os: 'centos',
      osName: 'Rocky Linux 9.2',
      host: '118.24.89.14',
      port: 22,
      user: 'admin',
      auth: 'Key (id_ed25519)',
      status: 'online',
      ping: '32ms',
      cpu: 19,
      mem: 52,
      disk: 38,
      tags: ['golang', 'redis', 'staging'],
      updatedAt: '5分钟前'
    },
    {
      id: 'vps-us-west-bench',
      name: 'vps-us-west-bench',
      group: 'cloud',
      os: 'alpine',
      osName: 'Alpine Linux 3.19',
      host: '142.171.8.60',
      port: 22,
      user: 'root',
      auth: 'Key (id_ed25519)',
      status: 'online',
      ping: '145ms',
      cpu: 8,
      mem: 24,
      disk: 18,
      tags: ['vps', 'bench', 'us-west'],
      updatedAt: '12分钟前'
    },
    {
      id: 'home-lab-macmini',
      name: 'home-lab-macmini',
      group: 'cloud',
      os: 'apple',
      osName: 'macOS Sonoma 14.3',
      host: '192.168.31.80',
      port: 22,
      user: 'chong',
      auth: 'SSH Agent',
      status: 'online',
      ping: '2ms',
      cpu: 11,
      mem: 45,
      disk: 52,
      tags: ['apple-silicon', 'lan', 'm2-pro'],
      updatedAt: '刚刚'
    }
  ],

  // 终端会话
  currentTerminalTabIndex: 0,
  terminalTabs: [
    {
      id: 0,
      hostId: 'prod-k8s-master-01',
      title: 'prod-k8s-master-01',
      sub: 'root@47.98.112.24',
      history: []
    },
    {
      id: 1,
      hostId: 'aliyun-hk-edge',
      title: 'aliyun-hk-edge',
      sub: 'deploy@43.154.201.89',
      history: []
    }
  ],

  // SFTP 模拟文件系统结构
  sftp: {
    currentHostId: 'prod-k8s-master-01',
    currentPath: '/etc/nginx',
    files: [
      { name: '..', isDir: true, size: '-', type: '上级目录', chmod: '755', owner: 'root:root', time: '2025-02-18 10:24' },
      { name: 'conf.d', isDir: true, size: '4.0 KB', type: '目录', chmod: 'drwxr-xr-x (755)', owner: 'root:root', time: '2025-02-19 14:32' },
      { name: 'sites-available', isDir: true, size: '4.0 KB', type: '目录', chmod: 'drwxr-xr-x (755)', owner: 'root:root', time: '2025-02-10 09:15' },
      { name: 'sites-enabled', isDir: true, size: '4.0 KB', type: '符号链接', chmod: 'lrwxrwxrwx (777)', owner: 'root:root', time: '2025-02-10 09:16' },
      { name: 'snippets', isDir: true, size: '4.0 KB', type: '目录', chmod: 'drwxr-xr-x (755)', owner: 'root:root', time: '2025-01-05 18:00' },
      { name: 'nginx.conf', isDir: false, size: '2.8 KB', type: 'Nginx 配置文件', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2025-02-20 16:45', editable: true },
      { name: 'mime.types', isDir: false, size: '5.2 KB', type: '配置文件', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2024-11-20 03:12', editable: true },
      { name: 'proxy_params', isDir: false, size: '1.1 KB', type: '配置文件', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2025-01-14 11:20', editable: true },
      { name: 'fastcgi_params', isDir: false, size: '1.4 KB', type: '配置文件', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2024-11-20 03:12', editable: true },
      { name: 'app-upstream.yaml', isDir: false, size: '890 B', type: 'YAML 数据', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2025-02-19 22:10', editable: true },
      { name: 'ssl-bundle.crt', isDir: false, size: '3.6 KB', type: 'SSL 证书', chmod: '-rw------- (600)', owner: 'root:root', time: '2025-01-01 00:00', editable: false },
      { name: 'ssl-private.key', isDir: false, size: '1.7 KB', type: 'SSL 私钥', chmod: '-rw------- (600)', owner: 'root:root', time: '2025-01-01 00:00', editable: false },
      { name: 'backup-20250218.tar.gz', isDir: false, size: '42.5 MB', type: 'Gzip 压缩档', chmod: '-rw-r--r-- (644)', owner: 'root:root', time: '2025-02-18 03:00', editable: false },
      { name: '.env.local', isDir: false, size: '420 B', type: '环境变量 (隐藏)', chmod: '-rw------- (600)', owner: 'root:root', time: '2025-02-12 18:22', isHidden: true, editable: true }
    ]
  }
};

// 预设的代码编辑器内容（模拟远程查看 nginx.conf）
const SampleCode = `# ==============================================
# SnailSSH 远程实时配置文件编辑器
# Path: /etc/nginx/nginx.conf
# ==============================================

user www-data;
worker_processes auto;
pid /run/nginx.pid;
include /etc/nginx/modules-enabled/*.conf;

events {
    worker_connections 2048;
    multi_accept on;
    use epoll;
}

http {
    ##
    # 基础性能优化与网络传输
    ##
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;
    server_tokens off;

    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    ##
    # SSL/TLS 现代加密套件
    ##
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;

    ##
    # 日志输出格式
    ##
    access_log /var/log/nginx/access.log;
    error_log /var/log/nginx/error.log warn;

    ##
    # Gzip 压缩配置
    ##
    gzip on;
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml;

    include /etc/nginx/conf.d/*.conf;
    include /etc/nginx/sites-enabled/*;
}
`;

// ==========================================
// 2. 核心视图切换逻辑
// ==========================================
function switchView(viewName) {
  AppState.activeView = viewName;

  // 面板显示隐藏
  document.querySelectorAll('.view-panel').forEach(panel => {
    panel.classList.add('hidden');
    panel.classList.remove('flex');
  });

  const targetPanel = document.getElementById(`view-${viewName}`);
  if (targetPanel) {
    targetPanel.classList.remove('hidden');
    targetPanel.classList.add('flex');
  }

  // 侧边栏按钮高亮
  document.querySelectorAll('.nav-item').forEach(btn => btn.classList.remove('active'));
  const targetNav = document.getElementById(`nav-${viewName}`);
  if (targetNav) targetNav.classList.add('active');

  // 如果切换到终端，让终端打字获得焦点
  if (viewName === 'terminal') {
    setTimeout(() => {
      const input = document.getElementById('term-active-input');
      if (input) input.focus();
    }, 100);
  }

  // 重新渲染图标
  if (window.lucide) lucide.createIcons();
}

// 分组过滤
function filterGroup(groupName, elem) {
  AppState.activeGroup = groupName;
  document.querySelectorAll('.group-filter-btn').forEach(btn => btn.classList.remove('active'));
  if (elem) elem.classList.add('active');
  renderHosts();
}

// 布局切换 (网格卡片 / 紧凑列表)
function setHostLayout(layout) {
  AppState.hostLayout = layout;
  const btnGrid = document.getElementById('btn-layout-grid');
  const btnList = document.getElementById('btn-layout-list');

  if (layout === 'grid') {
    btnGrid.classList.add('bg-surface-elevated', 'text-white');
    btnGrid.classList.remove('text-surface-subtle');
    btnList.classList.remove('bg-surface-elevated', 'text-white');
    btnList.classList.add('text-surface-subtle');
  } else {
    btnList.classList.add('bg-surface-elevated', 'text-white');
    btnList.classList.remove('text-surface-subtle');
    btnGrid.classList.remove('bg-surface-elevated', 'text-white');
    btnGrid.classList.add('text-surface-subtle');
  }

  renderHosts();
}

// 搜索主机过滤
function handleHostSearch(val) {
  AppState.searchQuery = val.trim().toLowerCase();
  renderHosts();
}

// ==========================================
// 3. 服务器列表渲染 (Hosts View)
// ==========================================
function renderHosts() {
  const container = document.getElementById('hosts-grid');
  if (!container) return;

  const filtered = AppState.hosts.filter(h => {
    const matchGroup = AppState.activeGroup === 'all' || h.group === AppState.activeGroup;
    const matchQuery = !AppState.searchQuery || 
      h.name.toLowerCase().includes(AppState.searchQuery) ||
      h.host.toLowerCase().includes(AppState.searchQuery) ||
      h.tags.some(t => t.toLowerCase().includes(AppState.searchQuery));
    return matchGroup && matchQuery;
  });

  const countBadge = document.getElementById('host-count-badge');
  if (countBadge) countBadge.textContent = `共 ${filtered.length} 台主机`;

  if (filtered.length === 0) {
    container.className = "flex flex-col items-center justify-center p-12 text-center";
    container.innerHTML = `
      <div class="w-14 h-14 rounded-2xl bg-surface-card border border-surface-border flex items-center justify-center text-slate-500 mb-3">
        <i data-lucide="server-off" class="w-7 h-7"></i>
      </div>
      <div class="text-sm font-semibold text-slate-300">未找到匹配的服务器</div>
      <div class="text-xs text-slate-500 mt-1">请尝试更换搜索关键字或清除筛选条件</div>
      <button onclick="clearSearch()" class="mt-4 px-3.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-brand-400">重置搜索</button>
    `;
    if (window.lucide) lucide.createIcons();
    return;
  }

  if (AppState.hostLayout === 'grid') {
    container.className = "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4";
    container.innerHTML = filtered.map(host => {
      // 操作系统小图标与颜色
      const osIcon = getOsIcon(host.os);
      return `
        <div class="host-card rounded-xl border border-surface-border p-4 flex flex-col justify-between group shadow-glow-card">
          <!-- 卡片头部：状态、名称、OS -->
          <div>
            <div class="flex items-start justify-between mb-2.5">
              <div class="flex items-center gap-2.5">
                <div class="w-8 h-8 rounded-lg bg-surface-card border border-surface-border/80 flex items-center justify-center text-brand-400 group-hover:border-brand-500/30 transition-colors">
                  ${osIcon}
                </div>
                <div>
                  <div class="text-xs font-bold text-white flex items-center gap-1.5 font-mono">
                    <span>${host.name}</span>
                    <span class="w-2 h-2 rounded-full bg-emerald-400" title="在线 (${host.ping})"></span>
                  </div>
                  <div class="text-[11px] font-mono text-surface-subtle flex items-center gap-1 mt-0.5">
                    <span>${host.user}@${host.host}:${host.port}</span>
                  </div>
                </div>
              </div>

              <!-- 右侧菜单按钮 -->
              <div class="relative">
                <button onclick="openHostMenu('${host.id}', event)" class="p-1 rounded text-surface-subtle hover:text-white hover:bg-surface-highlight transition-colors">
                  <i data-lucide="more-vertical" class="w-4 h-4"></i>
                </button>
              </div>
            </div>

            <!-- 系统与硬件微型指标 -->
            <div class="grid grid-cols-3 gap-2 my-3 p-2 rounded-lg bg-surface-base/60 border border-surface-border/50 text-[11px] font-mono">
              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">CPU</span>
                <div class="flex items-center gap-1.5 mt-0.5">
                  <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
                    <div class="bg-brand-500 h-full rounded-full" style="width: ${host.cpu}%"></div>
                  </div>
                  <span class="text-slate-300 font-semibold text-[10px]">${host.cpu}%</span>
                </div>
              </div>

              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">MEM</span>
                <div class="flex items-center gap-1.5 mt-0.5">
                  <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
                    <div class="bg-cyan-500 h-full rounded-full" style="width: ${host.mem}%"></div>
                  </div>
                  <span class="text-slate-300 font-semibold text-[10px]">${host.mem}%</span>
                </div>
              </div>

              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">延迟 (Ping)</span>
                <div class="flex items-center gap-1 mt-0.5">
                  <span class="text-emerald-400 font-semibold text-[11px]">${host.ping}</span>
                </div>
              </div>
            </div>

            <!-- 标签列表 -->
            <div class="flex flex-wrap gap-1.5 mb-4">
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-highlight text-slate-300 border border-surface-border/60">${host.osName}</span>
              ${host.tags.map(t => `<span class="text-[10px] font-mono px-2 py-0.5 rounded bg-brand-500/10 text-brand-300 border border-brand-500/20">#${t}</span>`).join('')}
            </div>
          </div>

          <!-- 卡片底部直连动作栏 -->
          <div class="pt-3 border-t border-surface-border/60 flex items-center justify-between text-xs">
            <span class="text-[10px] font-mono text-slate-500">${host.auth}</span>
            <div class="flex items-center gap-2">
              <button onclick="connectSftp('${host.id}')" class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card hover:bg-surface-elevated border border-surface-border hover:border-cyan-500/40 text-cyan-400 text-xs transition-all">
                <i data-lucide="folder-git-2" class="w-3.5 h-3.5"></i>
                <span>SFTP</span>
              </button>
              <button onclick="connectHost('${host.id}')" class="flex items-center gap-1.5 px-3 py-1 rounded-md bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald">
                <i data-lucide="terminal" class="w-3.5 h-3.5"></i>
                <span>连接 SSH</span>
              </button>
            </div>
          </div>
        </div>
      `;
    }).join('');
  } else {
    // 列表布局
    container.className = "flex flex-col gap-2";
    container.innerHTML = `
      <div class="bg-surface-card/60 rounded-xl border border-surface-border overflow-hidden">
        <table class="w-full text-left text-xs border-collapse">
          <thead class="bg-surface-sidebar border-b border-surface-border text-[11px] font-mono text-slate-400 uppercase">
            <tr>
              <th class="py-2.5 px-4 font-medium">服务器 / 别名</th>
              <th class="py-2.5 px-4 font-medium">系统</th>
              <th class="py-2.5 px-4 font-medium">IP & 端口</th>
              <th class="py-2.5 px-4 font-medium">认证方式</th>
              <th class="py-2.5 px-4 font-medium text-center">延迟</th>
              <th class="py-2.5 px-4 font-medium">标签</th>
              <th class="py-2.5 px-4 font-medium text-right">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-surface-border/40 font-mono">
            ${filtered.map(host => `
              <tr class="hover:bg-surface-card/50 transition-colors">
                <td class="py-3 px-4 font-medium text-white flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
                  <span class="font-bold">${host.name}</span>
                </td>
                <td class="py-3 px-4 text-slate-300 text-[11px]">${host.osName}</td>
                <td class="py-3 px-4 text-slate-400">${host.user}@${host.host}:${host.port}</td>
                <td class="py-3 px-4 text-slate-400">${host.auth}</td>
                <td class="py-3 px-4 text-center text-emerald-400 font-semibold">${host.ping}</td>
                <td class="py-3 px-4">
                  <div class="flex gap-1">
                    ${host.tags.map(t => `<span class="text-[9px] px-1.5 py-0.2 rounded bg-brand-500/10 text-brand-300 border border-brand-500/20">${t}</span>`).join('')}
                  </div>
                </td>
                <td class="py-3 px-4 text-right">
                  <div class="flex items-center justify-end gap-2">
                    <button onclick="connectSftp('${host.id}')" class="px-2 py-1 rounded bg-surface-card hover:bg-surface-elevated border border-surface-border text-cyan-400 text-xs">SFTP</button>
                    <button onclick="connectHost('${host.id}')" class="px-2.5 py-1 rounded bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs">连接</button>
                  </div>
                </td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    `;
  }

  if (window.lucide) lucide.createIcons();
}

function clearSearch() {
  AppState.searchQuery = '';
  const searchInput = document.getElementById('host-search-input');
  if (searchInput) searchInput.value = '';
  renderHosts();
}

function getOsIcon(os) {
  switch (os) {
    case 'ubuntu':
    case 'debian':
      return `<i data-lucide="cpu" class="w-4 h-4 text-amber-400"></i>`;
    case 'centos':
      return `<i data-lucide="box" class="w-4 h-4 text-blue-400"></i>`;
    case 'alpine':
      return `<i data-lucide="hard-drive" class="w-4 h-4 text-cyan-400"></i>`;
    case 'apple':
      return `<i data-lucide="command" class="w-4 h-4 text-slate-200"></i>`;
    default:
      return `<i data-lucide="server" class="w-4 h-4 text-brand-400"></i>`;
  }
}

function openHostMenu(hostId, e) {
  e.stopPropagation();
  showToast(`针对主机 ${hostId} 的操作菜单：支持复制连接串、克隆配置、导出凭证`, 'info');
}

// ==========================================
// 4. SSH 终端核心模拟器 (Terminal Engine)
// ==========================================
function connectHost(hostId) {
  const host = AppState.hosts.find(h => h.id === hostId);
  if (!host) return;

  switchView('terminal');
  showToast(`已成功建立 SSH 连接到 ${host.name} (${host.host})`, 'success');

  // 如果已经存在对应 tab，则激活
  let existingIndex = AppState.terminalTabs.findIndex(t => t.hostId === hostId);
  if (existingIndex >= 0) {
    switchTerminalTab(existingIndex);
  } else {
    // 新建 tab
    const newTab = {
      id: AppState.terminalTabs.length,
      hostId: host.id,
      title: host.name,
      sub: `${host.user}@${host.host}`,
      history: []
    };
    AppState.terminalTabs.push(newTab);
    renderTerminalTabs();
    switchTerminalTab(AppState.terminalTabs.length - 1);
  }
}

function renderTerminalTabs() {
  const container = document.getElementById('terminal-tabs-container');
  if (!container) return;

  container.innerHTML = AppState.terminalTabs.map((tab, idx) => {
    const isActive = idx === AppState.currentTerminalTabIndex;
    return `
      <div onclick="switchTerminalTab(${idx})" class="terminal-tab ${isActive ? 'active bg-surface-base border-t-2 border-brand-500 text-slate-200 border-x border-surface-border/50' : 'bg-surface-card/40 hover:bg-surface-card text-surface-subtle hover:text-slate-300 border-t-2 border-transparent'} group flex items-center gap-2 px-3 py-1.5 rounded-t-lg text-xs cursor-pointer transition-all">
        <span class="w-2 h-2 rounded-full ${isActive ? 'bg-emerald-400 animate-pulse' : 'bg-slate-500'}"></span>
        <span class="font-mono font-medium">${tab.title}</span>
        <span class="text-[10px] font-mono text-slate-500 hidden sm:inline">${tab.sub}</span>
        <button onclick="event.stopPropagation(); closeTerminalTab(${idx})" class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-all ml-1">
          <i data-lucide="x" class="w-3 h-3"></i>
        </button>
      </div>
    `;
  }).join('') + `
    <button onclick="addNewTerminalTab()" class="p-1 rounded-md text-surface-subtle hover:text-white hover:bg-surface-card transition-colors ml-1" title="打开新终端">
      <i data-lucide="plus" class="w-4 h-4"></i>
    </button>
  `;

  if (window.lucide) lucide.createIcons();
}

function switchTerminalTab(index) {
  if (index < 0 || index >= AppState.terminalTabs.length) return;
  AppState.currentTerminalTabIndex = index;
  renderTerminalTabs();
  renderTerminalContent();
}

function closeTerminalTab(index) {
  if (AppState.terminalTabs.length <= 1) {
    showToast('无法关闭最后一个终端会话', 'warning');
    return;
  }
  AppState.terminalTabs.splice(index, 1);
  if (AppState.currentTerminalTabIndex >= AppState.terminalTabs.length) {
    AppState.currentTerminalTabIndex = AppState.terminalTabs.length - 1;
  }
  renderTerminalTabs();
  renderTerminalContent();
}

function addNewTerminalTab() {
  const host = AppState.hosts[0];
  const newTab = {
    id: Date.now(),
    hostId: host.id,
    title: host.name,
    sub: `${host.user}@${host.host}`,
    history: []
  };
  AppState.terminalTabs.push(newTab);
  switchTerminalTab(AppState.terminalTabs.length - 1);
  showToast(`已开启新的 SSH 会话: ${host.name}`, 'info');
}

// 终端初始欢迎界面
const terminalInitialMotd = `
<div class="text-emerald-400 font-bold mb-2">
   _____             _ _   _____ _____ _    _ 
  / ____|           (_) | / ____/ ____| |  | |
 | (___  _ __   __ _ _| || (___| (___ | |__| |
  \\___ \\| '_ \\ / _\` | | | \\___ \\\\___ \\|  __  |
  ____) | | | | (_| | | | ____) |___) | |  | |
 |_____/|_| |_|\\__,_|_|_||_____/_____/|_|  |_|
</div>
<div class="text-slate-400 mb-2">Welcome to <span class="text-brand-400 font-semibold">Ubuntu 22.04.3 LTS (GNU/Linux 5.15.0-91-generic x86_64)</span></div>
<div class="text-slate-500 mb-3 border-l-2 border-brand-500 pl-3">
 * Documentation:  https://help.ubuntu.com<br>
 * Management:     https://landscape.canonical.com<br>
 * Support:        https://ubuntu.com/advantage
</div>
<div class="text-slate-300 mb-3">
 System load:  <span class="text-emerald-400">0.24, 0.18, 0.12</span>   Processes:     <span class="text-emerald-400">142</span><br>
 Usage of /:   <span class="text-cyan-400">42.8% of 98.42GB</span>        Users logged in: <span class="text-emerald-400">1</span><br>
 Memory usage: <span class="text-amber-400">64%</span>                     IP address:    <span class="text-brand-300">47.98.112.24</span>
</div>
<div class="text-slate-400 mb-4">Last login: Fri Feb 20 18:24:12 2025 from 123.120.45.67 via SSH</div>
`;

function renderTerminalContent() {
  const screen = document.getElementById('terminal-screen');
  if (!screen) return;

  const currentTab = AppState.terminalTabs[AppState.currentTerminalTabIndex];
  const host = AppState.hosts.find(h => h.id === currentTab.hostId) || AppState.hosts[0];

  // 拼接历史输出
  let historyHtml = '';
  if (!currentTab.history || currentTab.history.length === 0) {
    historyHtml = terminalInitialMotd;
  } else {
    historyHtml = currentTab.history.map(item => {
      return `
        <div class="mb-2">
          <div class="flex items-center gap-1.5 text-slate-300">
            <span class="text-emerald-400 font-bold">${host.user}@${host.name}</span>:<span class="text-cyan-400 font-semibold">~</span><span class="text-brand-400">#</span>
            <span class="text-white">${item.cmd}</span>
          </div>
          <div class="text-slate-300 mt-1 pl-1">${item.output}</div>
        </div>
      `;
    }).join('');
  }

  // 当前活动命令行
  const activePromptHtml = `
    <div class="flex items-center gap-1.5 text-slate-300 mt-2" id="term-active-line">
      <span class="text-emerald-400 font-bold">${host.user}@${host.name}</span>:<span class="text-cyan-400 font-semibold">~</span><span class="text-brand-400">#</span>
      <input type="text" id="term-active-input" onkeydown="handleTerminalKey(event)" class="bg-transparent text-white focus:outline-none flex-1 font-mono text-[13px] border-none p-0 caret-emerald-400" autocomplete="off" autofocus spellcheck="false" placeholder="可键入指令，如 help, ls, docker ps, free, neofetch...">
    </div>
  `;

  screen.innerHTML = historyHtml + activePromptHtml;
  screen.scrollTop = screen.scrollHeight;

  // 聚焦输入框
  setTimeout(() => {
    const input = document.getElementById('term-active-input');
    if (input) input.focus();
  }, 50);
}

function handleTerminalKey(e) {
  if (e.key === 'Enter') {
    const input = e.target;
    const cmd = input.value.trim();
    if (!cmd) return;

    executeTerminalCommand(cmd);
  }
}

function executeTerminalCommand(cmd) {
  const currentTab = AppState.terminalTabs[AppState.currentTerminalTabIndex];
  if (!currentTab.history) currentTab.history = [];

  // 清屏指令
  if (cmd === 'clear') {
    currentTab.history = [];
    renderTerminalContent();
    return;
  }

  // 模拟指令执行应答
  let output = '';
  const lowerCmd = cmd.toLowerCase();

  if (lowerCmd === 'help') {
    output = `
      <div class="text-brand-400 font-semibold mb-1">SnailSSH 交互式终端模拟器命令指南:</div>
      <div class="grid grid-cols-2 gap-2 text-slate-300 text-xs font-mono">
        <div><span class="text-cyan-400 font-bold">ls / ls -la</span> - 查看当前目录文件</div>
        <div><span class="text-cyan-400 font-bold">docker ps</span> - 查看运行中的 Docker 容器</div>
        <div><span class="text-cyan-400 font-bold">free -h</span> - 显示系统内存使用量</div>
        <div><span class="text-cyan-400 font-bold">df -h</span> - 显示磁盘文件系统使用量</div>
        <div><span class="text-cyan-400 font-bold">neofetch</span> - 打印服务器软硬件配置彩图</div>
        <div><span class="text-cyan-400 font-bold">sftp</span> - 一键跳转并打开该主机的 SFTP 管理</div>
        <div><span class="text-cyan-400 font-bold">clear</span> - 清空终端屏幕 (Ctrl+L)</div>
      </div>
    `;
  } else if (lowerCmd.startsWith('ls')) {
    output = `
      <div class="text-slate-300 font-mono">
        <span class="text-cyan-400 font-bold">app/</span>&nbsp;&nbsp;&nbsp;&nbsp;
        <span class="text-cyan-400 font-bold">docker/</span>&nbsp;&nbsp;&nbsp;&nbsp;
        <span class="text-cyan-400 font-bold">logs/</span>&nbsp;&nbsp;&nbsp;&nbsp;
        <span class="text-emerald-400">deploy.sh*</span>&nbsp;&nbsp;&nbsp;&nbsp;
        <span class="text-white">docker-compose.yml</span>&nbsp;&nbsp;&nbsp;&nbsp;
        <span class="text-amber-400">backup.tar.gz</span>
      </div>
    `;
  } else if (lowerCmd === 'docker ps') {
    output = `
      <div class="text-slate-300 font-mono overflow-x-auto text-xs leading-5">
        <div class="text-slate-500 font-semibold">CONTAINER ID   IMAGE                 COMMAND                  CREATED         STATUS         PORTS                    NAMES</div>
        <div><span class="text-emerald-400">a9c812d4ef01</span>   nginx:alpine          "/docker-entrypoint.…"   2 hours ago     Up 2 hours     0.0.0.0:80->80/tcp       web-gateway</div>
        <div><span class="text-emerald-400">f4b008e1a7b2</span>   redis:7.2-alpine      "docker-entrypoint.s…"   3 days ago      Up 3 days      0.0.0.0:6379->6379/tcp   redis-cluster</div>
        <div><span class="text-emerald-400">5e1289cf3019</span>   node:20-alpine        "node dist/server.js"    5 hours ago     Up 5 hours     0.0.0.0:3000->3000/tcp   snail-api-service</div>
        <div><span class="text-emerald-400">77da19ef01a8</span>   postgres:16           "docker-entrypoint.s…"   10 days ago     Up 10 days     0.0.0.0:5432->5432/tcp   db-postgres-prod</div>
      </div>
    `;
  } else if (lowerCmd.includes('free')) {
    output = `
      <div class="text-slate-300 font-mono leading-5">
        &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;total&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;used&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;free&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;shared&nbsp;&nbsp;buff/cache&nbsp;&nbsp;&nbsp;available<br>
        Mem:&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;15Gi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;9.6Gi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;1.2Gi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;180Mi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;4.2Gi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;5.2Gi<br>
        Swap:&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;4.0Gi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;320Mi&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;3.7Gi
      </div>
    `;
  } else if (lowerCmd.includes('neofetch') || lowerCmd.includes('fastfetch')) {
    output = `
      <div class="flex gap-4 font-mono text-xs">
        <div class="text-brand-400 leading-none font-bold">
          &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;_,met$$$$$gg.<br>
          &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;,g$$$$$$$$$$$$$$$P.<br>
          &nbsp;&nbsp;&nbsp;,g$$P""&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;"""Y$$.".<br>
          &nbsp;&nbsp;,$$P'&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;\`$$$.<br>
          &apos;,$$P&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;,ggs.&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;\`$$b:<br>
          \`d$$'&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;$P"'&nbsp;&nbsp;&nbsp;.&nbsp;&nbsp;&nbsp;&nbsp;$$$<br>
          &nbsp;$$P&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;d&apos;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;,d&nbsp;&nbsp;&nbsp;&nbsp;$$$<br>
          &nbsp;$$:&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;$.&nbsp;&nbsp;&nbsp;-&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;$$$<br>
          &nbsp;$$;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;Y._&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;d$$<br>
          &nbsp;$$;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;\`"Y$$$$$P"&nbsp;&nbsp;&nbsp;$$$
        </div>
        <div class="text-slate-300 space-y-0.5 leading-5">
          <div class="text-brand-300 font-bold">root@prod-k8s-master-01</div>
          <div class="text-slate-600">----------------------</div>
          <div><span class="text-cyan-400">OS:</span> Ubuntu 22.04.3 LTS x86_64</div>
          <div><span class="text-cyan-400">Host:</span> Aliyun ecs.c7.2xlarge</div>
          <div><span class="text-cyan-400">Kernel:</span> 5.15.0-91-generic</div>
          <div><span class="text-cyan-400">Uptime:</span> 42 days, 18 hours</div>
          <div><span class="text-cyan-400">Packages:</span> 982 (dpkg)</div>
          <div><span class="text-cyan-400">Shell:</span> bash 5.1.16</div>
          <div><span class="text-cyan-400">CPU:</span> Intel Xeon Platinum 8369B (8) @ 3.500GHz</div>
          <div><span class="text-cyan-400">Memory:</span> 9832MiB / 16048MiB</div>
        </div>
      </div>
    `;
  } else if (lowerCmd === 'sftp') {
    jumpToSftpCurrent();
    return;
  } else {
    // 通用回应
    output = `<span class="text-slate-400">指令已在远端安全执行完成：exit code 0 (耗时 12ms)</span>`;
  }

  currentTab.history.push({ cmd, output });
  renderTerminalContent();
}

// 常用命令片段点击一键发送
function sendSnippet(snippet) {
  executeTerminalCommand(snippet);
}

// 终端一键切换至 SFTP
function jumpToSftpCurrent() {
  const currentTab = AppState.terminalTabs[AppState.currentTerminalTabIndex];
  connectSftp(currentTab.hostId);
}

// 清屏
function clearTerminalScreen() {
  const currentTab = AppState.terminalTabs[AppState.currentTerminalTabIndex];
  currentTab.history = [];
  renderTerminalContent();
  showToast('屏幕已清空', 'info');
}

// 分屏提示
function toggleSplitScreen() {
  showToast('已激活水平/垂直分屏布局：支持并排对比多台主机输出', 'info');
}

// ==========================================
// 5. SFTP 远程文件管理器 (SFTP Explorer)
// ==========================================
function connectSftp(hostId) {
  AppState.sftp.currentHostId = hostId;
  const host = AppState.hosts.find(h => h.id === hostId);
  const nameElem = document.getElementById('current-sftp-host-name');
  if (nameElem && host) {
    nameElem.textContent = host.name;
  }

  switchView('sftp');
  renderSftpBreadcrumbs();
  renderSftpFiles();
  showToast(`已连接并加载 ${host ? host.name : hostId} 的 SFTP 文件系统`, 'success');
}

function renderSftpBreadcrumbs() {
  const container = document.getElementById('sftp-breadcrumb');
  if (!container) return;

  const parts = AppState.sftp.currentPath.split('/').filter(Boolean);
  let accumulated = '';
  
  let html = `
    <span onclick="navigateToPath('/')" class="hover:text-white cursor-pointer px-1 py-0.5 rounded hover:bg-surface-elevated font-semibold">/</span>
  `;

  parts.forEach((part, idx) => {
    accumulated += '/' + part;
    const isLast = idx === parts.length - 1;
    const pathRef = accumulated;
    html += `
      <span class="text-slate-600">/</span>
      <span onclick="navigateToPath('${pathRef}')" class="${isLast ? 'text-brand-400 font-bold' : 'hover:text-white text-slate-300'} cursor-pointer px-1.5 py-0.5 rounded hover:bg-surface-elevated">
        ${part}
      </span>
    `;
  });

  container.innerHTML = html;
}

function navigateToPath(newPath) {
  AppState.sftp.currentPath = newPath;
  renderSftpBreadcrumbs();
  // 模拟目录加载过渡动画
  const icon = document.getElementById('sftp-refresh-icon');
  if (icon) icon.classList.add('animate-spin');
  setTimeout(() => {
    if (icon) icon.classList.remove('animate-spin');
    renderSftpFiles();
  }, 250);
}

function renderSftpFiles() {
  const tbody = document.getElementById('sftp-file-rows');
  if (!tbody) return;

  const files = AppState.sftp.files.filter(f => {
    if (!AppState.showHiddenFiles && f.isHidden) return false;
    return true;
  });

  tbody.innerHTML = files.map((file, idx) => {
    const isDir = file.isDir;
    const icon = getFileIcon(file);
    return `
      <tr class="sftp-row hover:bg-surface-card/60 transition-colors group cursor-pointer" onclick="handleFileClick(${idx}, event)">
        <td class="py-2.5 px-4 text-center" onclick="event.stopPropagation()">
          ${file.name === '..' ? '' : `<input type="checkbox" class="file-checkbox rounded bg-surface-card border-surface-border text-brand-500 focus:ring-0" onchange="updateSelectedCount()">`}
        </td>
        <td class="py-2.5 px-4">
          <div class="flex items-center gap-2.5 font-medium ${isDir ? 'text-white' : 'text-slate-200'}">
            ${icon}
            <span class="${isDir ? 'hover:underline text-cyan-300' : 'text-slate-200'}">${file.name}</span>
          </div>
        </td>
        <td class="py-2.5 px-4 text-right text-slate-400 text-[11px]">${file.size}</td>
        <td class="py-2.5 px-4 text-slate-400 text-[11px]">${file.type}</td>
        <td class="py-2.5 px-4 text-slate-400 text-[11px] font-mono">${file.chmod}</td>
        <td class="py-2.5 px-4 text-slate-400 text-[11px]">${file.owner}</td>
        <td class="py-2.5 px-4 text-slate-400 text-[11px]">${file.time}</td>
        <td class="py-2.5 px-4 text-center" onclick="event.stopPropagation()">
          <div class="flex items-center justify-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
            ${file.editable ? `
              <button onclick="openFileEditor('${file.name}')" class="p-1 rounded hover:bg-surface-elevated text-brand-400 hover:text-brand-300" title="在线编辑">
                <i data-lucide="edit-3" class="w-3.5 h-3.5"></i>
              </button>
            ` : ''}
            <button onclick="downloadFile('${file.name}')" class="p-1 rounded hover:bg-surface-elevated text-cyan-400 hover:text-cyan-300" title="下载至本地">
              <i data-lucide="download" class="w-3.5 h-3.5"></i>
            </button>
            <button onclick="deleteFilePrompt('${file.name}')" class="p-1 rounded hover:bg-surface-elevated text-rose-400 hover:text-rose-300" title="删除文件">
              <i data-lucide="trash-2" class="w-3.5 h-3.5"></i>
            </button>
          </div>
        </td>
      </tr>
    `;
  }).join('');

  if (window.lucide) lucide.createIcons();
}

function getFileIcon(file) {
  if (file.name === '..') {
    return `<i data-lucide="corner-left-up" class="w-4 h-4 text-slate-400"></i>`;
  }
  if (file.isDir) {
    return `<i data-lucide="folder" class="w-4 h-4 text-amber-400 fill-amber-400/20"></i>`;
  }
  if (file.name.endsWith('.conf') || file.name.endsWith('.yaml') || file.name.endsWith('.env')) {
    return `<i data-lucide="file-cog" class="w-4 h-4 text-brand-400"></i>`;
  }
  if (file.name.endsWith('.log')) {
    return `<i data-lucide="file-text" class="w-4 h-4 text-blue-400"></i>`;
  }
  if (file.name.endsWith('.tar.gz') || file.name.endsWith('.zip')) {
    return `<i data-lucide="archive" class="w-4 h-4 text-purple-400"></i>`;
  }
  if (file.name.endsWith('.crt') || file.name.endsWith('.key')) {
    return `<i data-lucide="shield-check" class="w-4 h-4 text-emerald-400"></i>`;
  }
  return `<i data-lucide="file" class="w-4 h-4 text-slate-400"></i>`;
}

function handleFileClick(index, e) {
  const file = AppState.sftp.files[index];
  if (!file) return;

  if (file.isDir) {
    if (file.name === '..') {
      const parts = AppState.sftp.currentPath.split('/').filter(Boolean);
      parts.pop();
      navigateToPath('/' + parts.join('/'));
    } else {
      navigateToPath(AppState.sftp.currentPath + '/' + file.name);
    }
  } else if (file.editable) {
    openFileEditor(file.name);
  } else {
    showToast(`查看文件信息: ${file.name} (${file.size})`, 'info');
  }
}

function refreshSftpList() {
  const icon = document.getElementById('sftp-refresh-icon');
  if (icon) icon.classList.add('animate-spin');
  setTimeout(() => {
    if (icon) icon.classList.remove('animate-spin');
    renderSftpFiles();
    showToast('SFTP 目录已刷新', 'info');
  }, 300);
}

function toggleHiddenFiles() {
  AppState.showHiddenFiles = !AppState.showHiddenFiles;
  const btn = document.getElementById('btn-toggle-hidden');
  if (btn) {
    btn.classList.toggle('text-brand-400', AppState.showHiddenFiles);
  }
  renderSftpFiles();
  showToast(AppState.showHiddenFiles ? '已显示隐藏文件' : '已隐藏点开头文件', 'info');
}

function copyCurrentPath() {
  navigator.clipboard?.writeText(AppState.sftp.currentPath);
  showToast(`已复制路径: ${AppState.sftp.currentPath}`, 'success');
}

function openInTerminalCurrentDir() {
  switchView('terminal');
  executeTerminalCommand(`cd ${AppState.sftp.currentPath} && ls -lh`);
  showToast(`已在终端切至 ${AppState.sftp.currentPath}`, 'info');
}

// 模拟上传
function triggerUploadFile() {
  const input = document.createElement('input');
  input.type = 'file';
  input.onchange = (e) => {
    const file = e.target.files[0];
    if (file) {
      addTransferTask(`上传: ${file.name}`, 'upload', `${(file.size / 1024 / 1024).toFixed(1)} MB`);
      toggleTransferDrawer(true);
    }
  };
  input.click();
}

function downloadFile(fileName) {
  addTransferTask(`下载: ${fileName}`, 'download', '3.4 MB');
  toggleTransferDrawer(true);
}

function deleteFilePrompt(fileName) {
  if (confirm(`确定要从服务器远程删除 ${fileName} 吗？此操作无法撤销。`)) {
    AppState.sftp.files = AppState.sftp.files.filter(f => f.name !== fileName);
    renderSftpFiles();
    showToast(`已成功删除 ${fileName}`, 'warning');
  }
}

function toggleSelectAllFiles(checkbox) {
  const checkboxes = document.querySelectorAll('.file-checkbox');
  checkboxes.forEach(cb => cb.checked = checkbox.checked);
  updateSelectedCount();
}

function updateSelectedCount() {
  const checked = document.querySelectorAll('.file-checkbox:checked').length;
  const label = document.getElementById('sftp-selected-text');
  if (label) {
    label.textContent = checked > 0 ? `已选中 ${checked} 项` : '未选中项目';
    label.classList.toggle('text-brand-400', checked > 0);
  }
}

// ==========================================
// 6. 传输抽屉与任务进度中心 (Transfers)
// ==========================================
function toggleTransferDrawer(forceState) {
  const drawer = document.getElementById('transfer-drawer');
  if (!drawer) return;

  if (typeof forceState === 'boolean') {
    if (forceState) drawer.classList.add('drawer-open');
    else drawer.classList.remove('drawer-open');
  } else {
    drawer.classList.toggle('drawer-open');
  }
}

function addTransferTask(title, type, size) {
  const container = document.getElementById('transfer-list-items');
  if (!container) return;

  const isUpload = type === 'upload';
  const taskId = 'task-' + Date.now();

  const itemHtml = `
    <div id="${taskId}" class="p-2.5 rounded-lg bg-surface-card border border-surface-border/80 space-y-2">
      <div class="flex items-center justify-between text-xs">
        <div class="flex items-center gap-2 truncate pr-2">
          <i data-lucide="${isUpload ? 'arrow-up-circle' : 'arrow-down-circle'}" class="w-4 h-4 ${isUpload ? 'text-brand-400' : 'text-cyan-400'} shrink-0"></i>
          <span class="font-medium text-slate-200 truncate">${title}</span>
        </div>
        <span class="font-mono text-[11px] text-brand-400 font-semibold shrink-0" id="${taskId}-pct">12%</span>
      </div>
      <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
        <div id="${taskId}-bar" class="bg-gradient-to-r ${isUpload ? 'from-brand-500 to-cyan-400' : 'from-cyan-500 to-blue-400'} h-full rounded-full transition-all duration-300" style="width: 12%;"></div>
      </div>
      <div class="flex items-center justify-between text-[10px] font-mono text-slate-400">
        <span>${size}</span>
        <span id="${taskId}-speed">16.8 MB/s · 传输中</span>
      </div>
    </div>
  `;

  container.insertAdjacentHTML('afterbegin', itemHtml);
  if (window.lucide) lucide.createIcons();

  // 模拟进度条增长
  let progress = 12;
  const interval = setInterval(() => {
    progress += Math.floor(Math.random() * 20) + 15;
    const bar = document.getElementById(`${taskId}-bar`);
    const pct = document.getElementById(`${taskId}-pct`);
    if (bar && pct) {
      bar.style.width = Math.min(100, progress) + '%';
      pct.textContent = Math.min(100, progress) + '%';
    }
    if (progress >= 100) {
      clearInterval(interval);
      const speed = document.getElementById(`${taskId}-speed`);
      if (speed) speed.textContent = '传输已完成';
      showToast(`${title} 传输成功！`, 'success');
    }
  }, 400);
}

function clearCompletedTransfers() {
  showToast('已清理传输历史', 'info');
}

// ==========================================
// 7. 新建服务器弹窗 (New Host Modal)
// ==========================================
function openNewHostModal() {
  const modal = document.getElementById('modal-new-host');
  if (modal) {
    modal.classList.remove('hidden');
    setTimeout(() => modal.classList.add('modal-open'), 10);
  }
}

function closeNewHostModal() {
  const modal = document.getElementById('modal-new-host');
  if (modal) {
    modal.classList.remove('modal-open');
    setTimeout(() => modal.classList.add('hidden'), 200);
  }
}

function switchAuthType(type) {
  const pwdField = document.getElementById('auth-password-field');
  const keyField = document.getElementById('auth-key-field');
  if (pwdField) pwdField.classList.toggle('hidden', type !== 'password');
  if (keyField) keyField.classList.toggle('hidden', type !== 'key');
}

function testConnection() {
  const hostVal = document.getElementById('host-ip').value || '目标服务器';
  showToast(`正在通过 SSH 探测 ${hostVal}:22 连通性...`, 'info');
  setTimeout(() => {
    showToast(`连接测试成功！延迟 22ms，支持 OpenSSH_8.9`, 'success');
  }, 600);
}

function handleSaveHost(e) {
  e.preventDefault();
  const name = document.getElementById('host-name').value;
  const group = document.getElementById('host-group').value;
  const ip = document.getElementById('host-ip').value;
  const port = parseInt(document.getElementById('host-port').value || 22);
  const user = document.getElementById('host-user').value || 'root';
  const tagsStr = document.getElementById('host-tags').value || '';
  const tags = tagsStr.split(',').map(s => s.trim()).filter(Boolean);

  const newHost = {
    id: 'host-' + Date.now(),
    name: name || ip,
    group: group,
    os: 'ubuntu',
    osName: 'Ubuntu 22.04 LTS',
    host: ip,
    port: port,
    user: user,
    auth: 'Password',
    status: 'online',
    ping: '22ms',
    cpu: Math.floor(Math.random() * 30) + 10,
    mem: Math.floor(Math.random() * 40) + 20,
    disk: 35,
    tags: tags.length ? tags : ['custom', 'ssh'],
    updatedAt: '刚刚'
  };

  AppState.hosts.unshift(newHost);
  closeNewHostModal();
  renderHosts();
  showToast(`已成功添加主机: ${newHost.name}`, 'success');
}

// ==========================================
// 8. 在线文件代码编辑器 (Inline Code Editor)
// ==========================================
function openFileEditor(fileName) {
  const modal = document.getElementById('modal-file-editor');
  const pathLabel = document.getElementById('editor-file-path');
  const textarea = document.getElementById('editor-textarea');

  if (pathLabel) pathLabel.textContent = AppState.sftp.currentPath + '/' + fileName;
  if (textarea) textarea.value = SampleCode;

  if (modal) {
    modal.classList.remove('hidden');
    setTimeout(() => modal.classList.add('modal-open'), 10);
  }
}

function closeFileEditor() {
  const modal = document.getElementById('modal-file-editor');
  if (modal) {
    modal.classList.remove('modal-open');
    setTimeout(() => modal.classList.add('hidden'), 200);
  }
}

function saveEditorContent() {
  const status = document.getElementById('editor-sync-status');
  if (status) status.textContent = '正在通过 SFTP 写入远程服务器...';

  setTimeout(() => {
    if (status) status.textContent = '已成功写入并远程同步完成 (MD5 匹配)';
    showToast('文件已安全同步至远程服务器！', 'success');
    setTimeout(() => {
      closeFileEditor();
    }, 500);
  }, 400);
}

// ==========================================
// 9. 全局命令面板 (Ctrl+K Command Palette)
// ==========================================
function toggleCommandPalette(show) {
  const modal = document.getElementById('modal-command-palette');
  const input = document.getElementById('palette-input');
  if (!modal) return;

  if (show) {
    modal.classList.remove('hidden');
    setTimeout(() => {
      modal.classList.add('modal-open');
      if (input) {
        input.value = '';
        input.focus();
      }
    }, 10);
  } else {
    modal.classList.remove('modal-open');
    setTimeout(() => modal.classList.add('hidden'), 200);
  }
}

function handlePaletteBackdropClick(e) {
  toggleCommandPalette(false);
}

function selectPaletteAction(actionKey) {
  toggleCommandPalette(false);
  if (actionKey === 'ssh-prod-01') {
    connectHost('prod-k8s-master-01');
  } else if (actionKey === 'sftp-prod-01') {
    connectSftp('prod-k8s-master-01');
  }
}

function filterPalette(val) {
  const items = document.querySelectorAll('.palette-item');
  const query = val.toLowerCase();
  items.forEach(item => {
    const text = item.textContent.toLowerCase();
    item.style.display = text.includes(query) ? 'flex' : 'none';
  });
}

// 键盘快捷键监听
document.addEventListener('keydown', (e) => {
  // Ctrl+K 或 Cmd+K 打开命令面板
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    toggleCommandPalette(true);
  }
  // ESC 键关闭弹窗
  if (e.key === 'Escape') {
    toggleCommandPalette(false);
    closeNewHostModal();
    closeFileEditor();
    toggleTransferDrawer(false);
  }
  // Ctrl+N 新建主机
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n' && !e.shiftKey) {
    e.preventDefault();
    openNewHostModal();
  }
});

// ==========================================
// 10. 全局 Toast 提示组件
// ==========================================
function showToast(message, type = 'info') {
  const container = document.getElementById('toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  const typeColors = {
    info: 'border-blue-500/40 text-blue-300 bg-[#161d2d]/95',
    success: 'border-emerald-500/40 text-emerald-300 bg-[#11241f]/95',
    warning: 'border-amber-500/40 text-amber-300 bg-[#2b2214]/95',
    error: 'border-rose-500/40 text-rose-300 bg-[#2b161a]/95',
  };

  const typeIcons = {
    info: 'info',
    success: 'check-circle-2',
    warning: 'alert-triangle',
    error: 'alert-octagon',
  };

  toast.className = `flex items-center gap-2.5 px-3.5 py-2.5 rounded-xl border shadow-xl text-xs backdrop-blur-md transition-all duration-300 transform translate-y-2 opacity-0 font-medium ${typeColors[type] || typeColors.info}`;
  toast.innerHTML = `
    <i data-lucide="${typeIcons[type] || 'info'}" class="w-4 h-4 shrink-0"></i>
    <span class="flex-1 font-sans">${message}</span>
  `;

  container.appendChild(toast);
  if (window.lucide) lucide.createIcons();

  requestAnimationFrame(() => {
    toast.classList.remove('translate-y-2', 'opacity-0');
  });

  setTimeout(() => {
    toast.classList.add('opacity-0', '-translate-y-2');
    setTimeout(() => toast.remove(), 300);
  }, 2600);
}

// 模拟扩展弹窗
function openKeyVaultModal() {
  showToast('密钥库管理器：支持 RSA / Ed25519 密钥生成、硬件密钥 YubiKey 与凭证导入', 'info');
}

function openSnippetsModal() {
  showToast('常用指令库：可按团队共享命令片段、自动化运维脚本', 'info');
}

function openTunnelsModal() {
  showToast('端口转发隧道管理器：支持本地转发 (Local)、远程反向转发 (Remote) 及 SOCKS 动态代理', 'info');
}

function openNewFolderModal() {
  const name = prompt('请输入新文件夹名称:', 'new_folder');
  if (name) {
    AppState.sftp.files.splice(1, 0, {
      name: name,
      isDir: true,
      size: '4.0 KB',
      type: '目录',
      chmod: 'drwxr-xr-x (755)',
      owner: 'root:root',
      time: '刚刚'
    });
    renderSftpFiles();
    showToast(`已创建远程文件夹: ${name}`, 'success');
  }
}

function openSettingsModal() {
  showToast('偏好设置：支持终端主题切换 (Tokyo Night, Catppuccin, One Dark)、字体调节与快捷键定制', 'info');
}

function openImportModal() {
  showToast('支持从 SSH Config (~/.ssh/config)、FinalShell、Xshell、Termius 导入主机配置', 'info');
}

// ==========================================
// 11. 页面初始化
// ==========================================
document.addEventListener('DOMContentLoaded', () => {
  renderHosts();
  renderTerminalTabs();
  renderTerminalContent();
  renderSftpBreadcrumbs();
  renderSftpFiles();

  // 默认启动欢迎提示
  setTimeout(() => {
    showToast('SnailSSH 现代化原型加载就绪！随时使用 Ctrl+K 搜索', 'success');
  }, 400);
});
