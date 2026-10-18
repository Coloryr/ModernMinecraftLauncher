# ModernMinecraftLauncher (M²L)

[ColorMC](https://github.com/Coloryr/ColorMC) 的迭代软件，一个 Minecraft 启动器。

**未完成，请勿使用任何东西~~也不要催~~**

## 未来计划
- 主题包，安装后修改界面样式
- cleanroom 原生支持
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
- 客户端导出
- 服务器实例生成
- 启动器自更新
- 管理员启动
- 生成实例信息
- 导出模组列表
- 游戏时长统计
- 窗口透明
- 窗口全屏和占满
- 从存档启动游戏

## 技术栈

- **内核**：Rust workspace（`mml-core/` 本体 + 18 个子 crate，edition 2024）
- **界面**：Tauri 2 桌面壳（`mml-gui/`）+ Vue 3 / Vite 前端（`mml-vue/`）
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
| 界面 | 多窗口与单窗口两种模式，暗色 / 亮色 / 跟随系统，中英双语，可导入自定义主页面 |

## 目录结构

| 目录 | 职责 |
| --- | --- |
| `mml-core/` | 启动器内核，多 crate：游戏启动、版本管理、Mod 加载器、整合包、下载、认证、Java 管理、网络 API（Mojang / Modrinth / CurseForge / Fabric 等）、皮肤与贴图渲染、NBT 解析等 |
| `mml-gui/` | Tauri 桌面壳；窗口规格与 IPC 命令在 `src-tauri/src/windows/`，IPC 类型由 `ipc-gen` 扫描 Rust 源码自动生成 |
| `mml-vue/` | 前端（Vue3 + Vite）；窗口在 `src/windows/`，通用组件在 `src/components/` |
| `mml-ui-demo/` | **独立**的 Vue3 + Vite 示例工程：给服主写「自定义主页面」用的样板，`npm run build` 产出可直接导入启动器的 zip（不进 Cargo workspace，也不挂进 `mml-vue` 的构建） |
| `build/` | 构建脚本（正式发布 / 预发布 / 只跑前端），每个都有 PowerShell 版与 bash 版 |

三个子项目各有 README：[mml-gui](mml-gui/README.md)、[mml-vue](mml-vue/README.md)、[mml-ui-demo](mml-ui-demo/README.md)。

## 开发与构建

依赖：Rust（含 cargo）、Node.js。

| 命令 | 说明 |
| --- | --- |
| `cd mml-gui && npm run tauri dev` | 桌面开发模式（会先还原 vendor 插件并起 vite，端口 1420 必须空闲） |
| `build/dev-frontend.ps1` | 只跑前端（vite，浏览器访问 `http://localhost:1420/?window=<kind>` 预览窗口，无 IPC 数据） |
| `build/build-release.ps1` | 正式发布构建（fat LTO，含安装包） |
| `build/build-prerelease.ps1` | 预发布构建（thin LTO，只出 exe，更快） |
| `cd mml-gui && npm run gen` | 改过 Rust 侧命令 / DTO 后重新生成前端 `bindings.ts` |
| `cd mml-ui-demo && npm run build` | 构建自定义主页面示例并打包成 `mml-ui-demo.zip`（用法见该目录的 README） |

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
