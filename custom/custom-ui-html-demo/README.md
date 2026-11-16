# custom-ui-html-demo —— 整页 HTML 主题示例

第三种换皮「完全自定义界面」的 `index.html` 路线：整包就是一张普通网页，启动器把它
放进 iframe 铺满主窗口，并在响应里注入 `window.mml`。不需要 Vue，也不需要构建。

## mml-vue 的支持情况

支持，链路是完整的：

- [mml-vue/src/lib/customUi.ts](../../mml-vue/src/lib/customUi.ts) 启动时 `initCustomUi()`
  先拉 `custom_ui_status`，拿到 `pageEntryUrl`；
- [mml-vue/src/App.vue](../../mml-vue/src/App.vue) 在主窗口渲染
  [CustomUiPage.vue](../../mml-vue/src/components/CustomUiPage.vue)，它用一个
  `sandbox="allow-scripts allow-forms allow-popups allow-modals"` 的 iframe 加载这个地址，
  **没有 `allow-same-origin`**；
- Rust 侧 [protocol.rs](../../mml-gui/src-tauri/src/windows/custom_ui/protocol.rs) 的
  `mml-ui` 协议伺服包里的 `index.html`，并把桥接脚本注入到 HTML 的 `</head>` 之前，
  页面因此能拿到 `window.mml`；
- 包根同时有 `custom.js` 时优先走窗口组件回调，`index.html` 只是回退；本示例只放
  `index.html`，所以走 iframe。

页面与启动器是两个文档：拿不到启动器的 DOM，也不能用 `localStorage` / `cookie`
（opaque origin 会直接抛错）。要数据就调命令，要事件就订阅。

## Tailwind 是预编译的

`src/tailwind.css` 是**预编译产物**，随包一起压进 zip（`index.html` 里 `<link>` 它），
所以这个示例仍然零依赖、`npm run pack` 直接能出包。

- 页面里已经在用工具类：`titlebar / brand / titlebar-actions` 上的
  `flex items-center justify-between`、`gap-2.5`、`gap-0.5`（展开后的声明在
  `tailwind.css` 里，原来那几条已从 `style.css` 删掉）。
- **改了类名必须重新生成**，否则新类不会出现在 `tailwind.css` 里、页面上静默失效。
  生成用的是一次性工程（放在仓库的 `target/temp`，不进版本库）：

  ```bash
  mkdir -p target/temp/tw-pre && cd target/temp/tw-pre
  npm init -y && npm i -D tailwindcss @tailwindcss/cli mml-vue@file:../../../mml-vue
  cat > ui.css <<'EOF'
  @layer theme, base, components, utilities;
  @import "tailwindcss/theme.css" layer(theme) source(none);
  @import "tailwindcss/utilities.css" layer(utilities) source(none);
  @import "../../../mml-vue/src/styles/tailwind-tokens.css";
  @source "../../../custom/custom-ui-html-demo/src";
  EOF
  npx @tailwindcss/cli -i ui.css -o out.css
  cp out.css ../../../custom/custom-ui-html-demo/src/tailwind.css
  ```

  关键两点：`source(none)` 关掉自动扫描，否则会把 mml-vue 整棵树当内容源（产物 70 KB 起）；
  token 直接引 `mml-vue/src/styles/tailwind-tokens.css`，与启动器同一份。
- **颜色类依赖主题变量**：iframe 是独立文档，拿不到宿主的 `--bg-card`。页面里的那段监听
  （`index.html` 的 `<head>`）收到 [theme-tokens 消息](../README.md#iframe-主题拿主题色)
  会把变量写进 `:root`，`bg-card` / `text-dim` 这类类才算得出颜色；只写排版类（`flex` /
  `gap-*` / `text-meta`）则不需要它。


## 快速开始

没有 npm 依赖，不用 `npm install`：

```bash
npm run pack      # 把 src/ 压成 custom-ui-html-demo.zip
```

导入：启动器 → **设置 → 界面 → 主题包 → 导入主题包**，选 `custom-ui-html-demo.zip`，
再打开「启用主题包」。

浏览器里看界面：直接打开 `src/index.html`。没有 `window.mml` 时页面用一份假数据，
实例列表、启动进度、主题切换都能点，窗口按钮只闪提示。

## 这个示例做了什么

- 自绘标题栏：拖动、双击最大化、最小化 / 最大化 / 关闭都调启动器命令；
- 从 `main_get_instances` 拉实例列表，选中后调 `main_launch_game` 启动；
- 订阅 `launch-state` / `launch-error` / `game-exit`，实时显示启动阶段与进度；
- 订阅 `instance-change`，实例增删后自动刷新列表；
- 主题按钮改 `window_get_gui_config` / `window_save_gui_config` 的 `theme`，页面配色
  跟随桥接脚本写在 `<html data-mml-theme>` 上的值；
- 启动器不在时整页降级成预览模式，不阻塞界面。

## 目录

| 文件 | 作用 |
| --- | --- |
| `src/index.html` | 整页结构，包根入口 |
| `src/style.css` | 深 / 浅两套配色与全部布局 |
| `src/app.js` | `window.mml` 接线：实例、启动、事件、窗口按钮 |
| `src/theme.json` | 包清单（`type` 为 `page`），开发者手写，打包时复制到包根 |
| `scripts/pack.mjs` | 把 `src/` 原样复制成包根并压 zip，零依赖 |

`src/` 会整个进包，图片等静态资源直接放进去、用相对路径引用即可。

## 包格式

zip 根目录：

- `index.html`：必需，整页入口；没有它也没有 `custom.js` 的包会被拒绝导入
- `theme.json`：声明 `type` 为 `page`，源文件在 `src/theme.json`
- `style.css` / `app.js`：本示例自己的资源，可改名、可拆目录
- 不要用 `import` / `export`（页面脚本按普通 `<script>` 加载），也不要写 `<base>`

整包只套一层文件夹也认。单份包有条目数与解压后大小上限，导入时校验。

## 和 custom-ui-demo 的区别

| | custom-ui-demo | 本示例 |
| --- | --- | --- |
| 包根入口 | `custom.js` + `custom.css` | `index.html` |
| 渲染方式 | 在启动器页面里挂 Vue 组件，可复用内置 `MainWindow` | sandbox iframe 里的独立网页 |
| 技术栈 | Vue + Vite | 原生 HTML / CSS / JS，零依赖 |
| 适合 | 只改一部分窗口、想复用内置组件 | 完全重画整页、或换一套前端技术栈 |

两种包都归「主题包」一类，同时只能装一份；再导入就是替换旧的。

## 改哪里

| 想改什么 | 改哪 |
| --- | --- |
| 文案、公告、布局 | `src/index.html` |
| 配色、间距、响应式 | `src/style.css` 顶部的 `:root` 与 `[data-mml-theme="Light"]` |
| 拉哪些数据、按钮做什么 | `src/app.js` 的 `loadInstances` / `launch` / `toggleTheme` |
| 包名与版本 | `src/theme.json` |

## 约定与注意点

- `window.mml` 只在启动器里存在；直接打开页面或在开发参数下预览时没有，必须自己兜底；
- 命令名与参数用 Rust 侧的命令名，如 `main_launch_game` 的 `{ uuid, world, force, autoSave }`；
- 事件只放行 `launch-state` / `launch-error` / `game-exit` / `game-log` / `instance-change`；
- iframe 是 opaque origin，别访问 `document.cookie` / `localStorage` / `parent.document`；
- 窗口关闭会走启动器的关闭确认，和内置界面一致；
- 开发时也可以用启动器前端的 `?customPage=http://localhost:1522/` 参数直接看某个地址，
  但这种加载没有桥接脚本，页面仍是预览模式。
