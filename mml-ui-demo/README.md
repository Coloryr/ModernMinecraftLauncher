# mml-ui-demo —— 服务器主页示例工程

M²L 启动器可以把你自己的整页 HTML 当成「启动器主页」（顶栏「主页」按钮切过去的那一页）。
本工程是一个可以直接改的 **Vue3 + Vite** 样板，演示的就是**服务器主页**这个用法：
公告 / 活动日历 / 进服必读 / 群号 / 开服时间 / 赞助名单，加上一个「一键进服」按钮——
点下去真的会调启动器启动游戏，页面还能实时显示启动阶段。

`npm run build` 会产出静态站点，并顺手打成一个能直接在启动器里导入的 zip。

> 这是**独立工程**：`node_modules` 独立，不属于启动器仓库的 Cargo workspace，也不挂进
> `mml-vue` 的构建流程；**不要** import `mml-vue/src/` 下的代码。跨 IPC 的类型与命令包装
> 用仓库根的 npm 包 [`mml-ipc`](../mml-ipc)，`npm install` 会按 `file:../mml-ipc` 链接进来。

## 快速开始

```bash
npm install
npm run build      # 产出 dist/ 与 mml-ui-demo.zip
```

`npm install` 会把相邻的 `mml-ipc` 包按 `file:../mml-ipc` 链接进 `node_modules`；
复制本工程单独用时，把 `mml-ipc/` 一起带走，并把路径改到实际位置。

可用的 npm 脚本：

| 脚本 | 说明 |
| --- | --- |
| `npm run dev` | 浏览器预览（`http://localhost:1520`，无 IPC 数据） |
| `npm run build` | 类型检查 + 构建 + 打包 zip（= `build:only` + `pack`） |
| `npm run build:only` | 只构建出 `dist/`，跳过类型检查与打包 |
| `npm run pack` | 只把已有的 `dist/` 打包成 zip |

导入：启动器 → **设置 → 客户端设置 → 自定义主页面 → 导入**，选 `mml-ui-demo.zip`，
再打开「启用自定义主页面」开关 → 回到主窗口的主页即可看到。

导入后启动器会把压缩包**原样保存**在自己的运行目录里（`custom_home.zip`），
页面运行时**直接从压缩包里读取**，不会解压出一堆文件到磁盘上。所以：

- 想换页面：重新构建、重新导入一次即可（改包即生效）；
- 磁盘上不会有 `custom_home/` 之类的解压目录，没有"缓存过期"这种问题；
- 「定位压缩包」按钮会直接在资源管理器里选中那个 zip，方便替换或备份。

## 改哪里

服务器内容集中在 `src/App.vue` 顶部的两个常量里，照着改就行：

| 想改什么 | 改哪 |
| --- | --- |
| 服务器名 / 标语 / 滚动公告 | `SERVER.name` / `slogan` / `ticker` |
| 进服方式（地址、群号、版本） | `SERVER.join`（第一项是服务器地址，用来匹配"推荐"实例） |
| 进服必读、开服时间、赞助名单、页脚联系人 | `SERVER.rules` / `schedule` / `sponsors` / `contact` |
| 活动日历 | `EVENTS` |
| 配色（暗色 / 亮色两套） | `src/style.css` 的 `:root` 与 `[data-mml-theme="Light"]` |
| 页面结构（多一栏、换排版） | `src/App.vue` 的 `<template>` |

启动器交互那一段（拉实例、启动游戏、显示阶段）一般不用动；它是通用写法，注释里标了。

## 页面能力：`window.mml`

启动器会在返回的 HTML 的 `</head>` 前注入 `<script src="/__mml_bridge.js">`，
页面里因此有（类型就是 `mml-ipc` 导出的 `MmlApi`，导入本包即带全局声明）：

```js
window.mml = {
  invoke(cmd, args) -> Promise<any>,  // 调启动器命令
  on(event, cb) -> () => void,        // 订阅事件，返回取消订阅函数
  onTheme(cb) -> () => void,          // 订阅主题变化，返回取消订阅函数
  theme: "Dark" | "Light",            // 当前主题（启动器下发，实时更新）
  ready: Promise,                     // 与父窗口握手
};
```

- 底层 `window.mml.invoke` 收**原始命令名 + camelCase 参数**；本工程再套一层 `mml-ipc` 的
  `createCommands`（`src/bridge.ts` 的 `commands`），命令名与参数类型都在编译期检查。
- 事件订阅同理：`createEvents` 由 `mml-ipc` 生成，每个事件一个 `onXxx`，payload 类型跟着
  事件名走；`src/bridge.ts` 只把它接到 `window.mml.on`，再导出 `onLaunchState` 等。
- 本工程实际用到的三条：

  ```ts
  const list = await commands.main.getInstances();
  await commands.main.launchGame(uuid, null);
  onLaunchState((e) => console.log(e.state, e.progress)); // 事件名常量与 payload 类型来自 mml-ipc
  ```

- 可订阅的事件**白名单**（只有这些，其余会被拒绝）；事件名常量与 payload 类型都在 `mml-ipc`：

  | 事件名 | payload |
  | --- | --- |
  | `launch-state` | `StateEvent`，`{ uuid, state, progress }` |
  | `launch-error` | `ErrorEvent`，`{ uuid, message }` |
  | `game-exit` | `ExitEvent`，`{ uuid, code }` |
  | `game-log` | `LogEvent`，`{ uuid, lines, clear }`（成批下发、量大，注意节流） |
  | `instance-change` | `InstanceChangeEvent`，`{ type }` |

### 四个容易踩的坑

`src/bridge.ts` 已经把这些处理好了，照着用或直接抄：

1. `window.mml` **只在启动器里存在**；用浏览器直接打开页面时是 `undefined`。
2. 不在启动器里时，`invoke` 永远不会有应答（不 resolve 也不 reject）→ **必须自己加超时**，
   否则按钮会一直转圈。
3. `mml.ready` 在没有宿主时也会在约 10 秒后 resolve，**不能**拿它判断「我在不在启动器里」。
   本工程的做法是把一条最轻的命令（`main_load_state`）当探针：**有应答**（成功或报错都算）
   才说明父窗口在。
4. `on()` 返回的取消订阅函数要在页面/组件卸载时调用，否则事件会继续发往已经销毁的页面。

### 暗色 / 亮色（跟随启动器设置）

iframe 是独立文档，**拿不到**启动器页面 `<html data-theme>` 上的 CSS 变量，所以启动器会：

1. 把这页的 `<html>` 属性写成 `data-mml-theme="Dark"` 或 `"Light"`（握手时下发，之后主题变化
   实时更新）；
2. 顺手同步 `color-scheme`，让原生滚动条 / 表单控件跟着变；
3. 另外提供 `window.mml.theme` 与 `window.mml.onTheme(cb)`，给需要读值的场景（如 canvas 绘制）用。

**推荐做法是纯 CSS**（本工程就是这样）：在 `src/style.css` 里写「暗色为默认 +
`[data-mml-theme="Light"]` 覆盖变量」，组件里只用变量。要试效果，把启动器主题切成亮色即可，
页面会立刻跟着变。

另外，**启动器主题设为「跟随系统」时**：如果系统是在窗口打开后才切换深浅色，启动器未必推得
及时，所以桥接脚本自己也在盯着 `prefers-color-scheme`——只要启动器还没下发过明确主题，
系统一变页面就跟着变。用浏览器直接打开页面时同理。

## 目录结构

```
mml-ui-demo/
├── index.html          # 入口页模板；构建后 dist/index.html 就在根目录
├── src/
│   ├── App.vue         # 示例页面（服务器内容集中在顶部 SERVER / EVENTS）
│   ├── bridge.ts       # window.mml 的使用封装：宿主探测 / 带超时 invoke / 类型化命令与订阅
│   ├── main.ts
│   ├── style.css       # 全局样式与两套配色变量
│   └── vite-env.d.ts
├── scripts/pack.mjs    # 构建后打包成 zip（零依赖）
└── vite.config.ts      # base: "./"（资源一律相对路径）；预览端口 1520
```

## 打包规则（为什么 zip 根目录必须是 `index.html`）

启动器导入 zip 时的入口定位：**包内根目录的 `index.html` 优先**；否则若整包只有一个顶层
目录，就取那个目录里的 `index.html`。所以 `scripts/pack.mjs` 打的是 `dist/` 的**内容**
（不是 `dist/` 这层目录），zip 里第一条就是 `index.html`。

打包手段按可用性挑，都是零依赖：

1. **bsdtar**（Windows 10+ 自带的 `tar.exe`、macOS 自带的 `tar`）——写出的条目用正斜杠，最标准；
2. **PowerShell 的 `Compress-Archive`**——Windows 一定有（条目用反斜杠；启动器解包走的是
   zip crate 的 `Utf8WindowsPath`，两种分隔符都能正确还原目录结构）。

## 浏览器里预览

```bash
npm run dev        # http://localhost:1520
```

没连启动器时页面照样渲染：顶部会显示「未检测到启动器」的提示条，实例列表用示例数据，
方便先把样式调好。真实的命令与事件只有在启动器里加载时才生效。

## 安全说明（重要）

**自定义主页面能调用启动器的全部已注册命令**，等价于拿到启动器全部 IPC 权限：删实例、
改配置、装整合包、读本机文件等等都能做。这是「自定义主页面」的**既定设计**（信任级别与
「服主给的整合包本身就是代码」一致），不是漏洞。

因此：

- 只导入**你信任**的 zip，不要随手装别人发来的主页包；
- 页面拿不到启动器页面的 `localStorage` / cookie（iframe 没有 `allow-same-origin`，
  origin 是 opaque），但这**限制不了它能调的命令**；
- 启动器侧唯一的强制防护是 `mml-home` 协议的路径穿越防护（`../../gui_config.json`
  这种请求会 404），它保护的是「页面能读到哪些文件」，不是「页面能做什么」。
