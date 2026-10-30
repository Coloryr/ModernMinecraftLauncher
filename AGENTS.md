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

- **禁止过度检查 / 过度编译测试**：只跑与本次改动直接相关的最小验证——改动所在 crate 的
  `cargo check -p <crate>`、相关的那几个测试（`cargo test -p <crate> --lib`、
  `... --test <name>`，必要时加 `单个用例名` 过滤）。不要动辄
  `cargo check --workspace --all-targets` / `cargo test --workspace` / 整仓全量测试；
  同一结论不要重复跑第二遍（改了代码再验一次除外）。
- 整仓检查：根目录 `.\check.ps1 --all`（bash 版 `bash check.sh --all`）会先还原 vendor 插件；
  裸 `cargo check --all` 前必须已还原过 `target/vendor/tauri-plugin-decoration`，Cargo 没有可用的
  构建前钩子，别指望它自己准备。
- 桌面壳：`cd mml-gui && .\tauri.ps1 dev`（Linux / macOS 用 `bash tauri.sh dev`；会先还原 vendor 插件再起 mml-vue 的 vite，**端口 1420 必须空闲**）。
- 只跑前端：`build/dev-frontend.ps1`（bash 版 `build/dev-frontend.sh`；vite，1420）。
  只在 Rust 后端以 debug 模式单独运行（`cargo run`）时配合使用；`tauri dev` 已自带 vite，
  不要同时起。浏览器直连只能看到无 IPC 数据的空状态。
- **出包两个 profile**（各自独立 target 目录，来回切不会互相刷缓存）：
  - 正式发布：`build/build-release.ps1`（bash 版 `build/build-release.sh`；
    = `cd mml-gui && cargo tauri build`）。Windows 默认三种架构（x64 / x86 / arm64），
    只要一种用 `-Arch x64`；arm64 只打 NSIS。
    `[profile.release]` 是 fat LTO + `codegen-units = 1`：体积最小、运行最快，但末尾的
    跨 crate 优化是单线程的，**构建时会看到只有一个核在跑**，含安装包。
  - 预发布：`build/build-prerelease.ps1`（bash 版 `build/build-prerelease.sh`；
    = `cd mml-gui/src-tauri && cargo build --profile prerelease --features custom-protocol,prerelease`）。
    **`prerelease` feature 也不能漏**：主页面据它显示「预发布版」提示，见 `src-tauri/src/build_info.rs`。
    三个通道的界面口径：直接拉源码启动（`cargo run` / `tauri dev`，`debug_assertions`）是
    「测试版」，带 `prerelease` feature 的是「预发布版」，正式发布不提示。
    **`--features custom-protocol` 不能漏**：tauri 用它决定前端走内嵌 dist 还是 devUrl，
    漏了即使 profile 是 release-like 也会被当成 dev 构建，exe 去连 localhost:1420，
    没起 vite 就是"进程在跑、窗口不出来"。
    `[profile.prerelease]` 是 thin LTO + `codegen-units = 16`：快很多，体积略大，只出 exe。
    Linux / macOS 的各架构由 CI 出：Linux x64 / arm64、macOS x64 / arm64 都是原生 runner，
    只有 Linux x86 交叉编译（见 `.github/workflows/build.yml`）。
    Windows 默认三种架构，产物按架构命名：`target/prerelease/mml-gui-<arch>.exe`（`-Arch` 同样可过滤）。
  - 预发布也要**安装包**时：`tauri` CLI 不认 `--profile`（`--` 透传给 cargo 会让它去
    `target/release` 找错文件），只能覆盖 release 的设置：
    `CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 cargo tauri build --features prerelease`。
    PowerShell 里环境变量写成 `$env:CARGO_PROFILE_RELEASE_LTO = 'thin'` 单独两行再跑
    `cargo tauri build --features prerelease`；`--features prerelease` 不能省，
    否则这样出的包与正式版一模一样，主页面不会提示「预发布版」。
    代价是它与正式发布共用 target 目录，来回切会重编末尾那几个单元。
- Rust workspace 在**仓库根目录**（根 `Cargo.toml` 收编 `mml-core/` 全部子 crate 与
  `mml-gui/src-tauri` + `ipc-gen` / `ipc-gen/macros`；`workspace.dependencies` 与 `[profile.*]`
  也统一在根清单维护）。target 目录在根目录 `target\`（首次编译较慢）。

## 4. IPC 约定

- 命令名 = 函数名 = **`窗口名_方法名`**，例如 `main_get_instances`、`account_add_account`、
  `window_open_window`、`add_list_dir`。
- **命令与 DTO 的唯一来源是 Rust 源码**，全部由生成器产出。前端缺命令 / 缺字段时，
  先在 Rust 侧补 `#[gui_macros::command]` 或改 DTO 结构，再跑生成；**不要**在 `bindings.ts`
  或前端另写一套类型（下次生成会被覆盖，且与 Rust 侧脱节）。
  `src/lib/api.ts` 只是手写的便捷包装层，其中的类型一律 `import` 自 `./bindings`。
- 事件名由 `emit_xxx_yyy` 推导：去掉 `emit_` 前缀、`_` 换成 `-`，例如
  `emit_account_change` → `account-change`。
- 前端契约由 `mml-gui/src-tauri/build.rs`（逻辑在仓库根 `ipc-gen/`）扫描 Rust 源码自动生成，
  产物在独立的 npm 包 `mml-ipc/src/`（**勿手改**）：
  - `types/`：DTO，按来源 .rs 一个 .ts，`types/index.ts` 是 barrel；跨文件引用自动补 `import type`
  - `bindings.ts`：`createCommands(invoke)` 命令包装，再导出 `types/`
  - `listens.ts`：事件名常量、`EventPayloads` payload 映射与 `createEvents(on)` 订阅函数
  命令签名、DTO 结构、`#[serde(rename_all)]` 与事件 payload 都从 Rust 源码解析（事件 payload
  取的是发射函数体里 `emit` / `emit_to` 的实参，不是签名参数）。
  自定义主页面的 `window.mml` 契约（`MmlApi`、可订阅事件与 payload 映射，与 mml-vue 的
  `customHomeBridge.ts` 白名单对应）是手写的 `mml-ipc/src/home.ts`，不参与扫描。
  自定义主页面示例直接用 `mml-ipc`；已经依赖 mml-vue 的主题可从 `mml-vue/ipc` 取同一契约。
  `mml-vue/src/lib/bindings.ts` / `listens.ts` 只是兼容层：前者把 Tauri 的 invoke 绑到
  `createCommands`，后者再导出事件名，既有导入路径不变；`custom-main-demo` 直接引用 `mml-ipc`。
  新增命令加 `#[gui_macros::command]`，新增事件加 `#[gui_macros::emit]`；单独重新生成可跑
  `cd mml-gui && cargo check --manifest-path src-tauri/Cargo.toml`（或仓库根 `cargo check -p mml-gui`）。
- 扫描与生成逻辑在 `ipc-gen/`（包名 `gui-ipc-gen`，有单测），过程宏在 `ipc-gen/macros/`
  （包名 `gui-macros`）：`src-tauri/build.rs` 只是调用方（算路径、传 `GenConfig`、写文件、
  打 cargo 指令）；改生成规则改那个包，跑 `cd ipc-gen && cargo test` 验证。
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
- 扫描是**裸文本**：注释里写出 `#[gui_macros::command]` / `#[gui_macros::emit]` 的字面量，
  会让紧随其后的函数被误判成命令（实测踩过：`generate_handler!` 里出现不存在的
  `__cmd__xxx`，编译失败）。别在注释里写它们。
- **命令的可见性不能降**：`#[gui_macros::command]` 展开成 `#[tauri::command]`，生成的
  `__cmd__xxx` / `__tauri_command_name_xxx` 会继承函数可见性，把命令写成 `pub(super)` 后
  `generate_handler!` 从 crate 根引用不到
  （E0603 "macro import is private"）。搬命令时统一 `pub(super) fn` 的脚本要把它排除掉。
- **集成测试按原路径引用命令**：`src-tauri/tests/*.rs` 里 `use mml_gui_lib::windows::<窗口>::<命令>`
  的地方，命令拆进子模块后要在 `mod.rs` 再导出（`pub use self::<子模块>::<命令>;`），
  否则 `cargo check --all-targets` 才报错（只跑 `cargo check -p mml-gui` 是发现不了的）。
- **DTO 换文件会改生成物的顺序**：生成器按**排序后的文件路径**遍历 `dtos/`，把一个 DTO 文件拆成
  目录后，`bindings.ts` 里类型声明的先后会变（内容不变）。判定"没改契约"要比对**类型块集合**
  （每个 `export type X = {...}` 归一化后按名字比），而不是看 diff 行数。
- 磁盘配置的 JSON 键统一用**大驼峰**（如 `MainWindow` / `SelectedInstance`），
  跨 IPC 传输一律经 `src-tauri/src/dtos/` 转成 camelCase DTO。
- **实例列表跨窗口同步走 `instance-change`**：实例的增删只发生在主窗口，其余窗口的下拉是打开时
  拉的一份快照，"别的窗口删了、这里还留着"就会点出一个不存在的实例。凡是显示实例列表的窗口
  （导出、生成服务器实例、设置、收藏、大厅、建筑渲染、下载资源……）都挂一行
  `useInstanceChange(...)`（`src/composables/useInstanceChange.ts`）：事件来了重拉列表，
  当前选中的实例没了就清空 / 回落到第一个，别只重拉列表、留着失效的选中项。
  挂在实例名下的四类窗口由后端 `close_instance_windows` 直接关掉，不用再自己判断。
- **删除的界面处理放在删除开始时**：挪回收站要几十秒，别等它跑完再更新列表 / 关窗口。
  `main_delete_instance` 先用 `windows/main/deleting.rs` 的守卫标记、广播、关窗，再删文件；
  列表命令据这个标记过滤（`main_get_instances`），删失败时撤销标记并再广播一次把实例放回来。
  标记守卫**只能在释放锁之后构造**：它的 `Drop` 会去拿同一把锁，边持锁边丢就是自死锁。
- **实例被占用时禁止删除 / 改名**：判据集中在 `windows/main/instance/group.rs` 的 `busy_of`
  （运行中 / 下载 / 资源窗口操作 / 添加资源下载 / `windows/instance_task.rs` 的长任务），
  `ensure_idle` 与实例列表 DTO 的 `busy` 都用它，别在别处另写一套判断。
  导出、生成实例信息、生成服务器实例这类会整份读实例目录的命令，开头挂
  `instance_task::begin(&uuid)?`（守卫活到任务结束），正在删除的实例也会被它拒绝。
  `busy_of` 不锁实例本身：`main_get_instances` 是在持有实例读锁时调它的。

## 5. 目录分工

| 目录 | 职责 |
| --- | --- |
| `mml-core/` | 启动器内核（多 crate，隶属根目录 workspace） |
| `mml-gui/` | Tauri 桌面壳；`src-tauri/src/windows/<窗口>.rs` 放该窗口的规格 / 模型 / IPC |
| `mml-vue/` | 前端（Vue3 + Vite）；窗口在 `src/windows/<kind>/`，通用组件在 `src/components/`；`scripts/gen-ui.ts` 生成 `src/generated` 的 UI 出口；mml-ipc 与 mml-vue 都发布到 npm |
| `ipc-gen/` | IPC 代码生成器（`gui-ipc-gen`）+ 过程宏（`ipc-gen/macros`，`gui-macros`） |
| `mml-ipc/` | npm 契约包：按来源拆分的跨 IPC 类型、命令包装、事件订阅函数、自定义主页 `window.mml` 契约（`src/home.ts`）与自定义界面注册表 / 上下文（`src/customUi.ts`） |

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

## 6. 临时文件与缓存目录

> 原 §11「缓存目录」已并入本节；本节之后各章的编号保持不变，引用仍写 §12.1 / §13 等。

### 6.1 临时文件

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

### 6.2 缓存目录

- 渲染测试的运行缓存（下载的客户端 jar 等）在 `mml-core/mml-tex-draw/tests/out/run`
  （`render_all.rs` 的 `run_dir`，跨进程复用）。
- 工作缓存（反编译 jar、提取脚本等）放根目录 `target/temp`，与其他临时文件同源，
  不要另建目录，也不要放系统盘。
- 判据：**`target/temp` 放这一轮用完就能删的；`tests/out/run` 放删了要重下一遍、
  跨进程复用的。** 拿不准时按前者处理。

### 6.3 TASK.md（临时任务文件）

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

## 12. 前后端分工（默认只做前端，必要就一起改完）

- **默认只做前端**：界面与接线任务只改 `mml-vue/`（UI、状态、调用 `commands.xxx` 的接线）。
  "默认"是省事的起点，不是禁令——**功能确实需要后端时，就把后端一起改完，不要只留骨架**。
- **判据是"前端拿不到的能力"**：重启进程、直接读写磁盘文件、访问注册表、拉起外部程序、
  拿本机信息之类，不写后端就根本跑不通。这种情况把 `#[gui_macros::command]` 连同**函数体**
  一起实现，改完跑 `cd mml-gui && cargo check --manifest-path src-tauri/Cargo.toml` 重新生成
  `bindings.ts`。
  **不要留 `todo!()` / 空实现**：那会让用户点下去的那一刻 panic、或者毫无反应，
  比不改还糟（`window_restart_app` 就是这种"必须实现"的例子）。
- **什么时候才只写骨架**：用户明确说了"只加接口 / 函数体我自己写"的时候——补
  `#[gui_macros::command]` 声明与 DTO 定义（含注册进 `generate_handler!`），函数体留空，
  跑生成让前端接线类型对得上。
- **用户添加 DTO 后要同步前端**：用户在 Rust 侧添加 / 修改 DTO 模型时，读取对应源码，
  跑 `cd mml-gui && cargo check --manifest-path src-tauri/Cargo.toml` 重新生成 `bindings.ts`，并在前端接上新结构。
- 不论哪种情况，**命令与 DTO 的唯一来源仍是 Rust 源码**（见 §4），前端不另写一套类型。

### 12.1 前端只留这份界面要用的数据，其余放后端的窗口 model

- **webview 前端只存"这个界面当下渲染要用"的东西**：列表的一份副本、选中项、展开 / 折叠、
  输入框草稿这类。**跨界面复用、或要从零算一遍的数据，一律放后端的窗口 model**，
  按窗口隔离，见 `mml-gui/src-tauri/src/windows/lifecycle.rs` 的窗口模型表
  （表在 `WINDOW_MODELS`，每个窗口一个模型，代码放该窗口目录下的 `model.rs`）。
- **判据**："换个界面回来还要用吗？重新算一遍贵吗？" —— 任一为是，就不是前端该留的。
  命令按前端这一屏需要什么返回什么，别把整份数据推过去让前端自己筛、自己拼。
- **例：模组关系不能存在前端。** 谁依赖谁、前置齐不齐、版本合不合，都是内核扫描结果算出来的，
  前端的模组列表只是那一趟扫描的副本。**查看模组依赖一律请后端给树状图**
  （`resource_*` 命令返回画图要的节点 / 边），前端不自己建图、也不把关系表缓在内存里。
- **例：模组扫描结果存窗口 model**，见 `windows/resource/model.rs`：类型 / 元数据 / 在线信息
  存一份，更新检查、依赖检测直接读，不重扫 `mods` 目录。
- 窗口 model 跟窗口生命周期走：开窗后第一次用到时建立，**关窗前把里面在跑的任务全部取消**
  （扫描 / 网络 / 下载这类慢操作都要能在窗口关掉时收手），窗口销毁时释放。

### 12.2 自定义界面（三种换皮，互相独立）

| 种 | 作用 | 产物 | 开关 | 设置位置 |
| --- | --- | --- | --- | --- |
| 1 自定义主页面 | 主窗口主页内容区 | `home`（`index.html` 或 `custom.js`） | `client.custom_home` | 客户端设置 |
| 2 自定义 CSS | 内置界面换皮，可装多份 | `custom_ui/css/<id>.zip`，包根 `custom.css`，可选 `custom.js` 入口 | `client.custom_css_on` 加每条 `enabled` | 界面 |
| 3 完全自定义界面 | 整个主窗口换成整页 / 按 kind 换窗口组件 | `custom_ui/page.zip`（`index.html` 或 `custom.js`） | `client.custom_page_on` | 界面 |

- **三者互不联锁**：装 1 不动 2 / 3 的开关，装 3 也不动 2；别把它们做成一个三选一模式。
  3 开着时 1 的页面在它后面，属正常叠加；2 只作用于启动器内置界面，不进 iframe。
- 1 的压缩包走 `mml-home`，2 的每份 CSS 与 3 的整页包走 `mml-ui`。整页包与 1 共用
  `windows/web_assets.rs` 的整包句柄 / 越界防护 / 限额，新加协议直接复用，别另抄一遍
  `..` 检查。
- 2 的条目列表与两个开关都存 `gui_config.client`，列表只在设置窗口写；文件本身在
  `custom_ui/`。**不要把 CSS 文本或整页 HTML 塞进配置文件**。
- **三种都打包成 zip 分发**：1 的包根是 `index.html` 或 `custom.js`（可选 `custom.css`），
  2 的包根是 `custom.css`（可选 `custom.js`），3 的包根是 `index.html` 或 `custom.js`；
  整包套一层目录也认。
- **导入先校验、再替换**：1 确认 `index.html` 或 `custom.js`，2 确认 `custom.css`，
  3 确认 `index.html` 或 `custom.js`；
  3 落盘走 `.tmp` 中转再 rename；
  2 的每条按 uuid 落成 `css/<id>.zip`，删除时先删文件再动配置。
- **包内静态资源同源伺服**：CSS 包的 `/css/<id>/<路径>` 与整页包共用 `resolve_entry`，
  CSS 里用 `url(assets/bg.png)` 相对引用即可，不要求用户把图片塞进主题变量。
- **可选入口脚本**：2 的包根可以有 `custom.js`，`custom_ui_status` 每条多给 `scriptUrl`
  （没有就空串）。`lib/customUi.ts` 先挂 `<link>`，再把脚本作为 classic script 插进页面；
  脚本执行时同步 `window.__mmlCustomCss.push(fn)` 注册，`document.currentScript` 的
  `data-custom-ui-id` 决定属于哪个包。`main.ts` 在 `app.mount` 之后调 `startCustomCss()`
  才执行入口，入口拿到 `loadCss / select / replace / onCleanup`；关包 / 换脚本时跑清理。
  `replace` 除改文字 / 类名外还能给 `component` + `props`：原控件隐藏，用 `createApp`
  把 Vue 组件挂到旁边，关包时 unmount 并还原。组件由 Vite 按 IIFE 打进 `custom.js`，
  `vue` external 到 `window.__mmlVue`（`main.ts` 暴露），包里不重复打一份 Vue。
  入口脚本本身仍不要用 `import` / `export`（构建产物是 IIFE），否则
  `document.currentScript` 为 null，注册表分不清包。
- **自定义主页组件回调**：1 的包根可以有 `custom.js`（返回主页内容组件的回调）与
  `custom.css`（样式）；`custom_home_status` 多给 `scriptUrl` / `styleUrl`。
  `lib/customHomeUi.ts` 只在主窗口、挂载前加载脚本并装 host 版 `window.mml`，脚本用
  `window.__mmlHomePages.push(() => Component | null)` 注册；`CustomHomePage.vue` 有组件就
  渲染组件，没有就回落到 iframe。组件样式由示例的 PostCSS 前缀限定在 `.mml-home` 内。
- **自定义界面契约在 mml-ipc**：`WindowPageRegistry` / `HomePageRegistry` /
  `CustomCssContext` / `CustomCssReplaceOptions` 的定义都在 `mml-ipc/src/customUi.ts`，
  mml-vue 与前三个组件示例都按 type-import 取；整页 HTML 示例只用运行时 `window.mml`。
- **整页窗口回调**：3 的包根可以有 `custom.js`（窗口页面组件）与 `custom.css`（其样式）；
  `custom_ui_status` 多给 `pageScriptUrl` / `pageStyleUrl`。`lib/customUi.ts` 在挂载前
  **await** 加载脚本并装 host 版 `window.mml`（invoke / listen / theme），脚本用
  `window.__mmlWindowPages.push((kind) => Component | null)` 注册；`App.vue` 先按 kind
  取替换组件，取不到再走 iframe / 内置窗口。组件直接跑在 host 里，Vue 从 `__mmlVue` 取，
  所以挂载前就能替换，不会先闪一下内置页面。关包 / 换包时移除脚本与样式、清回调。
  `App.vue` 还把内置窗口表挂到 `window.__mmlWindows`，主题组件可以直接渲染
  `window.__mmlWindows.main` 复用原始主窗口，再自己加内容，不必把 MainWindow 打进主题包。
- **事件分工**：开关与列表变更走 `window_save_gui_config` → `client-config-change`；
  文件导入 / 删除单发 `custom-ui-change`。前端 `lib/customUi.ts` 两边都监听后重拉
  `custom_ui_status`。
- **启动顺序**：`main.ts` 必须在 `createApp().mount()` 之前 `await initCustomUi()`，
  否则整页模式会先闪一下内置界面。状态只来自后端，
  **不要把开关 / 列表 / 地址写回 localStorage**。
- **安全底线**：整页 iframe 保持 `sandbox` 且**不给 `allow-same-origin`**；`mml-ui` 只伺服
  `custom_ui/` 与包内条目，CSS 路径的 id 必须是 uuid，整页路径走 `resolve_entry`。
- **锁定功能预留**：后续要给服主 / 二次开发者一个锁住这三项的开关，加在 `ClientConfig`
  即可；届时要拦两处 —— 改开关 / 列表走 `window_save_gui_config`，文件操作走
  `custom_ui_*` 命令，别把判断散到前端。
- **控件只有一个来源**：`mml-vue/scripts/gen-ui.ts` 在 `mml-vue/src/generated` 生成
  `ui-kit.ts` / `ui-kit.css` / `controls.ts` / `windows.ts` / `components.ts`，由 `mml-vue`
  的 exports 暴露成 `mml-vue/ui-kit`、`mml-vue/ui-kit.css`、`mml-vue/ui-kit/controls`、
  `mml-vue/ui-kit/windows`、`mml-vue/ui-kit/components`。组件 / CSS 两个示例按
  `file:.mml-deps/mml-vue` 依赖（`scripts/deps.mjs` 在 install 前链到仓库，仓库不在就浅克隆），
  mml-ipc 与 mml-vue 都会发布到 npm。改完控件或样式重跑 `node mml-vue/scripts/gen-ui.ts`；
  新加控件加进 `COMPONENTS`。`./controls` 单独一个入口，是为了让 Node 只取清单时
  不必加载 `.vue`。
- **两个示例怎么用 kit**：自定义 CSS 的 `scripts/controls.ts` 读 `CONTROLS` / `CSS_VARIABLES`
  列选择器、生成覆盖骨架，`src/replace.ts` 是运行时入口源码（打包时编译成 `custom.js`），
  演示「选一个控件并换掉文字与外观」；
  整页示例 `import { KitButton, KitSearchBox, ... } from "mml-vue/ui-kit"`
  并引 `mml-vue/ui-kit.css`，启动进度用 `KitLaunchProgress`，`v-tip` 直接用导出的真实指令；
  自定义界面类型（`CustomCssContext` / `WindowPageRegistry` / `HomePageRegistry`）直接从 `mml-ipc` type-import。
- **`mml-vue` 是 `file:` 符号链接**：整页示例用 `resolve.dedupe: ["vue"]` 加 tsconfig
  `paths.vue`，所有 `.vue` 都用示例自己的 Vue；CSS 示例只取清单与类型。
- **CSS 示例的源码不进包**：`src/` 与 `scripts/` 都不进 zip；`pack.ts` 先跑
  `vite build` 把 `src/replace.ts` + `ReplaceButton.vue` 打成 IIFE `dist/custom.js`，
  复制到包根后与 `custom.css` 一起打包，打完删掉 `custom.js` 与 `dist`。
- **恢复路径**：整页页面自己挂掉时删掉 `custom_ui/page.zip` 或关掉 `customPageOn`；
  开发构建的整页右下角另有「退出整页界面」按钮。

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

### 14.3 四条硬约束（写错会出事）

- **别在注释里写出 `#[gui_macros::command]` / `#[gui_macros::emit]` 的字面量**（`//`、`///`、
  `/* */` 都不行）：生成器是裸文本 `find(marker)`，会把注释后面第一个函数误判成命令（§4）。
- **DTO 的 `///` 与 `#[serde(...)]` 之间别插普通 `//`**：生成器回溯属性块只认 `#[` / `///`
  开头的行，插一行 `//` 会让 `rename_all` 解析不到，`bindings.ts` 字段名静默退回 snake_case。
- **临时痕迹要能一次搜净**：临时验证代码标 `TEMP` 注释（§2）、临时诊断日志带 `[TEMP]` 前缀
  （§13），用完立即删；长期保留的运行日志不带 `[TEMP]`。
- **注释里不写彩蛋**：愚人节那类隐藏功能只按**行为**描述（如"标题栏按另一套样式展示"），
  不出现"彩蛋 / 愚人节 / 4 月 1 日"这类字样 —— 写明白等于提前泄露，
  也会让 `grep 彩蛋` 这种排查把普通逻辑一起拖进来。

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

## 15. 文案与日志的国际化（硬性）

**所有会被人看到的字都要能切语言**：前端界面文案、日志（`[TEMP]` 临时诊断除外）一律走各自的
i18n 表，**不写死中文或英文字面量**。三张表各管一摊，改哪一层就改哪张表：

| 层 | 表 | 写法 |
| --- | --- | --- |
| 前端界面 | `mml-vue/src/lib/i18n/locales/{zh-CN,en-US}.ts` | `t("winSettings.xxx")` |
| 内核日志 / 错误 | `mml-core/mml-names/src/i18_items/` 与 `i18/{zh_cn,en_us}.rs` | `mml_log::info_type(InfoType::X)` / `error_type(ErrorType::X)` |
| GUI 日志 | `mml-gui/src-tauri/src/log_text.rs`（自带中英两套） | `mml_log::info(LogText::X { .. }.text())` |

- **两种语言一起加。** 内核的 `zh_cn.rs` / `en_us.rs` 与 `LogText` 的两个 match 都是穷尽的：
  加了变体不加对应分支**编译不过**，所以别绕过它去写 `format!`。
- **新加变体，不要往现成变体里塞别的语义。** 带数据的（文件名、地址、数量、次数）用结构体字段传进去，
  别在调用处把数据拼成字符串再套一条通用文案。
- **判据**：这条字会不会出现在用户界面上，或者会不会被用户贴进 issue 的 `logs.log` 里？
  会 → 必须 i18n。只有 `[TEMP]` 临时诊断（§13）和 debug 级日志可以写字面量。
- **不翻译机器标识**：URL、文件名、SHA1、JSON 字段名、协议名、异常原文原样进日志，翻了反而没法排查；
  它们周围那句人话照样 i18n。
- 前端**不要**在模板里写中文（`<button>启动</button>`），tooltip / 占位符 / aria-label 同样走 `t()`。

## 16. 前端样式统一（硬性）

**一套外观只有一个来源。** 窗口的样式文件只写"这一屏怎么排"，不写"控件长什么样"。
颜色、控件尺寸、圆角、字号全部取变量与 `styles/`，不要在窗口里就地发明一套。

### 16.1 四个来源，各管一摊

| 位置 | 管什么 |
| --- | --- |
| `styles/themes.css` | 颜色 / 阴影 / 遮罩 / 跨窗口共用的固定尺寸（`--titlebar-h`、`--rail-w`） |
| `styles/controls.css` | 控件外观唯一来源：`--ctl-h` / `--ctl-radius` / `--ctl-font`，以及 `.field-*` / `.ui-btn` / `.ui-btn-icon` / `.page-btn` / `.empty-tip` |
| `styles/` 其余（base / modal / titlebar / tip / scrollbar / skeleton / decoration） | 全局行为：重置、过渡、弹窗、标题栏、提示、滚动条、骨架 |
| `windows/<窗口>/<窗口>.css`、组件 `<style scoped>` | 只放这一屏的排版与独有装饰 |

- 新加全局样式写进对应文件，并在 `styles/index.css` 注册；不要"在某个窗口里定义，指望别的窗口也能用到"。
- 跨窗口共用的尺寸（标题栏高度、分类栏宽度）必须提成 `themes.css` 的变量：两个窗口并排看，
  数字不一致就是 bug。
- 判断新样式该放哪：**换个窗口还用得上吗？** 用得上 → `styles/`；只属于这一屏 → 该窗口的 css。

### 16.2 颜色只用变量

- 除 `themes.css` 外**不写 `#rrggbb` / `rgb()` / `hsl()` 字面量**，一律 `var(--xxx)`。
- 主题变量要**成对加**：Dark 与 Light 两块都写，并各补一行用途说明。只加一块，
  另一套主题下就会静默取不到值。
- 半透明表面用 `color-mix(in srgb, var(--bg) 85%, transparent)`，或现成的 `--accent-soft` /
  `--overlay` / `--bg-modal` / `--bg-modal-raised`，不要自己调 rgba。
- 状态色只有 `--green` / `--red` / `--yellow` 与 `--accent*` 几组；要新色先加进主题，
  不要在窗口里写一个"差不多"的颜色。

### 16.3 字号 / 圆角 / 间距只有这几档

现状是字号出现 20 种取值、圆角 15 种。**新代码只用下表的值**；改到某个文件时，
顺手把该文件里的零散值归并到档位上。

字号：

| 档 | 值 | 用途 |
| --- | --- | --- |
| caption | 10.5px | 角标、徽章 |
| meta | 11.5px | 次要说明、表头、时间 |
| body | 12.5px，即 `--ctl-font` | 正文、控件、列表项 |
| title | 13px | 卡片名、小标题 |
| heading | 15px | 页面标题、`lg` 主操作按钮 |
| display | 18px | 只给空状态、首屏引导 |

圆角：

| 值 | 用途 |
| --- | --- |
| 8px，即 `--ctl-radius` | 控件：输入框、下拉、按钮 |
| 10px | 卡片、列表项 |
| 12px | 面板、弹窗、大容器 |
| 999px | 胶囊：标签、开关滑块 |
| 50% | 圆形：头像、状态点 |

间距：2px 网格，只用 2 / 4 / 6 / 8 / 10 / 12 / 14 / 16 / 20 / 24 / 32。
表格行那种密集处可以用 `4px 8px` 这样的组合，但每个数仍取上面的档。

### 16.4 一个视觉角色只定义一次

- 同一外观出现第二处就抽出来：控件类进 `controls.css`，整块界面进 `styles/parts/<域>.css`。
  现状里 `.progress-track` / `.progress-fill` 有 5 份、`pack-*` 一整套在两个窗口各写一遍、
  `.icon-btn` 有 3 份 —— 这类重复要收敛，**不要继续复制**。
- 进度条统一 `.progress-track` + `.progress-fill`，**尺寸只允许两档**：8px（卡片、弹窗内）与
  6px（表格行内等密集处）。要第三档先改本节。
- 抽出来的类在定义处写一行用途与尺寸口径，别再让调用方自己猜。

### 16.5 按钮与表单只用既有控件

- 按钮 / 开关 / 分段 / 步进器 / 弹窗 / 折叠 / 搜索框：`BaseButton`、`BaseSwitch`、
  `SegmentedTabs`、`NumberStepper`、`BaseModal`、`CollapsePanel`、`SearchBox`；
  裸标签写 `.ui-btn` + `.v-*` + `.s-*`。
- 表单行：`.field-label` / `.field-input` / `.field-select` / `.field-hint` /
  `.field-row-2` / `.path-row` / `.is-invalid`。
- **存量类名不许新增**：`.mini-btn`、`.icon-btn`、`.refresh-btn` 那一批已列在
  `controls.css` 的头注释里。改到该文件时顺手换成上面的写法；某个类的最后一个使用点被换掉时，
  把它的定义与清单条目一起删。
- 悬停反馈只有两种：`.ui-btn` 的底色变化、存量类的描边变强调色。不要发明第三种。

### 16.6 层级与动效

- `z-index` 只用下表档位，不许就地写数字；出现新角色先加进这张表再写代码：

| 值 | 用途 |
| --- | --- |
| -1 | 背景图层 |
| 1–10 | 窗口内的粘性元素：表头、工具条 |
| 20 / 40 | 窗口内浮层：下拉、右键菜单、浮动卡 |
| 100 | 弹窗内的浮层 |
| 200 | 模态遮罩与弹窗本体 |
| 250 | 挂在 body 上的弹层（如日期选择器月历）：要盖过模态里的控件 |
| 300 | 全局提示、tooltip |
| 999 以上 | 只给必须压住一切的极少数，如启动画面，并在代码里写一行说明为什么 |

- 过渡时长三档：`0.12s`（颜色、边框这类小反馈）、`0.15s`（背景、位移、缩放）、
  `0.18s`（浮层展开）；宽度 / 高度这类形变用 `0.3s`。不要随手写 0.2s / 0.25s。
- 关闭"界面动画"由 `base.css` 的 `html.no-anim *` 全局接管，组件里不要再写判断或自建开关。

### 16.7 命名与作用域

- 类名小写 kebab-case，按**角色**命名（`.pack-card`），不用 `.left`、`.red`、`.box2`。
- 状态类 `is-*`（`.is-invalid`、`.is-active`），修饰类 `v-*`（`.v-primary`），
  尺寸类 `s-*`（`.s-sm`）。
- 组件样式默认 `<style scoped>`；要穿透子组件用 `:deep()`，并在旁边写一行说明为什么必须穿透。
- 组件自己的样式跟着组件走，不要为了"好找"塞进窗口 css；窗口 css 只放多个组件共用的排版。
- 内联 `:style` 只用于运行时算出来的值（进度宽度、拖拽位移）；静态外观一律进样式表。
- `!important` 只允许出现在 `styles/` 的全局覆盖里（主题、`no-anim` 这类），组件里不许用。

### 16.8 检查

- `cd mml-vue && npm run lint` 必须 **0 error**。`vue/no-mutating-props` 是已知的共享设置对象
  模式，不新增即可；其它新增 warning 不要放着不管。
- 样式没有自动检查，靠本节加 review：改完对一眼该文件的字号 / 圆角 / 间距 / 颜色是否都在档位上，
  有没有把同一套外观又抄了一遍。
