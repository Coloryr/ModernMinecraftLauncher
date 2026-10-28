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

> 提权用例要当心：Linux 的 `pkexec` 用例在没有图形会话 / polkit 代理时会**卡住等密码**，
> CI 或远程 shell 里用 `cargo test -p mml-sys --test process_utils -- --skip admin` 跳过，
> 然后在有桌面的机器上单独跑一次。macOS 的 `osascript` 同理。
>
> `mml-base` 的归档测试会在 crate 目录下用相对路径 `test_run/`，跑完会删掉
> `mml-core/mml-base/test_run/test.zip`（这个文件在版本库里）。跑完记得
> `git checkout -- mml-core/mml-base/test_run/test.zip`，或先把它挪出仓库。

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

## 7. Linux 冒烟结论（2026-10-28）

跑过的范围与结果：

| 项 | 结果 |
| --- | --- |
| `npm run lint` / `npx vue-tsc --noEmit` | 0 error / 无输出 |
| `npm test`（mml-vue） | 71 passed |
| 界面场景 215 张（`--skip=lobby`，共 218 个场景） | 0 运行时错误 / 0 未 mock 命令 / 0 控制台错误；7 个交互断言场景 |
| `cargo check --workspace --all-targets` | 通过（仅 dead_code 警告） |
| `cargo test` 15 个 crate（game / downloader / net / auth / skin / skin-draw / config / names / base / ipc / jvms / log / nbt / analysis / sys） | 121 个测试套件、870 passed、0 failed、40 ignored |
| `cargo test -p mml-tex-draw` | 方块 / 物品全量渲染通过（781 个，真下载客户端 jar） |
| 真实链路 | 客户端文件真下载、真起 JVM 进程、Modrinth 真包安装、Java 下载解包注册全通过 |
| `mml-gui` | theme 31、java_download（真下载 JRE）、collect（新增）通过 |

修掉的问题：

1. `mml-sys/tests/process_utils.rs` 的 Linux / macOS 模块缺 `use mml_sys::process_helper;`，
   还残留 `use mml_base::process_utils;`（`mml-sys` 没有这个 dev-dependency）。
   这两个平台的提权测试从来没被编译过，Linux 下 `cargo check --all-targets` 直接失败。
2. `mml-config` 的测试仍按旧默认值断言 `download_thread == 5`，实际默认已是 10；
   同步了 `tests/config.rs` 与 `tests/config_obj.rs`。若默认值本该是 5，要改的是 `config_obj.rs`。
3. `path_helper` 的 `write_text_async` / `write_bytes_async` / `write_stream_async` 写完没 `flush`：
   tokio 的 `File` 自带缓冲，紧跟其后的同步读会读到空文件。已补 `flush`。
4. `AddModpackWindow` / `AddResourceWindow` 在 `onMounted` 的 `await` 之后调 `onUnmounted`，
   订阅永远不会注销（Vue 只打一条告警）。改成 `useUnlisteners()`。

后端（mml-game / mml-downloader / mml-net / mml-auth / mml-skin / mml-skin-draw）跑下来的结论：

| 项 | 结果 |
| --- | --- |
| 离线用例 | 除下面几条外全绿 |
| `server_pack_import` | 8 条用例共用一个实例名，并行会互相覆盖，且偶发失败；已改为独立实例名 + 串行锁，连跑 3 次稳定 |
| `shaderpacks` / `modrinth_api_test` | 5 条用例没启动 `mml_log`，`mml_net::init()` 一写日志就 panic；已补 `mml_log::start` |
| `url_helper` | 2 条期望值停在旧实现（`forge_url_fix` 现在对普通版本返回空串；NeoForge maven 已迁到 `maven.neoforged.net`） |
| `abort_test` | 断言中文「已中断」，实际错误是 `Aborted { url }`；改为断言变体名 |
| `real_pack_download`（真实 Modrinth 包） | 把线上包 NeoForge 补丁号写死成 `26.2.0.75`，上游已改成 `26.2.0.88`；改为只钉 `26.2.x` |
| `mml-gui/tests/java_download_test` | 真下载 JDK → 解包 → 注册；原来把系统写死成 `windows`，在 Linux / macOS 上会去下 Windows 包并卡住，已改为按当前系统取包 |
| `real_launch::real_download_prepares_game_files` | 真下载 1.20.1 客户端 jar + 53 个运行库，通过 |
| `mml-gui/tests/collect_test`（新增） | 收藏 / 分组持久化：收藏去重、建组、移动、清空、落盘、读回，通过 |
| `mml-tex-draw/tests/render_all` | 物品数量区间与 `apple → foodAndDrink` 都是 1.20.1 的旧数据；分组表按 26.3 生成、渲染用的是 Mojang 最新版，改为只卡下限 + 用跨版本稳定的 stick / torch 验分组 |

真实网络用例的两个环境前提（**Windows / macOS 上同样适用**）：

- `real_launch`、`real_pack_download`、`modpack_upgrade` 默认走 `127.0.0.1:7890` 代理（作者本机环境）。
  直连能上外网的机器要加 `MML_TEST_PROXY=off`，否则报 `tunnel error: Connection refused`；
  三个文件现在都认这个写法。
- `real_launch_downloads_and_starts` 需要带 `jdk.compiler` 的 JDK（探针走源文件模式），
  没有就自动跳过；可用启动器自己的「下载 Java」拉一个，再把 `JAVA_HOME` 指过去。

macOS 提权启动日志（本轮修改，**必须在真机复核**）：

- 原来走 `osascript` 的 `do shell script … 2>&1`：输出被攒成脚本返回值，游戏运行期间日志窗口是空的，
  退出后才一次性刷进来，长输出还可能被截断。
- 现在改成 FIFO：提权脚本把 stdout / stderr 重定向到 `mml-launch-*.fifo`，启动器后台线程打开读端，
  `read` 与普通管道一致（写端关闭即 EOF），脚本收尾 `rm -f` 掉 FIFO。
  建不出 FIFO 时自动退回旧写法，不影响启动。
- Linux 上有单测覆盖脚本拼接、FIFO 读写与清理；macOS 上要实测：日志是否实时滚动、
  取消授权后是否卡住、游戏退出后日志窗口是否正常结束。
- 顺带修了两个 macOS 编译错误（`memory_helper` 里的 `process_utils` 应为 `process_helper`、
  `open_helper` 的 `open -R` 参数类型）。`cargo check -p mml-sys --target aarch64-apple-darwin` 通过；
  整仓 macOS 交叉编译在 Linux 上验不了（`objc2-exception-helper` 需要 macOS SDK 的 `-arch`），
  只能在 macOS runner 上跑。

Windows / macOS 上要重点复核的：

- 上面第 1 条的两个提权用例能否真正跑通（Windows 命名管道 / UAC、macOS `osascript`）。
- 第 3 条在 Windows 上的表现（tokio 缓冲 + NTFS 语义）。
- 界面场景里字体、图标、滚动条、弹层定位与 Linux 的差异。
