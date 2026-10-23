# M²L Theme Studio

独立桌面主题开发工具，使用 Electron、Vue 与 Monaco。左侧项目文件树，中间代码编辑器，右侧实时内容预览，底部构建输出。F12 打开预览页面的完整 Chromium 开发者工具，包含 Elements、Console、Sources、Network。

## 启动

需要 Node.js 24+、npm 和 Git。首次安装需要访问 npm、GitHub 与 Electron 下载源。

```powershell
cd mml-theme-studio
npm install
npm start
```

`npm start` 先编译界面，再启动独立窗口。工具不依赖运行中的启动器，不占用启动器的 1420 端口。预览服务只监听 `127.0.0.1`，自动分配端口。

## 使用

1. 点「新建项目」，选择 demo 类型、父目录和项目名。默认自动从官方仓库下载 demo 以及同一提交的 `mml-ipc`、`mml-vue`；也可选择本地启动器源码仓库。
2. 工具复制源码、安装依赖并打开预览。SDK 快照保存在项目的 `.mml-deps`，后续安装不会悄悄换成另一个版本。
3. 在文件树选择文件，在 Monaco 中编辑。`Ctrl+S` 保存后自动重建预览；外部编辑器改过同一文件时，会阻止覆盖，可用编辑区右上角「从磁盘重新打开」读取新版本。
4. 顶部「界面配色」切换工具、代码编辑器与原生标题栏的亮色 / 暗色，重启后保留选择。右侧的预览配色独立切换，可选择内置窗口以及代码与预览的宽度比例。F12 或 `Ctrl+Shift+I` 打开开发者工具，在 Elements 中选取控件、查看样式，在 Console、Sources、Network 调试。
5. 「CSS 控件 / CSS 变量」读取项目 SDK 生成的清单。点击一项会在当前光标处插入选择器骨架或变量引用。
6. 保存后点「构建 ZIP」，输出日志会给出产物路径。把 ZIP 导入启动器验证实际功能。F12 中临时修改的 DOM / CSS 不会自动写回源码。

| Demo | 编辑入口 | ZIP 内容入口 |
| --- | --- | --- |
| CSS 样式包 | `custom.css`、`src/replace.ts` | `custom.css`、`custom.js` |
| Vue 整页主题 | `src/ReplaceWindow.vue`、`src/replace.ts` | `custom.js`、`custom.css` |
| Vue 主页组件 | `src/App.vue`、`src/entry.ts` | `custom.js`、`custom.css` |
| HTML 整页主题 | `src/index.html`、`src/style.css`、`src/app.js` | `index.html` |

三个组件类 demo 在项目快照中的真实 `mml-vue` 宿主上预览，CSS 和组件加载方式与启动器相同。HTML demo 使用其内置浏览器预览。预览没有启动器后端，登录、游戏启动、实例文件等操作需要导入启动器验证；需要实例参数的窗口只能展示无实例时的界面。代码与预览分栏支持实时迭代，暂不提供拖拽生成 Vue 布局。

关闭工具会回收安装、构建及预览子进程；有未保存内容或任务运行时先确认。项目源码留在所选目录，最近项目、语言与界面配色写入工具自己的 `studio.json`。源码运行的下载、构建缓存放在仓库 `target/temp/theme-studio`；分发版放在用户数据目录的 `target/temp`。SDK 缓存和预览临时文件不进入主题 ZIP。

## 分发

```powershell
npm run pack
```

生成当前系统的独立桌面程序目录，位置为仓库 `target/temp/theme-studio-package/MML-Theme-Studio-<平台>-<架构>`。Windows 运行其中的 `MML-Theme-Studio.exe`，分发时需包含整个目录。程序自带 Chromium，编辑和 F12 不需要浏览器；下载 demo、安装依赖、构建与预览仍需要系统 Node.js 24+、npm 和 Git。ZIP 构建沿用 demo 的脚本，Windows 使用自带 tar / PowerShell，macOS 使用 bsdtar，Linux 需自行提供能写 ZIP 的 bsdtar。

## 最小验证

```powershell
npm run lint
npm test
npm run build
npm run smoke
npm run smoke:theme
# 对已安装依赖的真实 demo 进一步验证组件加载、配色与源码实时重建：
npm run smoke -- D:/themes/css D:/themes/page D:/themes/home
```

Smoke 测试启动隐藏的 Electron 编辑器与预览视图，通过 DOM、IPC 和 DevTools 状态验证运行结果，不截图、不枚举或操纵用户桌面窗口。测试目录统一在仓库 `target/temp/theme-studio-tests`。

## English

An independent desktop theme editor with a project tree, Monaco editor, live preview and full Chromium DevTools. Install Node.js 24+, npm and Git, then run `npm install` and `npm start` in this directory. Choose English in the top-right language selector. Appearance switches the editor and native window between Light and Dark, remembers your choice, and stays independent of preview colors.

Create a project from one of the four official demos; the tool downloads a matching SDK snapshot, installs dependencies and starts preview. Save with Ctrl+S to rebuild, press F12 to inspect the preview, and use Build ZIP to produce a launcher-importable theme. DevTools edits are temporary. Game launch, login and instance file operations require validation in the launcher. The editor does not generate layouts by dragging controls.
