# mml-gui

M²L 启动器的 Tauri 2 桌面壳（Rust 后端 + 打包）。

## 结构

```
mml-vue  前端界面（独立 Vue 3 项目，纯浏览器可运行）
mml-gui  Tauri 壳（Rust 后端 + 打包）
mml-core 启动器内核（Rust workspace，成员与 profile 见仓库根 Cargo.toml）
```

`mml-gui` 本身不含前端代码，通过 [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) 引用 `../mml-vue`：

- `devUrl` → `http://localhost:1420`（mml-vue 的 Vite 开发服务器）
- `frontendDist` → `../../mml-vue/dist`（mml-vue 的构建产物）
- `beforeDevCommand` / `beforeBuildCommand` → 自动进入 `mml-vue` 执行 npm 脚本

## 运行

```bash
# 纯浏览器预览界面（不需要 Tauri）
cd ../mml-vue && npm install && npm run dev

# 以 Tauri 桌面应用运行（端口 1420 必须空闲）
npm install
npm run tauri dev
```

`npm run tauri` 会先跑 `npm run vendor`（见下），不用手工准备。

## 源码布局

| 路径 | 说明 |
| --- | --- |
| `src/main.rs`、`src/lib.rs` | 入口与 `run()`：注册 `mml-image` / `mml-home` 协议、窗口事件、Java 与下载器回调，再异步执行 `mml_core::load()` |
| `src/windows/<窗口>.rs` | 各窗口的规格（标签 / 标题 / 尺寸）、窗口专属数据模型与 IPC 命令 |
| `src/windows/mod.rs` | 全部窗口的创建 / 聚焦 / 关闭，窗口几何（`window_save.json`）与 GUI 配置的读写 |
| `src/dtos/` | 跨 IPC 的 DTO（磁盘配置 `snake_case` → 前端 `camelCase`） |
| `src/gui_config.rs`、`src/gui_setting.rs` | GUI 自身配置 / 实例界面设置（归属 GUI 而非内核） |
| `src/image_manager.rs` | `mml-image://` 协议：图片资源加载 |
| `src/windows/custom_home.rs` | `mml-home://` 协议：自定义主页面（zip 常驻句柄、按条目现读，不落盘解压） |
| `src/err_box.rs` | 致命错误弹窗 |
| `resources/custom_home_bridge.js` | 注入自定义主页面的桥接脚本（内置资源，不在用户包里） |
| `ipc-gen/` | IPC 代码生成器（`gui-ipc-gen`，有单测） |
| `macros/` | `#[gui_macros::emit]` 事件宏（`gui-macros`） |

## IPC 代码生成

**命令与 DTO 的唯一来源是 Rust 源码。** `src-tauri/build.rs` 调用 `ipc-gen` 扫描 `src/`，生成两个文件：

- `mml-vue/src/lib/bindings.ts`：命令的类型化包装（`commands.xxx(...)`）+ 跨 IPC 类型
- `mml-vue/src/lib/listens.ts`：事件名常量

**这两个文件是生成产物，不要手改。** 新增命令加 `#[tauri::command]`，新增事件加 `#[gui_macros::emit]`，然后重新生成：

```bash
npm run gen        # = npm run vendor && cargo check
```

命名、分组、事件名推导等规则见仓库根 [AGENTS.md](../AGENTS.md) §4。

## vendor：第三方插件还原

自绘窗口装饰依赖 `tauri-plugin-decoration`，但它需要本地改动（接受应用提供的窗口按钮矩形，
从而既保住 M²L 自己的标题栏外观、又保留 Win11 贴靠布局）。仓库里**只存 patch**，构建前由
[vendor/prepare.ps1](vendor/prepare.ps1) 按**固定 commit** 从上游克隆到 `target/vendor/` 再打补丁：

```bash
npm run vendor               # 幂等，已存在则跳过
npm run vendor -- -Force     # 强制重新拉取
```

必须早于任何 cargo 命令：cargo **解析依赖图时**就要读它的 `Cargo.toml`。各构建入口
（`build-*.bat` / npm 脚本）都已接好这一步。

## 构建

| 命令（仓库根目录） | 说明 |
| --- | --- |
| `build/build-release.ps1` | 正式发布：fat LTO + `codegen-units = 1`，含安装包。末尾的跨 crate 优化是单线程的，**构建时只有一个核在跑** |
| `build/build-prerelease.ps1` | 预发布：thin LTO + 16 个 codegen unit，只出 exe，快很多 |

两个脚本都有 bash 版（`build/*.sh`），做的是同一件事。

两者 target 目录独立（`target\release` / `target\prerelease`），来回切不会互相刷缓存。
预发布也要安装包时用
`CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 npm run tauri build`
（会与正式发布共用 target，来回切要重编末尾那几个单元）。

## 测试

```bash
cargo test -p mml-gui
```

- 内核那几个系统是进程级单例，测试共用 `src/test_support.rs` 里的一份 boot（原因见 AGENTS.md §6）。
- 测试的临时目录统一走 `mml-testutil`（落到根目录 `target/temp`），不要用系统 `%TEMP%`。
- `tests/colormc/` 是从真实 ColorMC 工作目录裁出的迁移测试样本（用例在 `src/windows/colormc.rs`），
  见该目录的 [README](src-tauri/tests/colormc/README.md)。
