# Windows / macOS 冒烟测试

> 临时任务清单，做完即删。Linux 的冒烟口径以此为准，两个平台只是换系统重跑。

## 范围

- 全功能冒烟，**不含联机大厅（`lobby*`）与云存档**。
- Linux 已完成一轮（前端 lint / 类型 / 单测 / 202 个界面场景 / 全仓 `cargo check --all-targets`）。
- 目标平台：Windows 10/11 x64、macOS 14+（arm64 与 x64 各一台）。
- 判定标准：命令零失败；界面场景零运行时错误、零控制台错误、零未 mock 命令；
  手测清单逐项有勾选结论。

## 0. 环境准备

- Node ≥ 24、npm（仓库用 `mml-vue/package-lock.json`，用 `npm ci`）。
- Rust stable ≥ 1.85（workspace 用 edition 2024），记录 `cargo --version`。
- Windows：WebView2 Runtime、PowerShell 5.1、Edge。
- macOS：Xcode Command Line Tools、系统 WebKit。
- 截图用的浏览器：
  - Windows：`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`
  - macOS：`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`

## 1. 前端静态检查

```bash
cd mml-vue
npm ci
npm run lint          # 期望 0 error
npx vue-tsc --noEmit  # 期望无输出
npm test              # 期望 71 passed / 0 failed
```

## 2. 界面冒烟（截图）

```bash
cd mml-vue
# 端口 1420 必须先空出来
npm run shots -- --clean --skip=lobby --edge="<上面的浏览器路径>"
npm run shots:docs
```

- 期望：202 张图，`target/screens/report.json` 里 `unknown` / `runtimeErrors` / `consoleErrors` 全空。
- 已知可接受的一条：`add-resource` 的 `onUnmounted ... after await` 警告（历史遗留，与功能无关）。
- 看 `target/screens/index.html` 逐张过一遍，重点看字体、图标、滚动条、弹层位置是否与 Linux 一致。

## 3. 后端编译与测试

```bash
cargo check --workspace --all-targets -j 4
cd ipc-gen && cargo test && cd ..
```

平台相关 crate 单独跑：

```bash
cargo test -p mml-sys     # 提权链路：Windows 命名管道 / Linux pkexec / macOS osascript
cargo test -p mml-base
cargo test -p mml-config
cargo test -p mml-jvms
cargo test -p mml-nbt
cargo test -p mml-game --lib
```

- `mml-gui` 的集成测试（`mml-gui/src-tauri/tests/`）按改动面挑相关的跑。
- 渲染相关（`mml-tex-draw`）依赖下载客户端 jar，按需跑，不要整仓全量。

## 4. 手测清单（`npm run tauri dev` 真实启动）

### 4.1 主窗口

- [ ] 实例网格 / 列表 / 分组三种视图切换，排序、筛选、搜索
- [ ] 新建实例入口、实例右键菜单（启动 / 设置 / 删除 / 收藏 / 打开目录）
- [ ] 多选工具条：批量启动、批量删除、批量分组
- [ ] 实例重命名、删除（回收站）、收藏、分组增删改与拖拽
- [ ] 启动游戏：进度、日志窗口、停止、启动参数、内存设置
- [ ] 窗口几何记忆（关窗再开位置尺寸一致）

### 4.2 添加实例

- [ ] 四种模式：从头新建 / 导入压缩包 / 添加文件夹 / 在线实例
- [ ] 页签栏最后一项「下载整合包」：点了开下载整合包窗口，页签不会变成选中
- [ ] 五个必填校验：名称、游戏版本、压缩包、文件夹、下载地址 —— 红框 + 红字贴在输入框下方
- [ ] 读包失败 / 读目录失败的红字同样贴在对应输入框下
- [ ] 整合包（CurseForge / Modrinth）导入后走安装进度

### 4.3 下载与资源

- [ ] 下载整合包：搜索、安装、更新、取消、进度指示器
- [ ] 下载队列：暂停 / 继续 / 取消 / 重试 / 失败提示
- [ ] 资源管理：模组 / 资源包 / 光影 / 存档 / 服务器 / 截图各分类，启用禁用、删除、拖入安装
- [ ] 依赖检测与依赖图、更新检查、更新记录
- [ ] 添加资源窗口、文件选择、下载进度条

### 4.4 实例相关窗口

- [ ] 游戏日志（含启动与尾部日志跟随）
- [ ] 配置文件编辑：JSON / TOML / YAML / XML / JS 语法高亮与差异
- [ ] 导出实例、生成服务器实例、服务器包同步
- [ ] 方块与物品、统计数据、Java 下载与扫描
- [ ] 帮助窗口

### 4.5 设置

- [ ] 外观：主题、强调色、字体、字体大小、窗口材质
- [ ] 启动、Java、网络与下载、游戏、手柄
- [ ] 客户端设置：登录方式锁定、自定义主页面 / 自定义 CSS / 整页界面
- [ ] 本地文件：打开目录、清理缓存
- [ ] 语言切换（中文 / English）全窗口即时生效

### 4.6 账户

- [ ] 离线账户：新建、改名改 UUID、删除
- [ ] 微软账户：设备码登录、取消、重新登录
- [ ] LittleSkin / 外置登录 / 统一通行证：新增、重新登录（登录地址锁定不可改）
- [ ] 皮肤弹窗：卡片上点皮肤 / 披风图能打开，2D / 3D 切换、设为装备
- [ ] 空账户时标题栏只剩标题，添加入口在空状态里
- [ ] 登录方式锁定后新增账户只有锁定的类型

### 4.7 自定义界面

- [ ] 自定义主页面：导入 zip（`index.html` 或 `custom.js`）、开关生效、恢复
- [ ] 自定义 CSS：导入多份、单条启用 / 停用、`custom.js` 入口
- [ ] 完全自定义界面：整页包替换、退出整页、异常恢复（删包后能回内置界面）

### 4.8 平台能力

- [ ] 系统浏览器打开链接、复制到剪贴板、拖入文件 / 文件夹 / 网址
- [ ] 管理员 / 提权启动（Windows UAC、macOS osascript）
- [ ] 单窗口 / 多窗口模式、窗口最小化与关闭、退出

## 5. 两个平台的重点差异

| 方向 | Windows | macOS |
| --- | --- | --- |
| 提权 | UAC、命名管道 | `osascript` + 系统密码框 |
| 进程 | `cmd`、任务管理器式终止 | POSIX 信号、进程组 |
| 文件 | 盘符路径、回收站 | 沙箱与"完全磁盘访问"权限、`~/Library` |
| 显示 | DPI 缩放（125% / 150%） | Retina 缩放、菜单栏 |
| 打包 | NSIS（x64 / x86 / arm64） | `.app` / `.dmg`，Gatekeeper 首次启动 |
| 输入 | 手柄 XInput | 手柄 GameController |

## 6. 记录方式

每个平台开一张表，逐项写结论；失败的附日志片段或截图名：

| 用例 | 结果 | 证据 |
| --- | --- | --- |
| 例：导入 zip 整合包 | 通过 | `target/screens/add-archive.png` |

## 7. 已知问题（Linux 冒烟发现，已修）

- `mml-core/mml-sys/tests/process_utils.rs` 的 Linux / macOS 模块缺 `use mml_sys::process_helper;`，
  且残留 `use mml_base::process_utils;`（`mml-sys` 没有这个 dev-dependency）。
  Linux 下 `cargo check --all-targets` 直接编译失败 —— 这两个平台的提权测试从未被编译过。
  已补上 import、删掉无效引用；Windows / macOS 上仍要确认这两个用例真的能跑起来。
