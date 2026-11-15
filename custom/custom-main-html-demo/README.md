# custom-main-html-demo —— HTML 自定义主页面示例

「自定义主页面」的 `index.html` 路线：包根是一张普通网页，启动器把它放进主页内容区的
iframe，并在响应里注入 `window.mml`。启动器的标题栏、侧栏和其它窗口都不受影响。
不需要 Vue，也不需要构建。

主页包也可以放 `custom.js` 注册 Vue 组件，那条路线见
[custom-main-demo](../custom-main-demo/README.md)；本示例只有 `index.html`，走 iframe。

## mml-vue 的支持情况

支持，链路是完整的：

- [mml-vue/src/lib/customHomeUi.ts](../../mml-vue/src/lib/customHomeUi.ts) 在挂载前拉
  `custom_home_status`，有 `scriptUrl` 就加载组件，没有就留给 iframe；
- [CustomHomePage.vue](../../mml-vue/src/components/CustomHomePage.vue) 没有组件时用
  `sandbox="allow-scripts allow-forms allow-popups allow-modals"` 的 iframe 加载
  `entryUrl`，**没有 `allow-same-origin`**；
- Rust 侧的 `mml-home` 协议伺服包里的 `index.html`，并把桥接脚本注入到
  `</head>` 之前，页面因此能拿到 `window.mml`；
- 包根同时有 `custom.js` 时优先走组件回调，`index.html` 只是回退；本示例只放
  `index.html`。

页面与启动器是两个文档：拿不到启动器的 DOM，也不能用 `localStorage` / `cookie`
（opaque origin 会直接抛错）。要数据就调命令，要事件就订阅。

## 快速开始

没有 npm 依赖，不用 `npm install`：

```bash
npm run pack      # 把 src/ 压成 custom-main-html-demo.zip
```

导入：启动器 → **设置 → 客户端设置 → 自定义主页面 → 导入**，选
`custom-main-html-demo.zip`，再打开「启用自定义主页面」→ 回到主窗口主页即可看到。

浏览器里看界面：直接打开 `src/index.html`。没有 `window.mml` 时页面用一份假数据，
实例列表、启动进度、主题切换都能点。

## 这个示例做了什么

- 从 `main_get_instances` 拉实例列表，选中后调 `main_launch_game` 启动；
- 订阅 `launch-state` / `launch-error` / `game-exit`，实时显示启动阶段与进度；
- 订阅 `instance-change`，实例增删后自动刷新列表；
- 主题跟随启动器：`onTheme` 推什么就把 `<html data-mml-theme>` 设成什么，页面配色
  跟着换；浏览器预览里右上角的按钮才可切换；
- 启动器不在时降级成预览模式，不阻塞界面。

## 目录

| 文件 | 作用 |
| --- | --- |
| `src/index.html` | 主页结构，包根入口 |
| `src/style.css` | 深 / 浅两套配色与全部布局 |
| `src/app.js` | `window.mml` 接线：实例、启动、事件、主题 |
| `src/theme.json` | 包清单（`type` 为 `home`），开发者手写，打包时复制到包根 |
| `scripts/pack.mjs` | 把 `src/` 原样复制成包根并压 zip，零依赖 |

`src/` 会整个进包，图片等静态资源直接放进去、用相对路径引用即可。

## 包格式

zip 根目录：

- `index.html`：必需，主页入口；没有它也没有 `custom.js` 的包会被拒绝导入
- `theme.json`：声明 `type` 为 `home`，源文件在 `src/theme.json`
- `style.css` / `app.js`：本示例自己的资源，可改名、可拆目录
- 不要用 `import` / `export`（页面脚本按普通 `<script>` 加载），也不要写 `<base>`

整包只套一层文件夹也认。单份包有条目数与解压后大小上限，导入时校验。

## 和 custom-main-demo 的区别

| | custom-main-demo | 本示例 |
| --- | --- | --- |
| 包根入口 | `custom.js` + `custom.css` | `index.html` |
| 渲染方式 | 在启动器页面里挂 Vue 组件，能用 `window.mml` 与 mml-vue 的控件 | sandbox iframe 里的独立网页 |
| 技术栈 | Vue + Vite | 原生 HTML / CSS / JS，零依赖 |
| 适合 | 想复用启动器样式与控件、只写内容 | 完全自己画主页，或换一套前端技术栈 |

两种包都归「自定义主页面」一类，同时只能装一份；再导入就是替换旧的。

## 改哪里

| 想改什么 | 改哪 |
| --- | --- |
| 文案、公告、活动、布局 | `src/index.html` |
| 配色、间距、响应式 | `src/style.css` 顶部的 `:root` 与 `[data-mml-theme="Light"]` |
| 拉哪些数据、按钮做什么 | `src/app.js` 的 `loadInstances` / `launch` |
| 包名与版本 | `src/theme.json` |

## 约定与注意点

- `window.mml` 只在启动器里存在；直接打开页面或在主题编辑器里预览时没有，必须自己兜底；
- 命令名与参数用 Rust 侧的命令名，如 `main_launch_game` 的 `{ uuid, world, force, autoSave }`；
- 事件只放行 `launch-state` / `launch-error` / `game-exit` / `game-log` / `instance-change`；
- iframe 是 opaque origin，别访问 `document.cookie` / `localStorage` / `parent.document`；
- 这一页只铺满主页内容区，不要自己画标题栏或窗口按钮，那些由启动器负责。
