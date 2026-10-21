# ModernMinecraftLauncher (M²L)

[ColorMC](https://github.com/Coloryr/ColorMC) 的迭代软件，一个 Minecraft 启动器。

**未完成，请勿使用任何东西~~也不要催~~**

## 未来计划
- 实验性功能：模组映射
- 跨版本迁移助手
- 大屏手柄模式
- 文件占用分析
- 手柄设置
- 模组二分模式

## 待完成列表
- MCMOD搜索
- 简易崩溃分析
- 简易Mixin分析
- 手柄模式
- 存档云同步(ColorMC)
- 服务器列表导入导出
- 映射大厅 / 联机
- 客户端导出(服务器模式包)
- 服务器实例生成
- 启动器自更新
- 管理员启动
- 生成实例信息
- 导出模组列表
- 游戏时长统计
- 窗口全屏和占满

## 技术栈

- **内核**：Rust workspace（`mml-core/` 本体 + 18 个子 crate，edition 2024）
- **界面**：Tauri 2 桌面壳（`mml-gui/`）+ Vue 3 / Vite 前端（`mml-vue/`）
- **IPC**：Rust 源码是命令与 DTO 的唯一来源，`ipc-gen` 生成独立的 npm 契约包 `mml-ipc`，
  启动器前端与自定义主页面共用
- **渲染**：wgpu（方块 / 物品图标、皮肤与披风 2D / 3D）+ glow（皮肤模型预览）

## 功能

| 模块 | 说明 |
| --- | --- |
| 实例 | 新建、导入（文件夹 / 压缩包 / URL）、重命名、分组排序、导出为 CurseForge / Modrinth 整合包、删除；多个实例可同时运行 |
| 加载器 | Forge、NeoForge、Fabric、Quilt、OptiFine、LiteLoader，以及自定义加载器 |
| 整合包 | CurseForge / Modrinth 搜索、详情、按文件安装（带进度与取消） |
| 资源管理 | 模组（分组 / 启用停用 / 备注）、资源包、光影包、存档（备份与还原）、截图、结构文件、数据包、服务器列表 |
| 账户 | 离线、微软 OAuth、Nide8、Authlib-Injector、LittleSkin、自建 LittleSkin 六种认证 |
| Java | 扫描本机 Java；从 Adoptium / Zulu / OpenJ9 / Foojay 下载 JRE / JDK |
| 下载 | 多线程、断点续传、哈希校验，带独立的下载管理窗口 |
| 渲染 | 从客户端 jar 离线渲染方块 / 物品图标；皮肤、披风、头像的 2D / 3D 绘制 |
| 迁移 | 从 ColorMC / HMCL / MMC / 官方启动器导入数据 |
| 界面 | 多窗口与单窗口两种模式，暗色 / 亮色 / 跟随系统，中英双语；可导入自定义主页面、注入自定义 CSS，或整页重写主窗口 |

## 目录结构

| 目录 | 职责 |
| --- | --- |
| `mml-core/` | 启动器内核，多 crate：游戏启动、版本管理、Mod 加载器、整合包、下载、认证、Java 管理、网络 API（Mojang / Modrinth / CurseForge / Fabric 等）、皮肤与贴图渲染、NBT 解析等 |
| `mml-gui/` | Tauri 桌面壳；窗口规格与 IPC 命令在 `src-tauri/src/windows/`，命令与 DTO 由 `ipc-gen` 扫描生成到 `mml-ipc` |
| `mml-vue/` | 前端（Vue3 + Vite）；窗口在 `src/windows/`，通用组件在 `src/components/` |
| `ipc-gen/` | IPC 契约生成器（`gui-ipc-gen`）与过程宏（`ipc-gen/macros`，`gui-macros`） |
| `mml-ipc/` | npm 契约包：按来源拆分的跨 IPC 类型、命令包装、事件订阅函数与自定义主页 `window.mml` 契约，`mml-vue` 与 `custom/` 下的示例共用 |
| `custom/` | 四个自定义界面示例：`custom-main-demo/`（自定义主页面）、`custom-ui-demo/`（整页主题 · 组件）、`custom-ui-html-demo/`（整页主题 · HTML）、`custom-ui-css-demo/`（自定义 CSS）；各自构建出可直接导入启动器的 zip，用法见 [custom/README.md](custom/README.md) |
| `build/` | 构建脚本（正式发布 / 预发布 / 只跑前端），每个都有 PowerShell 版与 bash 版 |

各子项目各有 README：[mml-gui](mml-gui/README.md)、[mml-vue](mml-vue/README.md)、
[custom](custom/README.md)（四个示例各自的说明也在里面）、[mml-ipc](mml-ipc/README.md)。

### IPC 契约：Rust 是唯一来源

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
- 生成规则、分组方式与常见坑见 [AGENTS.md](AGENTS.md) §4；包内用法见 [mml-ipc/README.md](mml-ipc/README.md)。

## 开发与构建

依赖：Rust（含 cargo）、Node.js。`mml-vue/` 与前三个示例各自 `npm install`；
示例的 `mml-ipc` / `mml-vue` 由 `scripts/deps.mjs` 在安装前链接到本仓库，仓库不在时
自动浅克隆一份，不需要单独发布。`custom-ui-html-demo` 没有 npm 依赖，不用 install。

| 命令 | 说明 |
| --- | --- |
| `cd mml-gui && npm run tauri dev` | 桌面开发模式（会先还原 vendor 插件并起 vite，端口 1420 必须空闲） |
| `build/dev-frontend.ps1` | 只跑前端（vite，浏览器访问 `http://localhost:1420/?window=<kind>` 预览窗口，无 IPC 数据） |
| `build/build-release.ps1` | 正式发布构建（fat LTO，含安装包） |
| `build/build-prerelease.ps1` | 预发布构建（thin LTO，只出 exe，更快） |
| `cd mml-gui && npm run gen` | 改过 Rust 侧命令 / DTO 后重新生成 `mml-ipc` 契约 |
| `cd custom/custom-main-demo && npm run build` | 构建自定义主页面示例并打包成 `custom-main-demo.zip`（四个示例的用法见 [custom/README.md](custom/README.md)） |

上面三个脚本都在 `build/` 下，各有一份 PowerShell 版（`.ps1`，Windows）与 bash 版
（`.sh`，Linux / macOS / Git Bash），做的是同一件事。

测试按改动范围只跑相关的那几个：`cargo test -p <crate>`（不要整仓全量测试）。

协作约定（IPC 生成、窗口模型、临时目录、日志调试等）统一写在 [AGENTS.md](AGENTS.md)。

## 开源许可与第三方组件

本体以 **Apache-2.0** 发布（见下文[许可证](#许可证)）。安装包内含下列第三方组件；
**许可全文不随安装包分发**，同一份清单与上游入口也放在应用内「帮助 → 开源许可」里；
另有本项目会去调用的第三方服务，见文末[第三方服务与平台](#第三方服务与平台)。

### 内嵌的 Java 组件

| 组件 | 版本 | 许可 | 上游 |
| --- | --- | --- | --- |
| ColorASM | 1.1 | Apache-2.0，本项目作者作品 | [Coloryr/ColorMC](https://github.com/Coloryr/ColorMC) |
| ForgeWrapper | prism-2025-12-07 | MIT | [ZekerZhayard/ForgeWrapper](https://github.com/ZekerZhayard/ForgeWrapper) |
| OptifineWrapper | 1.1 | MIT | [Coloryr/OptifineWrapper](https://github.com/Coloryr/OptifineWrapper) |

三个都是 `include_bytes!` 编进二进制的，首次使用时写到 `libraries/` 下。

### 自绘标题栏插件

| 组件 | 版本 | 许可 | 上游 |
| --- | --- | --- | --- |
| tauri-plugin-decoration | 3.0.5 | MIT | [oovz/tauri-plugin-decoration](https://github.com/oovz/tauri-plugin-decoration) |

本仓库对它打了两个 patch（[mml-gui/vendor/patches/](mml-gui/vendor/patches/)），改动处已标
「M²L 本地修改」；上游源码按固定 commit 拉到 `target/vendor/`，仓库里不存第三方源码副本。

### 前端库（打进 `dist/`）

| 组件 | 版本 | 许可 | 上游 |
| --- | --- | --- | --- |
| vue | 3.5.43 | MIT | [vuejs/core](https://github.com/vuejs/core) |
| @tauri-apps/api | 2.12.2 | Apache-2.0 OR MIT | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) |
| @tauri-apps/plugin-dialog | 2.8.1 | MIT OR Apache-2.0 | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) |
| monaco-editor | 0.56.0 | MIT | [microsoft/monaco-editor](https://github.com/microsoft/monaco-editor) |
| vscode `extensions/log`（日志语法，本项目转写为 Monarch） | main | MIT | [microsoft/vscode](https://github.com/microsoft/vscode/blob/main/extensions/log/syntaxes/log.tmLanguage.json) |
| dompurify | 3.4.16 | Apache-2.0 | [cure53/DOMPurify](https://github.com/cure53/DOMPurify) |
| marked | 18.1.0 | MIT | [markedjs/marked](https://github.com/markedjs/marked) |
| three | 0.156.1 | MIT | [mrdoob/three.js](https://github.com/mrdoob/three.js) |
| skinview3d | 3.4.2 | MIT | [bs-community/skinview3d](https://github.com/bs-community/skinview3d) |
| skinview-utils | 0.7.1 | MIT | [bs-community/skinview-utils](https://github.com/bs-community/skinview-utils) |
| @vue-flow/core | 1.48.2 | MIT | [bcakmakoglu/vue-flow](https://github.com/bcakmakoglu/vue-flow) |
| @vue-flow/background | 1.3.2 | MIT | [bcakmakoglu/vue-flow](https://github.com/bcakmakoglu/vue-flow) |
| @dagrejs/dagre | 3.1.1 | MIT | [dagrejs/dagre](https://github.com/dagrejs/dagre) |

dompurify 上游是 `MPL-2.0 OR Apache-2.0`，本项目按 Apache-2.0 履行；
monaco-editor 自身还打包了 marked 14.0.0（MIT）、dompurify 3.4.8（Apache-2.0）与 Codicon 字体（MIT）。

### Rust 静态链接依赖

`cargo metadata` 的非 dev 依赖闭包共 **475** 个 crate，全部为宽松许可，无 GPL / AGPL / LGPL：

| 许可表达式 | 数量 | 许可表达式 | 数量 |
| --- | --- | --- | --- |
| MIT OR Apache-2.0 | 233 | Unlicense OR MIT | 10 |
| MIT | 82 | BSD-3-Clause | 8 |
| Apache-2.0 OR MIT | 39 | MPL-2.0 | 6 |
| MIT/Apache-2.0 | 18 | ISC | 5 |
| Unicode-3.0 | 18 | 其余 29 种表达式 | 46 |
| Apache-2.0 | 10 | | |

其中 6 个是 **MPL-2.0**（文件级弱 copyleft，本项目未修改其源码，按许可要求给出源码位置）：
[cssparser](https://crates.io/crates/cssparser)、
[cssparser-macros](https://crates.io/crates/cssparser-macros)、
[dtoa-short](https://crates.io/crates/dtoa-short)、
[dwrote](https://crates.io/crates/dwrote)、
[option-ext](https://crates.io/crates/option-ext)、
[selectors](https://crates.io/crates/selectors)。
各许可原文见 [SPDX 许可列表](https://spdx.org/licenses/)。

### 第三方服务与平台

下列服务由第三方运营，本项目只是调用方，与它们没有隶属关系；使用对应功能要用你自己的账号。
应用内同样在「帮助 → 第三方服务与平台」里给了这些入口。

| 服务 | 用途 | 链接 |
| --- | --- | --- |
| Minecraft（Mojang / Microsoft） | 游戏本体与官方资源由 Mojang / Microsoft 分发；本启动器是第三方软件，与它们没有隶属关系 | [minecraft.net](https://www.minecraft.net/) |
| SakuraFrp | 联机（内网穿透）平台，后端接口见 [sakurafrp_api.rs](mml-core/mml-net/src/sakurafrp_api.rs) | [用户中心](https://www.natfrp.com/user/) |
| OpenFrp | 联机（内网穿透）平台，后端接口见 [openfrp_api.rs](mml-core/mml-net/src/openfrp_api.rs) | [控制台](https://console.openfrp.net/home/) · [API 文档](https://github.com/ZGIT-Network/OPENFRP-APIDOC) |

### 测试样本

`mml-core/mml-game/tests/` 与 `mml-gui/src-tauri/tests/` 下的样本（AllMusic 各加载器版本、
各上游启动器的实例样本等）**只用于测试，不随发行包分发**；其中 AllMusic 与本项目的
ColorASM 均为本项目作者的作品，样本文件由软件自身生成。

## 许可证

Apache 2.0

```
Copyright 2026 coloryr

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
