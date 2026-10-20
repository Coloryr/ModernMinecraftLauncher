# mml-ui-demo —— 服务器主页组件示例

M²L 启动器可以把你自己的 Vue 组件当成「启动器主页」（顶栏「主页」按钮切过去的那一页）。
本工程演示的就是**服务器主页**：公告 / 活动日历 / 进服必读 / 群号 / 开服时间 / 赞助名单，
加上一个「一键进服」按钮——点下去真的会调启动器启动游戏，页面还能实时显示启动阶段。

只保留组件入口，没有 `index.html` / `main.ts`：包根是 `custom.js` 与 `custom.css`，
mml-vue 在挂载前加载并渲染组件，不再使用 iframe。

## 快速开始

```bash
npm install
npm run build      # vue-tsc + 组件入口构建 + 打包
```

产物：

```
mml-ui-demo.zip
├─ custom.js       主页组件入口（IIFE，Vue 走 __mmlVue）
└─ custom.css      限定在 .mml-home 内的样式
```

导入：启动器 → **设置 → 客户端设置 → 自定义主页面 → 导入**，选 `mml-ui-demo.zip`，
再打开「启用自定义主页面」开关 → 回到主窗口主页即可看到。

## 组件入口

`src/entry.ts` 把 `App` 注册给 mml-vue：

```ts
(window as ...).__mmlHomePages.push(() => App);
```

- mml-vue 用 `custom_home_status` 取 `scriptUrl` / `styleUrl`，在挂载前加载；
- 组件直接渲染在主窗口主页内容区，`window.mml` 由宿主提供；
- `custom.css` 由 PostCSS 前缀限定在 `.mml-home` 内，不会改到设置 / 下载等窗口；
- 自定义主页包仍然支持 `index.html` + iframe，本示例不用那条回退路径。

## 页面能力：`window.mml`

组件运行在启动器窗口里，宿主会提供：

```js
window.mml = {
  invoke(cmd, args) -> Promise<any>,  // 调启动器命令
  on(event, cb) -> () => void,        // 订阅事件，返回取消订阅函数
  onTheme(cb) -> () => void,          // 订阅主题变化
  theme: "Dark" | "Light",            // 当前主题
  ready: Promise,                     // 宿主握手
};
```

- 底层 `invoke` 收原始命令名 + camelCase 参数；本工程再套一层 `mml-ipc` 的
  `createCommands`（`src/bridge.ts`），命令名与参数类型在编译期检查。
- 事件订阅同理：`createEvents` 由 `mml-ipc` 生成，`src/bridge.ts` 接到 `window.mml.on`。
- 本工程实际用到：

  ```ts
  const list = await commands.main.getInstances();
  await commands.main.launchGame(uuid, null);
  onLaunchState((e) => { /* 启动阶段与进度 */ });
  ```

## 改哪里

服务器内容集中在 `src/App.vue` 顶部的两个常量里：

| 想改什么 | 改哪 |
| --- | --- |
| 服务器名 / 标语 / 滚动公告 | `SERVER.name` / `slogan` / `ticker` |
| 进服方式（地址、群号、版本） | `SERVER.join` |
| 进服必读、开服时间、赞助名单、页脚联系人 | `SERVER.rules` / `schedule` / `sponsors` / `contact` |
| 活动日历 | `EVENTS` |
| 配色（暗色 / 亮色两套） | `src/style.css` 的 `:root` 与 `[data-mml-theme="Light"]` |
| 页面结构 | `src/App.vue` 的 `<template>` |

## 目录

```
src/
├── App.vue         主页内容（服务器信息 + 启动交互）
├── bridge.ts       window.mml → mml-ipc 的命令 / 事件封装
├── entry.ts        注册 __mmlHomePages 回调
├── style.css       主页样式（构建时被限定在 .mml-home 内）
└── vite-env.d.ts   Vite 类型
scripts/pack.mjs    把 dist-theme 打成 custom.js / custom.css
vite.theme.config.ts 组件入口构建配置
```

依赖只有 `mml-ipc` 与 `vue`；不依赖 `mml-vue`，组件由宿主加载。
