# custom —— 自定义界面示例

M²L 的界面有三种互相独立的换皮方式，这里给五个能直接导入启动器的示例（整页主题与
自定义主页面都有组件与 HTML 两条路线）。各例自成一个 npm 工程，互不依赖，按需进目录构建即可。

| 目录 | 换哪一块 | 包根入口 | `theme.json` 的 `type` | 启动器里的开关 | 产物 |
| --- | --- | --- | --- | --- | --- |
| [custom-main-demo](custom-main-demo/README.md) | 主窗口「主页」的内容区，组件路线 | `custom.js` + `custom.css` | `home` | 客户端设置 → 启用自定义主页面 | `custom-main-demo.zip` |
| [custom-main-html-demo](custom-main-html-demo/README.md) | 主窗口「主页」的内容区，换成一张 HTML 页面（iframe） | `index.html` | `home` | 客户端设置 → 启用自定义主页面 | `custom-main-html-demo.zip` |
| [custom-ui-demo](custom-ui-demo/README.md) | 整个主窗口，按 kind 换窗口组件 | `custom.js` + `custom.css` | `page` | 界面 → 样式与主题包 → 主题包 | `custom-ui-demo.zip` |
| [custom-ui-html-demo](custom-ui-html-demo/README.md) | 整个主窗口，换成一张 HTML 页面（iframe） | `index.html` | `page` | 界面 → 样式与主题包 → 主题包 | `custom-ui-html-demo.zip` |
| [custom-ui-css-demo](custom-ui-css-demo/README.md) | 内置界面的样式，可装多份叠加 | `custom.css`（可选 `custom.js`） | `css` | 界面 → 样式与主题包 → 样式包 | `custom-ui-css-demo.zip` |

三种方式**互不联锁**：装哪一种都不动另外两种的开关；整页主题开着时自定义主页仍在它
后面，属正常叠加。

## 快速开始

需要带代码编辑、实时预览和完整 F12 的独立界面时，可用 [M²L Theme Studio](../mml-theme-studio/README.md)，自动下载这五类 demo 并安装开发依赖。

每个示例各自构建，除两个 HTML 示例外都要先装依赖：

```bash
cd custom/custom-main-demo
npm install
npm run build      # 类型检查 + 打入口 + 压 zip

cd ../custom-ui-demo
npm install
npm run build

cd ../custom-ui-html-demo
npm run pack       # 无依赖，直接把 src 压成 zip

cd ../custom-main-html-demo
npm run pack       # 同上，主页 HTML 路线

cd ../custom-ui-css-demo
npm install
npm run build      # vite build 入口 + 压 zip
```

`mml-ipc` 与 `mml-vue` 没有发布到 npm，示例里写的是 `file:.mml-deps/<名字>`，
由各自的 `scripts/deps.mjs` 在 npm 解析依赖之前铺好：上级目录有本仓库就直接链到
`../../mml-ipc` / `../../mml-vue`，没有就浅克隆仓库到 `.mml-deps/repo` 再链。所以示例
单独拷走也能 `npm install` 起来，而在仓库里改完 `mml-vue` 的控件或 `mml-ipc` 的契约
直接重建即可。`.mml-deps/` 是生成物、已忽略，手动重建跑 `npm run deps`；克隆地址可用
`MML_REPO_URL` 覆盖。HTML 示例没有依赖，只用运行时的 `window.mml`。

构建产物（zip）留在各自目录下，已在 `.gitignore` 里忽略；重新构建会覆盖同名 zip。

## Tailwind 支持情况

五个示例都能用 Tailwind：三个带构建的在构建里编译（`@import "mml-vue/tailwind.css"`，
token 映射到启动器主题变量、不含 preflight）；两个 HTML 示例没有构建，用**预编译好的
`src/tailwind.css`**，改了类名要按各自 README 的命令重新生成。

| 示例 | 构建 | Tailwind 怎么来 |
| --- | --- | --- |
| custom-main-demo | Vite | 构建时编译，入口 `src/theme.css` |
| custom-ui-demo | Vite | 构建时编译，入口 `src/theme.css` |
| custom-ui-css-demo | Vite | 构建时编译，入口 `src/theme.css`，皮肤规则在 `src/custom.css` |
| custom-main-html-demo | 无 | 预编译产物 `src/tailwind.css`（已提交，重新生成见该示例 README） |
| custom-ui-html-demo | 无 | 同上 |

- 工具类：`bg-card` / `text-dim` / `text-meta` / `rounded-panel` / `border-border` 等，
  颜色与档位都取自启动器主题（长短名等价，长名形如 `bg-bg-card`）。
- **iframe 类示例（两个 HTML）必须先收主题变量**：页面在 `<head>` 里监听
  `theme-tokens` 消息并写进 `:root`，否则 `bg-card` 这类颜色类算出来是空的，
  只写排版类（`flex` / `gap-*`）才不受影响。
- 组件型主题（前三个）跑在宿主 DOM 里，变量天然可用，不用这套监听。

## 五个示例各自改什么

### custom-main-demo —— 自定义主页面

给服主写「服务器主页」用的样板：公告、活动日历、进服必读、群号、开服时间、赞助名单，
外加一个真的会拉起游戏的「一键进服」按钮，启动阶段实时显示。

- 组件入口是 `src/App.vue`，注册在 `src/entry.ts` 的 `window.__mmlHomePages`；
- 命令 / 事件封装在 `src/bridge.ts`，类型来自 `mml-ipc`；
- 样式由 PostCSS 限定在 `.mml-home` 内，不会污染启动器其它窗口。

### custom-main-html-demo —— 自定义主页面（HTML）

同一块主页区域的 iframe 路线，包根只有 `index.html` / `style.css` / `app.js`：

- 没有 Vue、没有打包器，`npm run pack` 只是复制 + 压 zip；
- 启动器把它放进主页内容区的 sandbox iframe，并注入 `window.mml`；
- 启动器的标题栏、侧栏与其它窗口不受影响；
- 浏览器直接打开 `src/index.html` 就是预览模式，用假数据把界面跑起来。

### custom-ui-demo —— 整页主题

用 `window.__mmlWindowPages` 按窗口 kind 换组件：`main` 返回 `ReplaceWindow`，
直接渲染宿主里的内置主窗口再附加自己的面板，其它窗口返回 `null` 继续用内置页面。

- 主题包不需要把 `MainWindow` 重新打一份，`custom.js` 只有几十 KB；
- 右下角的主题按钮通过 `getCommands` 调 `window_get_gui_config` /
  `window_save_gui_config` 切换亮 / 暗主题；
- 关包即恢复内置窗口，开发构建另有「退出整页界面」按钮兜底。

### custom-ui-html-demo —— 整页 HTML

包根只有 `index.html` / `style.css` / `app.js`，启动器把它放进 iframe 并注入
`window.mml`，整页由自己的 HTML / CSS / JS 决定。

- 没有 Vue、没有打包器，`npm run pack` 只是复制 + 压 zip；
- 自己画标题栏、实例列表与启动按钮，命令与事件都走 `window.mml`；
- iframe 是 opaque origin，拿不到启动器 DOM，也不能用 `localStorage`；
- 浏览器直接打开 `src/index.html` 就是预览模式，用假数据把界面跑起来。

### custom-ui-css-demo —— 自定义 CSS

先挂 `custom.css`、再执行可选的 `custom.js` 入口，用来给内置界面换皮：

- `custom.css` 覆盖 `--accent*` 一组变量，主页面与所有内置窗口一起换色；
- `src/replace.ts` 是入口源码，打包成 `custom.js` 后可以用 `ctx.select` /
  `ctx.replace` 改 DOM，或挂一个 Vue 组件替换原控件；
- `scripts/controls.ts` 读 `mml-vue/ui-kit` 的控件清单，可直接生成覆盖骨架。

## 包格式与约定

- zip 根目录就是包根，整包只套一层文件夹也认；`theme.json` 声明类型，导入时按它判定。
  各示例的清单是手写的 `src/theme.json`，打包时原样复制到包根，改名字 / 版本直接改它。
- 三种包都只认这些固定文件名：`custom.css`、`custom.js`、`index.html`（整页 iframe）。
- 包里的脚本按 classic script 加载，**不要用 `import` / `export`**；组件回调包
  还要靠 `document.currentScript` 分辨是哪个包在注册。
- Vue 不打进包：入口把 `vue` 标成 external，映射到页面全局 `__mmlVue`。
- 图片、字体等静态资源放子目录，CSS 里用相对路径引用；CSS 包走 `/css/<id>/<路径>`，
  整页包与主页包共用同一套包内路径解析。
- 导出的包在启动器里按上面的表导入；恢复内置界面只需关掉对应开关或删掉包。

## 与启动器共享的契约

- `mml-ipc`：生成的命令包装、事件订阅、DTO，以及手写的 `window.mml` 契约
  （`MmlApi`）和自定义界面类型（`WindowPageRegistry` / `HomePageRegistry` /
  `CustomCssContext` 等）；前三个示例按 type-import 取，HTML 示例只用运行时的
  `window.mml`，不各写一份类型。
- `mml-vue/ui-kit`：控件与样式变量的唯一来源；改完 `mml-vue` 的控件后重跑
  `node mml-vue/scripts/gen-ui.ts`，示例拿到的清单跟着更新。
- `mml-vue/tailwind.css`：Tailwind 入口（不含 preflight），工具类直接映射启动器的
  主题变量，用法见下一节。

### 用 Tailwind 写主题

组件型主题（`custom.js`）跑在启动器页面里，可以直接用 Tailwind：颜色与档位取自启动器
当前主题，明暗切换、换强调色都跟着变。

```bash
npm i -D tailwindcss @tailwindcss/vite
```

```ts
// vite.config.ts
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({ plugins: [vue(), tailwindcss()] });
```

```css
/* 打包后就是 custom.css：先引入口，再写自己的样式 */
@import "mml-vue/tailwind.css";

.hero {
  @apply bg-card text-fg border border-border rounded-panel p-4 text-meta;
}
```

- 可用工具类：底色 `bg-page` / `bg-side` / `bg-card` / `bg-hover` / `bg-raised` /
  `bg-modal` / `bg-overlay`，文字 `text-fg` / `text-dim` / `text-sub`，强调 `text-accent` /
  `bg-accent-soft` / `border-accent-border` / `text-on-accent`，状态 `text-green` / `text-red` /
  `text-yellow`；字号 `text-caption` / `text-meta` / `text-body` / `text-title` / `text-heading` /
  `text-display`；圆角 `rounded-ctl` / `rounded-card` / `rounded-panel` / `rounded-pill`；
  控件高 `h-ctl`。
- 另有 `bg-bg-card`、`text-text-dim` 这类长名（工具类名 = 前缀 + token 名），两套等价，
  新代码用短名即可。
- 入口**不含 preflight**：它要挂进启动器页面，重置会把启动器自己的按钮与边距打乱。
  需要重置就在自己的选择器里局部写。
- 别在自己的样式里写字面色值，深浅主题会同时失效。
- **整页 iframe 主题**（`index.html`）拿不到宿主的 CSS 变量，但启动器会把变量值
  发过来，见下一节。

### iframe 主题拿主题色

iframe 是独立文档，宿主的 `--bg-card` 这类变量不会继承进去。启动器在握手后、
以及主题 / 强调色 / 纯色模式变化时，会往 iframe 发一条消息（整页包与主页面包通用）：

```ts
// 父 → iframe
{ __mml: 1, type: "theme-tokens", theme: "Dark" | "Light", tokens: Record<string, string> }
// iframe → 父：主动要一次（页面加载晚于握手时用）
{ __mml: 1, type: "theme-tokens?" }
```

`tokens` 是「变量名 → 计算值」，例如 `{ "--bg-card": "rgb(34 38 46 / 85%)", "--text": "#e8eaed" }`，
取不到的项不会出现。页面侧最小写法：

```html
<script>
  // 只认父窗口发来的、带协议标记的消息
  window.addEventListener("message", (e) => {
    if (e.source !== window.parent) return;
    const data = e.data;
    if (!data || data.__mml !== 1 || data.type !== "theme-tokens") return;
    const root = document.documentElement;
    for (const [name, value] of Object.entries(data.tokens)) {
      root.style.setProperty(name, value);
    }
    root.dataset.mmlTheme = data.theme;
  });
  // 页面自己决定什么时候要：父窗口也会在握手后主动发一次
  window.parent.postMessage({ __mml: 1, type: "theme-tokens?" }, "*");
</script>
```

拿到之后 `bg-card` 这类工具类就能用了，做法和组件型主题一样：

```css
@import "mml-vue/tailwind.css"; /* 打包成页面自己的 css，跟 index.html 一起进 zip */
```

- 变量是**值**不是引用：页面刷新后要重新收一次，父窗口会在 iframe `load` 时补发。
- 页面用 `window.mml` 时也能自己监听，消息格式与上面一致，不用另写一套。
- `--accent-grad` 这类渐变不放进来，需要渐变就在页面里用 `--accent` 自己拼。

各示例的详细步骤、可改位置和实现说明见各自 README。
