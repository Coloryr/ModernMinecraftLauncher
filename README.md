# ModernMinecraftLauncher (M²L)

[ColorMC](https://github.com/Coloryr/ColorMC) 的迭代软件，一个 Minecraft 启动器。

**未完成，请勿使用任何东西~~也不要催~~**

## 未来计划
- 主题包，安装后修改界面样式
- cleanroom 原生支持

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

三个子项目各有 README：[mml-gui](mml-gui/README.md)、[mml-vue](mml-vue/README.md)、[mml-ui-demo](mml-ui-demo/README.md)。

## 开发与构建

依赖：Rust（含 cargo）、Node.js。

| 命令 | 说明 |
| --- | --- |
| `cd mml-gui && npm run tauri dev` | 桌面开发模式（会先还原 vendor 插件并起 vite，端口 1420 必须空闲） |
| `dev-frontend.bat` | 只跑前端（vite，浏览器访问 `http://localhost:1420/?window=<kind>` 预览窗口，无 IPC 数据） |
| `build-release.bat` | 正式发布构建（fat LTO，含安装包） |
| `build-prerelease.bat` | 预发布构建（thin LTO，只出 exe，更快） |
| `cd mml-gui && npm run gen` | 改过 Rust 侧命令 / DTO 后重新生成前端 `bindings.ts` |
| `cd mml-ui-demo && npm run build` | 构建自定义主页面示例并打包成 `mml-ui-demo.zip`（用法见该目录的 README） |

测试按改动范围只跑相关的那几个：`cargo test -p <crate>`（不要整仓全量测试）。

协作约定（IPC 生成、窗口模型、临时目录、日志调试等）统一写在 [AGENTS.md](AGENTS.md)。

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
