# M²L Theme Studio

独立桌面主题开发工具，使用 Tauri 2、Vue 3 与 Monaco。左侧项目文件树，中间代码编辑器，右侧实时内容预览，底部构建输出。预览是嵌在编辑器里的设计画布，带 VS WPF 设计器风格的选择框和八个缩放手柄；窗口超出预览区时出现滚动条。工具栏 F12 打开开发者工具，可在 Elements / Console 里检查预览 iframe。

## 启动

需要 Rust 工具链（Windows 用 MSVC）、Node.js 24+、npm 与 Git。Windows 10/11 自带 WebView2。

```powershell
cd mml-theme-studio
npm install
npm start
```

`npm start` 就是 `tauri dev`：先起前端 Vite（`127.0.0.1:1421`，与启动器的 1420 错开），再编译并启动 Rust 桌面壳。工具不依赖运行中的启动器。

只调界面可以跑 `npm run dev`，浏览器打开 `http://127.0.0.1:1421`；没有 Tauri IPC，数据类操作会报错。

## 使用

1. 点「新建项目」，选择 demo 类型、父目录和项目名。默认自动从官方仓库下载 demo 以及同一提交的 `mml-ipc`、`mml-vue`；也可选择本地启动器源码仓库。
2. 工具复制源码、安装依赖并打开预览。SDK 快照保存在项目的 `.mml-deps`，后续安装不会悄悄换成另一个版本。
3. 文件树按目录层级展开 / 收起；选择文件后在 Monaco 中编辑。`Ctrl+S` 保存后自动重建预览；外部编辑器改过同一文件时，会阻止覆盖，可用编辑区右上角「从磁盘重新打开」读取新版本。
4. 顶部「界面配色」切换工具、代码编辑器与原生标题栏的亮色 / 暗色，重启后保留选择。右侧的预览配色独立切换，可选择内置窗口，并通过八个手柄或「窗口大小」输入框调整预览窗口尺寸。预览窗口的下限取启动器对应窗口的最小客户区尺寸（主窗口 920×600、设置 850×600、账户 800×650 等），预览区放不下时滚动查看。
5. 「CSS 控件 / CSS 变量」读取项目 SDK 生成的清单。点击一项会把选择器骨架或变量定义追加到项目根目录的 `custom.css` 末尾并打开它；页面 / 主页主题第一次追加时，会自动在入口文件里引入 `custom.css`。
6. 保存后点「构建 ZIP」，输出日志会给出产物路径。把 ZIP 导入启动器验证实际功能。开发者工具里临时修改的 DOM / CSS 不会自动写回源码。
7. `F12` / `Ctrl+Shift+I` 只在预览窗口内生效，或点工具栏 F12；开发者工具打开后，在 Elements / Console 里选中预览 iframe 检查。编辑器里的 F12 保留给 Monaco（转到定义）。

| Demo | 编辑入口 | ZIP 内容入口 |
| --- | --- | --- |
| CSS 样式包 | `custom.css`、`src/replace.ts` | `custom.css`、`custom.js` |
| Vue 整页主题 | `src/ReplaceWindow.vue`、`src/replace.ts` | `custom.js`、`custom.css` |
| Vue 主页组件 | `src/App.vue`、`src/entry.ts` | `custom.js`、`custom.css` |
| HTML 整页主题 | `src/index.html`、`src/style.css`、`src/app.js` | `index.html` |

三个组件类 demo 在项目快照中的真实 `mml-vue` 宿主上预览，CSS 和组件加载方式与启动器相同。HTML demo 使用其内置浏览器预览。预览没有启动器后端，登录、游戏启动、实例文件等操作需要导入启动器验证；需要实例参数的窗口只能展示无实例时的界面。预览窗口支持拖拽八个手柄或输入宽高；超出预览区时画布出现滚动条，可以滚动查看整窗。

关闭工具会回收安装、构建及预览子进程；有未保存内容或任务运行时先确认。项目源码留在所选目录，最近项目、语言与界面配色写入工具自己的 `studio.json`。

## 目录

```
src/            Vue 编辑器（Monaco、文件树、预览占位、输出）
src-tauri/      Rust 桌面壳：项目 / 文件 / 子进程 / 预览服务 / 命令
preview/        由系统 Node.js 执行的预览脚本（Vite 服务、HTML 预览、清单读取）
lib/messages.mjs 界面文案（中英两套，预览脚本也用它）
```

编辑器前端只通过 `src-tauri` 的 `studio_*` 命令取数据；预览页面来自本机随机端口，通过 iframe 嵌在编辑器里。它是跨源页面，不能访问编辑器；超出预览区时由设计画布滚动。

## 数据与缓存

- 配置：Windows 为 `%APPDATA%\com.coloryr.mml-theme-studio\studio.json`，键用大驼峰（`Locale` / `Recent` / `Theme`），无 BOM。
- 源码运行的下载、构建缓存：仓库 `target/temp/theme-studio`。
- 分发版的缓存：工具数据目录下的 `target/temp`。
- SDK 缓存与预览临时文件不进入主题 ZIP。

## 分发

```powershell
npm run pack
```

等于 `tauri build`。正式安装包在仓库 `target/release/bundle`（Windows 为 MSI / NSIS）。程序只带桌面壳，WebView2 用系统自带的（Windows 10/11 已预装）；编辑和开发者工具不需要浏览器。下载 demo、安装依赖、构建与预览仍需要系统 Node.js 24+、npm 和 Git。ZIP 构建沿用 demo 的脚本，Windows 使用自带 tar / PowerShell，macOS 使用 bsdtar，Linux 需自行提供能写 ZIP 的 bsdtar。

预发布或只想验证能编译：

```powershell
npm run tauri build -- --debug --no-bundle
```

## 最小验证

```powershell
npm run lint
npm test
npm run test:rust
npm run build
npm run tauri build -- --debug --no-bundle
```

`npm test` 只测预览脚本与静态资源边界；项目创建、文件读写、构建产物识别在 `cargo test -p mml-theme-studio --lib` 里。界面、预览画布与开发者工具需要手动启动确认：

1. 新建一个 CSS demo，确认依赖安装完成、预览出现在右侧区域。
2. 改 `custom.css` 并保存，确认预览不刷新整页也能看到变化。
3. 切换预览窗口种类与配色，确认只重载预览、不影响编辑器；把窗口拉到超过预览区，确认出现横向 / 纵向滚动条，拖动能滚到四角的手柄。
4. 切换界面配色，确认 Monaco 与原生标题栏一起变化，重启后保持。
5. 点 F12，确认 Elements 里能选中预览 iframe，Console / Network 正常可用。
6. 构建 ZIP，导入启动器验证。
7. 有未保存修改时关窗，确认先弹确认框；取消后窗口保留。

## English

An independent desktop theme editor built with Tauri 2, Vue 3 and Monaco. Install a Rust toolchain, Node.js 24+, npm and Git, then run `npm install` and `npm start` in this directory. The dev server listens on `127.0.0.1:1421`, separate from the launcher's 1420, and the Rust shell is compiled and launched by `tauri dev`.

Create a project from one of the four official demos; the tool downloads a matching SDK snapshot, installs dependencies and starts preview. Save with Ctrl+S to rebuild. The preview is a design canvas with VS WPF designer-style selection handles and a size input; resize the window with the eight handles, and scroll the canvas when it is larger than the preview area. The preview enforces the launcher's per-window minimum size (for example 920×600 for the main window). Press F12 / Ctrl+Shift+I inside the preview or use the toolbar F12 button; the editor's F12 stays with Monaco. Select the preview iframe in Elements / Console. DevTools edits are temporary. Game launch, login and instance file operations require validation in the launcher.

`npm run pack` builds installers under the repository's `target/release/bundle`. The user project's npm/Vite build still needs system Node.js 24+, npm and Git.
