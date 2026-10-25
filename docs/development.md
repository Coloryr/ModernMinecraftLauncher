# 开发与构建

写给要改代码、跑起来或自己出包的人。界面用法见[软件手册](README.md)；
协作约定（IPC 生成、窗口拆分、临时目录、日志调试、样式规范等）统一在仓库根目录 [AGENTS.md](../AGENTS.md)。

## 技术栈

- **内核**：Rust workspace（`mml-core/` 本体 + 18 个子 crate，edition 2024）
- **界面**：Tauri 2 桌面壳（`mml-gui/`）+ Vue 3 / Vite 前端（`mml-vue/`）
- **IPC**：Rust 源码是命令与 DTO 的唯一来源，`ipc-gen` 生成独立的 npm 契约包 `mml-ipc`，
  启动器前端与自定义主页面共用
- **渲染**：wgpu（方块 / 物品图标、皮肤与披风 2D / 3D）+ glow（皮肤模型预览）

## 目录结构

| 目录 | 职责 |
| --- | --- |
| `mml-core/` | 启动器内核，多 crate：游戏启动、版本管理、Mod 加载器、整合包、下载、认证、Java 管理、网络 API（Mojang / Modrinth / CurseForge / Fabric 等）、皮肤与贴图渲染、NBT 解析等 |
| `mml-gui/` | Tauri 桌面壳；窗口规格与 IPC 命令在 `src-tauri/src/windows/`，命令与 DTO 由 `ipc-gen` 扫描生成到 `mml-ipc` |
| `mml-vue/` | 前端（Vue3 + Vite）；窗口在 `src/windows/`，通用组件在 `src/components/` |
| `ipc-gen/` | IPC 契约生成器（`gui-ipc-gen`）与过程宏（`ipc-gen/macros`，`gui-macros`） |
| `mml-ipc/` | npm 契约包：按来源拆分的跨 IPC 类型、命令包装、事件订阅函数与自定义主页 `window.mml` 契约，`mml-vue` 与 `custom/` 下的示例共用 |
| `custom/` | 四个自定义界面示例：`custom-main-demo/`（自定义主页面）、`custom-ui-demo/`（整页主题 · 组件）、`custom-ui-html-demo/`（整页主题 · HTML）、`custom-ui-css-demo/`（自定义 CSS）；各自构建出可直接导入启动器的 zip，用法见 [custom/README.md](../custom/README.md) |
| `build/` | 版本文件、版本同步 / 打包辅助脚本，以及初始化 / 开发 / 构建脚本 |
| `design/` | 程序图标的 SVG 源文件；改完用 `tauri icon` 重新生成各平台图标 |

各子项目的内部目录（`mml-core` 的各个 crate、`mml-gui` / `mml-vue` / `mml-ipc` 的子目录等）
见根 README 的[项目结构](../README.md#项目结构)。各子项目另有 README：[mml-gui](../mml-gui/README.md)、
[mml-vue](../mml-vue/README.md)、[mml-ipc](../mml-ipc/README.md)、[custom](../custom/README.md)、
[mml-theme-studio](../mml-theme-studio/README.md)。

## IPC 契约：Rust 是唯一来源

```text
mml-gui/src-tauri/src/            ipc-gen/                 mml-ipc/src/
  命令 #[gui_macros::command] ──▶ 扫描 + 生成 ──▶ bindings.ts（createCommands + DTO）
  事件 #[gui_macros::emit]                        listens.ts（事件名常量）
                                                       │
                                       ┌───────────────┴───────────────┐
                                       ▼                               ▼
                              mml-vue（Tauri invoke）      custom-main-demo（window.mml 桥接）
```

- 新增命令加 `#[gui_macros::command]`（过程宏在 `ipc-gen/macros/`，展开成 Tauri 的命令宏），
  新增事件加 `#[gui_macros::emit]`；改完跑 `cd mml-gui && npm run gen`（或任意 `cargo check`）。
- `mml-ipc/src/` 下的 `types/`、`bindings.ts`、`listens.ts` 是生成产物，**不要手改**：DTO 按
  来源 .rs 拆在 `types/` 下（`types/index.ts` 是 barrel），`bindings.ts` 是 `createCommands(invoke)`
  命令包装，`listens.ts` 是事件名、payload 类型映射与 `createEvents(on)` 订阅函数。
  命令与事件都不绑定传输层，调用方自己给 `invoke` / `on`；`mml-vue/src/lib/bindings.ts` 只是
  绑定 Tauri `invoke` 的兼容层。
- 自定义主页面的 `window.mml` 契约（`MmlApi`、白名单事件）在 `mml-ipc/src/home.ts`，手写、
  不参与扫描；`custom-main-demo` 从这里取类型、事件名与生成的 `onXxx` 函数，不再自建一份。
- 生成规则、分组方式与常见坑见 [AGENTS.md](../AGENTS.md) §4；包内用法见 [mml-ipc/README.md](../mml-ipc/README.md)。

## 开发与构建

环境要求（Windows 的 MSVC / WebView2 等）、依赖安装与出包产物见根 README 的[构建与运行](../README.md#构建与运行)。

依赖：Rust（含 cargo）、Node.js。启动器本体先跑 `build/init.ps1` 或 `bash build/init.sh`，
它会装好 `mml-vue` / `mml-gui` 的 npm 依赖；`custom/` 下前三个示例各自 `npm install`。
示例的 `mml-ipc` / `mml-vue` 由 `scripts/deps.mjs` 在安装前链接到本仓库，仓库不在时
自动浅克隆一份，不需要单独发布。`custom-ui-html-demo` 没有 npm 依赖，不用 install。

| 命令 | 说明 |
| --- | --- |
| `build/init.ps1` | 首次安装 `mml-vue` / `mml-gui` 的 npm 依赖（bash 版 `build/init.sh`） |
| `cd mml-gui && npm run tauri dev` | 桌面开发模式（会先还原 vendor 插件并起 vite，端口 1420 必须空闲） |
| `build/dev-frontend.ps1` | 只起 Vite，配合 debug 版 Rust 后端单独运行；`tauri dev` 已自带 Vite，不要重复起 |
| `build/build-release.ps1` | 正式发布构建（fat LTO，含安装包，按版本收集到 `build_out/`） |
| `build/build-prerelease.ps1` | 预发布构建（thin LTO，只出 exe，更快） |
| `bash build/package-flatpak.sh` | 用已构建的二进制打 `.flatpak`，需要 `flatpak`，不重新编译 |
| `bash build/package-arch.sh` | 用已构建的二进制打 `.pkg.tar.zst`，不依赖 `makepkg`，任一 Linux 可跑 |
| `cd mml-gui && npm run gen` | 改过 Rust 侧命令 / DTO 后重新生成 `mml-ipc` 契约 |
| `cd mml-gui && npx tauri icon ../design/icon/f-blend-mirror.svg` | 改过程序图标后重新生成 `src-tauri/icons/`；会顺带产出 `android/` 与 `ios/`，桌面端用不到可删 |
| `cd custom/custom-main-demo && npm run build` | 构建自定义主页面示例并打包成 `custom-main-demo.zip`（四个示例的用法见 [custom/README.md](../custom/README.md)） |

`build/` 下带 PowerShell 版（`.ps1`，Windows）与 bash 版（`.sh`，Linux / macOS / Git Bash）的脚本
做的是同一件事；`build/*.mjs` 是版本同步与产物打包用的跨平台辅助脚本。

Flatpak 与 Arch 包不在 `build-release` 里自动跑，由 `build/package-flatpak.sh`、
`build/package-arch.sh` 单独完成，输入都是 `target/release/mml-gui`：
Flatpak 走 `flatpak build-export` + `build-bundle`，不依赖 SDK / runtime；
Arch 包按 `.PKGINFO` + `tar.zst` 直接组装，不依赖 `makepkg`，有 `bsdtar` 时顺带生成 `.MTREE`。
桌面文件与 AppStream 元数据在 `build/linux/`。

Flatpak 默认声明 `org.freedesktop.Platform//25.08`，安装时由 Flathub 提供运行时：
`flatpak install --user ./mml-linux-x64-0.0.1.flatpak`。本地系统比 25.08 新时（例如 Ubuntu 26.04），
编出的二进制可能要求更高版本的 glibc，只有 CI 在 `ubuntu-22.04` 上编的包才与 25.08 匹配；
要换运行时用 `MML_FLATPAK_RUNTIME=26.08 bash build/package-flatpak.sh`。

发布版本从 `0.0.1` 开始，唯一来源是 `build/version`；`build-release` 先把版本同步到
`mml-gui/src-tauri/Cargo.toml`、`tauri.conf.json` 与 `mml-gui/package.json`，再把 Tauri 产物
按版本重命名到 `build_out/`。完整发版流程见根 README 的[版本与发布](../README.md#版本与发布)。

构建 profile 与产物：正式发布是 `[profile.release]`（fat LTO + `codegen-units = 1`），
预发布是 `[profile.prerelease]`（thin LTO + `codegen-units = 16`）；两者分别在 `target/release/`
与 `target/prerelease/` 下，来回切不会互相刷缓存。预发布也要安装包时，只能覆盖 release 的设置
（`tauri` CLI 不认 `--profile`）：

```powershell
$env:CARGO_PROFILE_RELEASE_LTO = 'thin'
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '16'
cd mml-gui
npm run tauri build -- --features prerelease
```

`--features prerelease` 不能漏，否则主界面不会提示「预发布版」，出的包与正式版一样。
vendor 插件的还原方式见 [AGENTS.md](../AGENTS.md) §3。

测试按改动范围只跑相关的那几个：`cargo test -p <crate>`（不要整仓全量测试）。

## 自动化构建（GitHub Actions）

[`.github/workflows/release.yml`](../.github/workflows/release.yml) 把 `init` 与 `build-release`
两个本地脚本串成三平台流水线，不另写一份构建逻辑：

| 触发 | 行为 |
| --- | --- |
| 推送 `v*` 标签（如 `v0.0.1`） | `check-version` 校验 `build/version` 与标签一致，随后并行构建三平台，最后汇总成一个 GitHub Release |
| 推送 `releases/**` 分支 | 只构建、把产物传成 Actions artifact，不发 Release，用来验证构建 |
| Actions 页面手动 `workflow_dispatch` | 同上，验证用 |

每个平台的步骤都是同一套：

1. `actions/checkout@v4` + `actions/setup-node@v4`（Node 22，按两个 lockfile 缓存）+
   `dtolnay/rust-toolchain@stable` + `Swatinem/rust-cache@v2`；
2. Windows 跑 `build/init.ps1` / `build/build-release.ps1`，
   Linux 与 macOS 跑 `bash build/init.sh` / `bash build/build-release.sh`；
3. Linux 额外跑 `build/package-flatpak.sh` 与 `build/package-arch.sh`，
   把 `.flatpak` 与 `.pkg.tar.zst` 也放进 `build_out/`；
4. 脚本把安装包按版本改名到 `build_out/`，`actions/upload-artifact@v4` 收走；
   发版 job 用 `actions/download-artifact@v4`（`merge-multiple: true`）合并，
   再交给 `softprops/action-gh-release@v2`。

- Linux runner 固定 `ubuntu-22.04` 作为 glibc 基线，并预装 Tauri 需要的
  `libwebkit2gtk-4.1-dev` / `libayatana-appindicator3-dev` / `librsvg2-dev` /
  `patchelf` / `libxdo-dev` / `libfuse2` / `rpm` / `libdbus-1-dev` /
  `libarchive-tools` / `zstd`。`libdbus-1-dev` 是 `mml-sys` 在 Linux 上
  「在文件管理器中显示」用的 D-Bus 绑定，缺了会编译失败；`libarchive-tools` 用来给 Arch 包生成 `.MTREE`。
- 产物名与本地一致：`mml-windows-x64-0.0.1.msi`、`mml-linux-x64-0.0.1.deb`、
  `mml-linux-x64-0.0.1.flatpak`、`mml-arch-x64-0.0.1.pkg.tar.zst`、
  `mml-macos-arm64-0.0.1.dmg` 等，架构取自 `runner.arch`。
- Flatpak 不需要在 runner 上装 `org.freedesktop.Platform`：封装只做 OSTree 提交与 bundle，
  runtime 是安装时由用户从 Flathub 获取的依赖。Arch 包只是把二进制与 `.PKGINFO` 打成 tar.zst，
  不需要装 `base-devel` 或 WebKitGTK。
- 脚本里的交互式暂停靠 `CI` 环境变量跳过（工作流顶层设 `CI: true`）；
  给脚本喂 stdin 会在无终端环境下卡住，不要那么做。
- 版本号只在 `build/version` 改：标签与文件不一致时 `check-version` 直接失败，
  避免发出错版本的包。
- macOS runner 的 `bash` 是 3.2，`build/*.sh` 与 `mml-gui/vendor/prepare.sh` 都不能用
  `mapfile` / `readarray` 这类 bash 4 语法。
- 发版 job 需要 `contents: write`（工作流默认只给 `contents: read`），只在 tag 触发时执行。
