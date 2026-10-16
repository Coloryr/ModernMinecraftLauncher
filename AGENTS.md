# M²L 项目约定

> 适用于本仓库内的所有协作方（人类与 AI 助手）。

## 1. 屏幕与桌面操作（硬性）

- **禁止截图 / 录屏**：不得对屏幕或任何窗口截图、捕获图像、录屏，包括"为了看界面效果"。
  只有**用户在当轮对话中明确允许**时才可进行。
- **禁止操作用户桌面**：不得移动或点击鼠标、发送键盘输入、切换窗口前台、枚举 / 操纵窗口
  （如 `SetForegroundWindow`、`EnumWindows`、窗口矩形抓取等），除非用户明确允许。
- 需要确认界面效果时：不要自己去"看"，而是
  1. 说明改了哪些文件、期望表现如何，由用户自行启动查看；或
  2. 给出用户可自行打开的入口（命令、URL、界面按钮位置）。

## 2. 临时验证代码

- 为验证效果临时加入的代码（假数据、自动开窗、调试输出等）必须：
  - 在代码中标注 `TEMP` 注释；
  - 验证结束后**立即还原**，不得留在工作区。

## 3. 构建与运行

- **改任何代码之前，必须先关闭程序——没有例外。** 流程固定为：
  1. 先停掉正在运行的 `tauri dev` 及其带起的应用进程；
  2. 再改代码；
  3. 改完跑验证（`cargo check -p <crate>` / `npm run build`，见下一条）。
  **改完不需要（也不要）把启动器重新启动**，启动由用户自己来。
  运行中的 dev 监听会在文件保存中途触发重建 / 热更新，容易出现半成品编译错误、窗口反复重启或
  行为异常；用户正在用界面排查问题时尤其不能这么干，会打乱他手上的复现步骤。
  **不要因为「只是小改」「想省一次重启」就跳过第 1 步**，也不要指望靠热更新看效果。
- **前端一视同仁：`mml-vue/` 下的样式 / 文案 / 组件微调也要先关程序再改。**
  "只是改几个字""vite 会热更新"都不是理由——热更新恰恰是在文件改到一半时把半成品送进
  正在运行的界面，而用户很可能正拿着那个界面复现问题。
- **关不掉就先提醒用户，别硬改。** 出现下面任一情况，就把改动先放着，
  告诉用户"请先关闭启动器（`tauri dev` / `mml-gui.exe` / 占用 1420 的 vite），关好我再动手"，
  等用户确认已关闭再继续：
  - 结束进程失败（无权限、`taskkill` 报错、进程结束不掉）；
  - 不确定某个进程是不是这个项目的（例如有别的 Electron / Node 程序在跑）；
  - 用户说自己正开着界面（哪怕我这边查不到进程）。
- **`tauri dev` 通常是用户手动起的，关它由你负责，别让用户自己关。**（用户已明确授权。）
  动手前先查进程；发现还在跑就先结束掉，再开始改；**结束失败或进程归属不明时走上一条，
  提醒用户关闭**。参考命令（Windows / bash）：

  ```bash
  tasklist //FI "IMAGENAME eq mml-gui.exe"        # 应用进程
  netstat -ano | grep ":1420"                      # vite 监听（最后一列是 PID）
  taskkill //IM mml-gui.exe //F                   # 结束应用
  taskkill //PID <pid> //F                         # 结束 vite
  ```

  结束应用后 `tauri dev` 的父进程（cargo-tauri / npm）一般会自行退出；顺手 `tasklist` 确认一下
  `cargo.exe` / `node.exe` 也没了。
- **禁止过度检查 / 过度编译测试**：只跑与本次改动直接相关的最小验证——改动所在 crate 的
  `cargo check -p <crate>`、相关的那几个测试（`cargo test -p <crate> --lib`、
  `... --test <name>`，必要时加 `单个用例名` 过滤）。不要动辄
  `cargo check --workspace --all-targets` / `cargo test --workspace` / 整仓全量测试；
  同一结论不要重复跑第二遍（改了代码再验一次除外）。
- 桌面壳：`cd mml-gui && npm run tauri dev`（会先起 mml-vue 的 vite，**端口 1420 必须空闲**）。
- 只跑前端：`build/dev-frontend.ps1`（bash 版 `build/dev-frontend.sh`；vite，1420）；
  浏览器可访问 `http://localhost:1420/?window=<kind>` 预览某个窗口（无 IPC 数据）。
- **出包两个 profile**（各自独立 target 目录，来回切不会互相刷缓存）：
  - 正式发布：`build/build-release.ps1`（bash 版 `build/build-release.sh`；
    = `cd mml-gui && npm run tauri build`）。
    `[profile.release]` 是 fat LTO + `codegen-units = 1`：体积最小、运行最快，但末尾的
    跨 crate 优化是单线程的，**构建时会看到只有一个核在跑**，含安装包。
  - 预发布：`build/build-prerelease.ps1`（bash 版 `build/build-prerelease.sh`；
    = `cd mml-gui/src-tauri && cargo build --profile prerelease --features custom-protocol`）。
    **`--features custom-protocol` 不能漏**：tauri 用它决定前端走内嵌 dist 还是 devUrl，
    漏了即使 profile 是 release-like 也会被当成 dev 构建，exe 去连 localhost:1420，
    没起 vite 就是"进程在跑、窗口不出来"。
    `[profile.prerelease]` 是 thin LTO + `codegen-units = 16`：快很多，体积略大，只出 exe。
  - 预发布也要**安装包**时：`tauri` CLI 不认 `--profile`（`--` 透传给 cargo 会让它去
    `target/release` 找错文件），只能覆盖 release 的设置：
    `CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 npm run tauri build`。
    代价是它与正式发布共用 target 目录，来回切会重编末尾那几个单元。
- Rust workspace 在**仓库根目录**（根 `Cargo.toml` 收编 `mml-core/` 全部子 crate 与
  `mml-gui/src-tauri` + `ipc-gen` / `macros`；`workspace.dependencies` 与 `[profile.*]`
  也统一在根清单维护）。target 目录在根目录 `target\`（首次编译较慢）。

## 4. IPC 约定

- 命令名 = 函数名 = **`窗口名_方法名`**，例如 `main_get_instances`、`account_add_account`、
  `window_open_window`、`add_list_dir`。
- **命令与 DTO 的唯一来源是 Rust 源码**，全部由生成器产出。前端缺命令 / 缺字段时，
  先在 Rust 侧补 `#[tauri::command]` 或改 DTO 结构，再跑生成；**不要**在 `bindings.ts`
  或前端另写一套类型（下次生成会被覆盖，且与 Rust 侧脱节）。
  `src/lib/api.ts` 只是手写的便捷包装层，其中的类型一律 `import` 自 `./bindings`。
- 事件名由 `emit_xxx_yyy` 推导：去掉 `emit_` 前缀、`_` 换成 `-`，例如
  `emit_account_change` → `account-change`。
- 前端接口由 `mml-gui/src-tauri/build.rs`（逻辑在 `ipc-gen` 包）扫描 Rust 源码自动生成（**勿手改**）：
  - `mml-vue/src/lib/bindings.ts`：命令的类型化包装（`commands.xxx(...)`）+ 跨 IPC 类型的
    TS 定义。命令签名、DTO 结构、`#[serde(rename_all)]` 都是从 Rust 源码解析出来的
  - `mml-vue/src/lib/listens.ts`：事件名常量
  新增命令加 `#[tauri::command]`，新增事件加 `#[gui_macros::emit]`；单独重新生成可跑
  `cd mml-gui && npm run gen`（或任意 `cargo check`）。
- 扫描与生成逻辑在 `mml-gui/ipc-gen/`（包名 `gui-ipc-gen`，有单测）：
  `src-tauri/build.rs` 只是调用方（算路径、传 `GenConfig`、写文件、打 cargo 指令）；
  改生成规则改那个包，跑 `cd mml-gui/ipc-gen && cargo test` 验证。
- 命令在 `bindings.ts` 里按**来源 .rs 模块**分组（组键取模块最后一段，如
  `windows::account` → `commands.account`），组内剥掉命令名的公共前段
  （`add_modpack_search` → `commands.addModpack.search`）。同一个名字的命令**不能定义两次**，
  否则 `generate_handler!` 会报重复定义。组内顺序按**最终方法名**排序，所以命令在文件之间
  怎么搬都不会改变生成结果 —— 这也正是"拆分是否安全"的判据（拆完 `bindings.ts` 应逐字节不变）。
- **把命令拆进子模块**时要给它加 `#[gui_macros::ipc_group("原组名")]`（属性宏，仅作扫描标记）：
  组键默认取"来源 .rs 模块最后一段"，不钉住的话 `windows::resource::mods` 会让组键变成 `mods`，
  前端 `commands.resource.*` 全断。带命令的子模块还要写成 `pub(crate) mod`（生成的
  `tauri_commands!` 从 crate 根引用它）。生成器侧实现见 `ipc-gen/src/scan.rs` 的 `group_of`。
- **`ipc_group` 的值必须是"原始组名"（模块名），不是驼峰后的名字**：合并是按原始字符串做的，
  而 TS 属性名是 `camel(组键)`。写 `ipc_group("addModpack")` 会与 `add_modpack/mod.rs` 里
  留下的命令（组键 `add_modpack`）**分成两组**，生成物里出现两个 `addModpack: {` 对象 ——
  前端 `vue-tsc` 报 TS1117（对象字面量重名属性）。正确写法是 `ipc_group("add_modpack")`
  （实测踩过：`java_download` 拆分后 `npm run build` 才暴露）。
  判据：拆完 `npm run build` 必须过，`bindings.ts` 里同一组只能出现一个对象。
- 扫描是**裸文本**：注释里写出命令属性 / 事件属性的字面量，会让紧随其后的函数被误判成命令
  （实测踩过：`generate_handler!` 里出现不存在的 `__cmd__xxx`，编译失败）。别在注释里写它们。
- **命令的可见性不能降**：`#[tauri::command]` 生成的 `__cmd__xxx` / `__tauri_command_name_xxx`
  会继承函数可见性，把命令写成 `pub(super)` 后 `generate_handler!` 从 crate 根引用不到
  （E0603 "macro import is private"）。搬命令时统一 `pub(super) fn` 的脚本要把它排除掉。
- **集成测试按原路径引用命令**：`src-tauri/tests/*.rs` 里 `use mml_gui_lib::windows::<窗口>::<命令>`
  的地方，命令拆进子模块后要在 `mod.rs` 再导出（`pub use self::<子模块>::<命令>;`），
  否则 `cargo check --all-targets` 才报错（只跑 `cargo check -p mml-gui` 是发现不了的）。
- **DTO 换文件会改生成物的顺序**：生成器按**排序后的文件路径**遍历 `dtos/`，把一个 DTO 文件拆成
  目录后，`bindings.ts` 里类型声明的先后会变（内容不变）。判定"没改契约"要比对**类型块集合**
  （每个 `export type X = {...}` 归一化后按名字比），而不是看 diff 行数。
- 磁盘配置用 Rust 命名（snake_case），跨 IPC 传输一律经 `src-tauri/src/dtos/` 转成
  camelCase DTO。

## 5. 目录分工

| 目录 | 职责 |
| --- | --- |
| `mml-core/` | 启动器内核（多 crate，隶属根目录 workspace） |
| `mml-gui/` | Tauri 桌面壳；`src-tauri/src/windows/<窗口>.rs` 放该窗口的规格 / 模型 / IPC |
| `mml-vue/` | 前端（Vue3 + Vite）；窗口在 `src/windows/<kind>/`，通用组件在 `src/components/` |

### 5.1 窗口模块的拆分约定

一个窗口的代码超过**约 400 行**就目录化，别让它长成千行文件：

```
windows/<窗口>/mod.rs        ← 命令 + 该窗口的对外接口（按原路径再导出下面模块的东西）
windows/<窗口>/<职责>.rs     ← 一个职责一个文件（如 model / create / loader / list / skin）
windows/<窗口>/mod.rs 里：
    pub(crate) mod <带命令的子模块>;   // 生成的 tauri_commands! 从 crate 根引用它
    mod <纯辅助子模块>;                // 不带命令的可以私有
```

- **命令留在 `mod.rs` 最省事**；要下沉就必须带 `#[gui_macros::ipc_group("原组名")]`（见 §4），
  且子模块声明为 `pub(crate) mod`。
- **窗口之间共用的东西往下沉到 `windows/` 顶层**，不要互相 `use` 成环：
  `registry.rs`（窗口注册表：uuid / 标签 / 最小尺寸 / **一实例一窗的窗口族**，唯一来源）、
  `geometry.rs`（几何口径与
  `window_save.json`）、`lifecycle.rs`（句柄表 / 模型表 / 建窗 / 关窗 / 窗口事件）、
  `modpack_task.rs`（整合包安装任务注册表，`add` 与 `add_modpack` 共用）。
- **「一个实例一个窗口」走 `registry.rs` 的窗口族表** `WINDOW_FAMILIES`（日志 / 资源管理 /
  添加资源 / 配置文件四族）：窗口 uuid = 该窗口**基准 uuid** 与实例 uuid 的 UUIDv5，
  标签 `<前缀><实例uuid>`，前端认标签前缀表 `windowManager` 的 `FAMILY_LABEL_PREFIX`。
  新加一族只需在两边各补一条，按标签反查 / 按实例派生 / 最小尺寸 / 开窗 / 关窗 / 重开都跟着通；
  **尺寸只在静态条目里写一份**，族表不另存（另存就会出现"改了 `XXX_MIN_WIDTH` 而真实窗口
  纹丝不动"）；**凡是要查窗口规格的地方一律按标签查**（`min_size_of_label`），
  按 uuid 查在族内窗口上会静默退化成"查不到"。
  **这类窗口的实例参数不能省**：Rust 侧 `kind_parse_check` 只放行窗口族与导出窗口带 instance。
  窗口内全局状态要按实例归属（如 `nbt_session` 的会话），否则关一个实例的窗会打断另一个。
- 拆分时按"**一个类型 + 它的所有 `impl`**"整体搬（含反向 `From`）；只搬 `struct` 会把
  `impl` 留在原地，编译报"找不到类型 / 没有 new"。
- 被兄弟模块读到的**字段**与 **`impl` 方法**要显式 `pub(super)` / `pub(crate)`（E0616 / E0624）。
- **`include_str!` / `include_bytes!` 的相对路径**随文件位置变化：文件下沉一层就要多一个 `../`
  （踩过两次：`image_manager` 的内置预览皮肤、`custom_home` 的桥接脚本）。
- `dtos/` 也可以按用途目录化，类型在 `mod.rs` 用 `pub use` 再导出，既有路径就不会变
  （注意 §4 里"生成物顺序会变"那条）。

## 6. 临时文件

- 中间产物（下载、解包、日志、试验脚本）统一放根目录 `target/temp`（`target\` 已被 git
  忽略），不要写进版本控制；也不要放系统盘 `%TEMP%`（会被清理）。
- **测试的临时目录同样放 `target/temp`**：不要用 `std::env::temp_dir()`（系统 `%TEMP%`）——
  它会被清理，测试留下的样本下次就找不到了，散在系统临时目录里也根本没法排查。
  统一走 `mml_testutil::temp_dir()` / `temp_path(..)`（`mml-core/mml-testutil`，
  按 `[dev-dependencies]` 引入），不要在测试里自己拼 `CARGO_MANIFEST_DIR/../..`。
  - **只改测试代码**：生产代码里的临时目录（如 `base_archive` 解包时的中转目录）本来就该用
    系统临时目录，不要换成 `mml_testutil` —— 那是 dev-dependency，正式产物里不存在。
  - 每个用例**各用各的子目录**（名字带用例特征 + `Uuid` / 进程 id）：落盘是异步的，
    两个用例共用一个目录时，先跑完的那个可能在另一个 `remove_dir_all` 之后才写文件，
    于是后者读到前者的数据（表现为随机失败、单独跑就过）。
  - 内核那几个系统（`mml_base` / `mml_names` / `mml_log` / `mml_config`）是**进程级单例**，
    测试里不要各自写一份启动代码：`mml_log::STREAM` 是 `OnceLock`，第二次 `start()` 会
    **panic**。同一 crate 内共用一个 boot（见 `mml-game/src/test_support.rs`）。

## 6.1 TASK.md（临时任务文件）

- **`TASK.md` 是临时的任务清单，不是仓库文档**：一轮任务做完就**删掉**，不要把它当交接文档
  长期维护，也不要往里写"已完成"记录。
- **不要主动改 `TASK.md`**：既不改写、也不追加。只有用户明确说"更新 TASK.md / 写进 TASK.md"
  时才动它。
- 开工时它可以作为待办来源读一次；读完就按它干活，干完把文件删掉（`git rm` / 直接删）。
- 需要长期保留的约定，写进 `AGENTS.md`（本文件）；**不要**写进 `TASK.md`。

## 7. 写应用会读取的文件（重要）

- 应用读取的 JSON（`gui_config.json`、`window_save.json` 等）**必须不带 BOM**。
- 本机 `pwsh` 是 **Windows PowerShell 5.1**：`Set-Content -Encoding UTF8` / `Out-File -Encoding UTF8`
  都会写入 UTF-8 BOM，而 Rust 侧 `serde_json` 解析带 BOM 的文件会**失败**，
  表现为配置/窗口几何**静默重置**（启动后按默认值打开，关窗时只写回自己知道的那几条）。
- 写入这类文件请用无 BOM 写法：

  ```powershell
  $utf8 = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($path, $json, $utf8)
  ```

### 7.1 前端本地存储（localStorage）只放临时数据

- **`mml-vue/src/lib/storage.ts` 只放临时数据**：浏览上下文、一次性开关、丢了也无所谓的缓存
  （如上次停留的分类、搜索词、列宽）。
- **永久数据一律由 Rust 落本地文件**：用户数据（分组 / 收藏 / 顺序 / 备注）、配置，
  凡是"丢了用户会心疼"的东西都不能进 localStorage。
  - 样板见 `mml-gui/src-tauri/src/collect_utils.rs`：内存一份 `RwLock` + `init/load/set` +
    `config_save` 异步落盘，前端走一对命令整份读写。
  - 新加这类数据时：文件名加进 `mml-names` 的 `names.rs`（并在其测试列表里登记），
    落盘任务 uuid 加进 `uuids.rs`；模块放 `mml-gui/src-tauri/src/` 下（**不要**塞进 `dtos/`），
    命令与 DTO 按 §4 走生成器。
  - 理由：localStorage 跟着 webview 用户数据目录走，清缓存 / 迁移 / 换内核就没了；
    文件在运行根目录，能备份、能被其它工具读，也和内核配置放在一起。

## 8. 网页获取

- 获取网页内容一律使用 `python`（`urllib` / `requests`）或 `curl`，
  不使用其他网页抓取工具（含内置的 WebFetch 等）。

## 9. 读取文件的范围

- 只读本次任务直接相关的文件：用户在指令里点名的文件，以及为完成改动所必需的那些。
- 非必要不要扩大范围去翻别的 `.rs` 文件。确有必要时，**先说明要读哪个文件、为什么**，
  得到用户同意再读；不要静默地顺藤摸瓜。

## 10. 子代理使用

- **查找代码等探索类工作可以用子代理**（Explore 等，并行摸底、汇总结论）。
- **写计划不能使用代理**：方案设计、计划文件必须由主会话自己完成，不派 Plan 代理。

## 11. 缓存目录

- 渲染测试的运行缓存（下载的客户端 jar 等）在 `mml-core/mml-tex-draw/tests/out/run`
  （`render_all.rs` 的 `run_dir`，跨进程复用）。
- Claude 的工作缓存（反编译 jar、提取脚本等）放根目录 `target/temp`（与 §6 同源，
  不再放 H 盘）。

## 12. 前后端分工（默认只做前端，必要就一起改完）

- **默认只做前端**：界面与接线任务只改 `mml-vue/`（UI、状态、调用 `commands.xxx` 的接线）。
  "默认"是省事的起点，不是禁令——**功能确实需要后端时，就把后端一起改完，不要只留骨架**。
- **判据是"前端拿不到的能力"**：重启进程、直接读写磁盘文件、访问注册表、拉起外部程序、
  拿本机信息之类，不写后端就根本跑不通。这种情况把 `#[tauri::command]` 连同**函数体**
  一起实现，改完跑 `cd mml-gui && npm run gen`（或任意 `cargo check`）重新生成
  `bindings.ts`。
  **不要留 `todo!()` / 空实现**：那会让用户点下去的那一刻 panic、或者毫无反应，
  比不改还糟（`window_restart_app` 就是这种"必须实现"的例子）。
- **什么时候才只写骨架**：用户明确说了"只加接口 / 函数体我自己写"的时候——补
  `#[tauri::command]` 声明与 DTO 定义（含注册进 `generate_handler!`），函数体留空，
  跑生成让前端接线类型对得上。
- **用户添加 DTO 后要同步前端**：用户在 Rust 侧添加 / 修改 DTO 模型时，读取对应源码，
  跑 `cd mml-gui && npm run gen` 重新生成 `bindings.ts`，并在前端接上新结构。
- 不论哪种情况，**命令与 DTO 的唯一来源仍是 Rust 源码**（见 §4），前端不另写一套类型。

## 13. 复杂问题先写日志调试（硬性）

- **凡是「原因不明显」的问题，先加日志把链路打断点，再动手改代码**；不要靠读代码猜、
  更不要一次改多处"可能的原因"。判据很简单——**如果你需要用"可能是…"来解释现象，
  那就属于本条**（并发 / 时序 / 缓存单飞 / 中断与取消 / 跨窗口状态 / 只有用户环境才复现
  的问题，都是典型）。
- 流程固定为：

  1. **先定位**：在关键分叉处加日志（进入 / 退出 / 耗时 / 状态值），让日志把范围钉死到
     "哪一层、哪一行"；
  2. **再修复**：只改被日志指认的那一处，不做顺手重构；
  3. **后清理**：问题解决后**立即删除全部临时日志**，只保留有长期价值的运行日志。

- 日志写法约定：

  - 临时诊断日志统一带 **`[TEMP]` 前缀**，便于 `Select-String "TEMP"` 一次性找全并清除
    （与 §2「临时验证代码必须标 `TEMP` 并在验证后立即还原」同源）；
  - 用 `mml_log::info(...)`（**参数是 `String`，不是 `&str`**，字面量要 `.to_string()` 或
    用 `format!`），前端侧走 `console`；不要用 `println!`（release 无控制台，看不到）；
  - 日志要能**独立回答一个是非问题**（"走到了没" "值是多少" "耗时多久"），
    而不是打印一堆无结构的信息；
  - 涉及并发时，把**世代号 / 序号 / 任务 id** 一起打出来，否则分不清是哪一次调用。

- **长期保留的运行日志**是另一回事：像「HTTP 客户端重启」这种能解释"用户可见行为"的日志，
  属于正式日志（不带 `[TEMP]`），应当留下，方便用户贴日志排查。
- 提给用户的配合方式：让用户**复现一次并把日志贴回来**，比来回猜快得多；
  给用户判定表（"如果看到 A 说明是 X，看到 B 说明是 Y"），让一次复现就能定位。

## 14. 注释约定

**总则：精炼。** 注释只写"读代码看不出来"的东西——意图、约束、口径、为什么这么写；
**不复述代码**，也不在函数里写大段说明（长解释放文件头的 `//!` 模块文档）。一律中文。
改动代码时顺手改掉过时注释，别留下与代码矛盾的说明。
**不堆括号**：别用 `（…）` 塞口径、例外、原因、举例——要么直接写进正文，要么删掉。
一行注释至多一处括号，括号里只放非说不可的短补充。
**单位写成「，单位 X」跟在正文后**：`最大内存，单位 MB`，不写成 `最大内存（MB）`。
**控件文案不带解释**：i18n 里给控件用的那几条（标签 / 按钮 / 下拉项 / 开关名）只写它自己 ——
`自动`，不是 `自动（按版本挑）`；口径、依据、注意事项放 tooltip、说明行或详情文案。
完整句子的提示、状态、确认文案不受此限（"首次渲染会先下载核心（约 40MB）"这种补充是正常的）。

### 14.1 函数（Rust）

- 函数注释写**功能**一句话；**有参数**就逐条列 `- \`参数名\`: 说明`；**有返回值**（非 `()`）
  再写 `# 返回值`。私有小函数只留功能那一行即可，不硬凑三段。
- 函数**内部**只在逻辑不自明处加**一行**简短注释说明意图；不逐行注释、不写步骤流水账。

```rust
/// 按标签取窗口的最小客户区尺寸
///
/// - `label`: 窗口标签，形如 `mml-<kind>`
///
/// # 返回值
///
/// 见 [`MinSize`]；未注册的标签返回 `None`
pub(super) fn min_size_of_label(label: &str) -> Option<MinSize> {
    // 标签非法时按"没有历史几何"处理，默认尺寸由调用方给
    let kind = label.strip_prefix("mml-")?;
    min_size_of(&uuid_for_kind(kind)?)
}
```

### 14.2 结构体 / 枚举与字段

- 结构体、枚举用 `///` 说明**它是干什么的、在哪个链路里用**。
- **每个字段 / 变体都要有 `///`**：口径、单位、取值范围、是否可缺省。
- 有状态机含义的枚举把取值列出来（如 `state`：`waiting` / `xbox` / `token` / …）。

```rust
/// 窗口注册表条目
pub(super) struct WindowEntry {
    /// 窗口标签，形如 `mml-<kind>`，与前端 kind 对应
    pub(super) label: &'static str,
    /// 最小**客户区**宽度，单位逻辑像素；没有历史几何时就是默认宽度
    pub(super) min_width: f64,
}
```

**复合元组只准在函数体内当局部值用。** 一旦要**作为参数传入**或**作为返回值传出**——含集合
元素、`type` 别名与模块级数据（`const` / `static` 表、结构体字段）的元素类型——就定义具名结构体。
元组靠位置记含义，调用方与实现两侧都看不出 `(String, String)` 里哪个是哪个；
`(String, String, String, bool)` 这种更是只能回头翻文档。函数**内部**的中间值（去重用的
`Vec<(u8, String)>`、临时坐标对）不必为此建类型。

```rust
/// 解析出的服务器地址
pub(super) struct MotdAddr {
    /// 主机名或 IP；地址为空时是空串
    pub(super) host: String,
    /// 端口；没写端口或地址为空时取 `MOTD_DEFAULT_PORT`
    pub(super) port: u16,
}

/// - `address`: 用户输入的服务器地址，前后空白会被去掉
///
/// # 返回值
///
/// 见 [`MotdAddr`]
pub(super) fn parse_motd_addr(address: &str) -> MotdAddr { /* ... */ }
```

### 14.3 三条硬约束（写错会出事）

- **别在注释里写出 `#[tauri::command]` / `#[gui_macros::emit]` 的字面量**（`//`、`///`、
  `/* */` 都不行）：生成器是裸文本 `find(marker)`，会把注释后面第一个函数误判成命令（§4）。
- **DTO 的 `///` 与 `#[serde(...)]` 之间别插普通 `//`**：生成器回溯属性块只认 `#[` / `///`
  开头的行，插一行 `//` 会让 `rename_all` 解析不到，`bindings.ts` 字段名静默退回 snake_case。
- **临时痕迹要能一次搜净**：临时验证代码标 `TEMP` 注释（§2）、临时诊断日志带 `[TEMP]` 前缀
  （§13），用完立即删；长期保留的运行日志不带 `[TEMP]`。

### 14.4 前端（TS / Vue）

- **函数注释与 14.1 同一口径**，用 JSDoc `/** */`：写**功能**一句话；**输入**逐条列
  `- \`参数名\`: 说明`；**输出**写 `# 返回值`（组合式函数要写清返回哪几个 `ref` / 方法）。
  只服务本文件的私有小函数留一行功能即可。
- **函数内部**用 `//` 一行说明不自明的意图，不逐行注释。
- **组件要像结构体一样交代清楚**：props / emits / 具名插槽逐条 `/** */` 说明，
  口径同 14.2（含义、取值范围、是否可缺省）。
- **界面相关代码从简**（模板 / 样式 / 组件状态）：只说明"这块界面是干什么的、为什么这么排"。
  **不逐行注释模板、不解释显而易见的 CSS**（`display: flex` 这类不必说）；值得写的只有
  **反直觉点**——为什么必须 `pointer-events: none`、`z-index` 为什么取这个值、
  某个 `watch` 为什么不能省。
- 交互整块复杂时（拖拽、自绘标题栏、多窗口同步），在**文件顶部或该区块开头**用一小段说明，
  别把解释散进模板。
- 临时 `console.log` 不留仓库（eslint `no-console` 为 warn，只放行 `warn` / `error`）。

```ts
/**
 * 给本窗口挂上拖放监听
 *
 * - `onDrop`: 松手时回调，文件路径与文本地址都在里面
 * - `options.accept`: 自定义"这次拖入接不接"
 *
 * # 返回值
 *
 * - `fileDragOver`: 是否正拖在窗口上，给遮罩层用
 * - `dragKind`: 这次拖的是哪一档；空串表示未知或拖入已结束
 */
export function useFileDrop(onDrop: ..., options: FileDropOptions = {}) { ... }
```

```vue
<script setup lang="ts">
// 拖入遮罩：四个落点共用。只画外观——是否显示由调用方 v-if、标题由调用方算好传入
defineProps<{
  /** 标题，调用方已 `t()` 好的成品 */
  title: string;
}>();
</script>

<style scoped>
/* 事件穿透：拖放走窗口级 DOM 事件，遮罩不能挡住 dragover / drop */
.drop-layer {
  pointer-events: none;
}
</style>
```


