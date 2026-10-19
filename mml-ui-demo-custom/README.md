# mml-ui-demo-custom —— 整页自定义界面示例

启动器「自定义界面」的第二种模式：**整个主窗口换成这一页**。标题栏、侧栏、实例列表、
启动按钮、窗口按钮都由页面自己画，通过 `window.mml` 调启动器命令。

和 [mml-ui-demo](../mml-ui-demo)（自定义主页面）的区别：

| | 自定义主页面 | 整页自定义界面 |
| --- | --- | --- |
| 换掉的范围 | 主窗口**内容区** | 整个主窗口（含标题栏与窗口按钮） |
| 入口 | 设置 → 客户端设置 → 自定义主页面 | 设置 → 界面 → 自定义界面 |
| 页面能力 | `window.mml` | `window.mml`（同一套桥接） |

## 快速预览

```bash
# 1. 起启动器前端（1420）
cd mml-vue && npm run dev

# 2. 起本示例（1522）
cd mml-ui-demo-custom && npm install && npm run dev
```

然后打开：

```
http://localhost:1420/?window=main&customPage=http://localhost:1522/
```

`?customPage=` 是开发参数：只在本次会话生效，不写配置。正式使用是在设置里填页面地址
（后端导入 zip 还没接，见 [task1.md](../task1.md)）。开发构建右下角有一枚
「退出整页界面」的逃生按钮，页面地址填错时点它退回内置界面。

## 页面里做了什么

- `src/App.vue`：整页布局。按住标题栏调 `commands.windows.startDragging()`，
  最小化 / 最大化 / 关闭分别对应 `commands.windows.minimize/toggleMaximize/closeWindow`。
- `src/bridge.ts`：把 `window.mml` 接成 `mml-ipc` 的 `commands` 与 `onXxx` 事件函数，
  带宿主探测与 invoke 超时。
- 实例列表 / 启动 / 启动状态：`commands.main.getInstances()`、
  `commands.main.launchGame(uuid, null)`、`onLaunchState` / `onGameExit`。
- 「打开内置设置」保留了一条回内置界面的路：`commands.windows.openWindow("settings", null, null)`。

浏览器直接打开 `http://localhost:1522/` 也能看样式：没有 `window.mml` 时用示例实例，
顶部会标「浏览器预览」。

## 构建

```bash
npm run build      # 类型检查 + 构建 + 打包 mml-ui-demo-custom.zip
```

打包规则与自定义主页面一致：zip 根目录必须是 `index.html`（见 `scripts/pack.mjs`）。

## 包格式（后端接入后）

整页自定义界面压缩包：**zip 根目录放 `index.html`**，其余资源相对引用。
后端要提供导入、常驻读取与自定义协议，见仓库根 [task1.md](../task1.md)。
