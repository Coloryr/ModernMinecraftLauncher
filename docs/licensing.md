# 开源许可与第三方组件

本体以 **Apache-2.0** 发布，许可全文见仓库根目录 [LICENSE](../LICENSE)。

安装包内含下列第三方组件；**许可全文不随安装包分发**，同一份清单与上游入口也放在应用内
「帮助 → 开源许可」里。另有本项目会去调用的第三方服务，见文末[第三方服务与平台](#第三方服务与平台)。

## 内嵌的 Java 组件

| 组件 | 版本 | 许可 | 上游 |
| --- | --- | --- | --- |
| ColorASM | 1.2.1 | Apache-2.0，本项目作者作品 | [Coloryr/ColorMC](https://github.com/Coloryr/ColorMC) |
| ForgeWrapper | prism-2025-12-07 | MIT | [ZekerZhayard/ForgeWrapper](https://github.com/ZekerZhayard/ForgeWrapper) |
| OptifineWrapper | 1.1 | MIT | [Coloryr/OptifineWrapper](https://github.com/Coloryr/OptifineWrapper) |

三个都是 `include_bytes!` 编进二进制的，首次使用时写到 `libraries/` 下。

## 自绘标题栏插件

| 组件 | 版本 | 许可 | 上游 |
| --- | --- | --- | --- |
| tauri-plugin-decoration | 3.0.5 | MIT | [oovz/tauri-plugin-decoration](https://github.com/oovz/tauri-plugin-decoration) |

本仓库对它只维护一个 patch（[mml-gui/vendor/patches/](../mml-gui/vendor/patches/)），改动处已标
「M²L 本地修改」；上游源码按固定 commit 拉到 `target/vendor/`，仓库里不存第三方源码副本。

## 前端库（打进 `dist/`）

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

## Rust 静态链接依赖

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

## 第三方服务与平台

下列服务由第三方运营，本项目只是调用方，与它们没有隶属关系；使用对应功能要用你自己的账号。
应用内同样在「帮助 → 第三方服务与平台」里给了这些入口。

| 服务 | 用途 | 链接 |
| --- | --- | --- |
| Minecraft（Mojang / Microsoft） | 游戏本体与官方资源由 Mojang / Microsoft 分发；本启动器是第三方软件，与它们没有隶属关系 | [minecraft.net](https://www.minecraft.net/) |
| SakuraFrp | 联机（内网穿透）平台，后端接口见 [sakurafrp_api.rs](../mml-core/mml-net/src/sakurafrp_api.rs) | [用户中心](https://www.natfrp.com/user/) |
| OpenFrp | 联机（内网穿透）平台，后端接口见 [openfrp_api.rs](../mml-core/mml-net/src/openfrp_api.rs) | [控制台](https://console.openfrp.net/home/) · [API 文档](https://github.com/ZGIT-Network/OPENFRP-APIDOC) |

## 测试样本

`mml-core/mml-game/tests/` 与 `mml-gui/src-tauri/tests/` 下的样本（AllMusic 各加载器版本、
各上游启动器的实例样本等）**只用于测试，不随发行包分发**；其中 AllMusic 与本项目的
ColorASM 均为本项目作者的作品，样本文件由软件自身生成。
