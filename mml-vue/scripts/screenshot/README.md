# 前端页面截图

把启动器的每个窗口按固定视口渲染成 PNG，不需要启动 Rust 后端，也不需要真实数据。

## 用法

```bash
cd mml-vue
npm run shots                 # 全部场景
npm run shots -- --list       # 只列场景 id
npm run shots -- --only=main  # 只跑 id 含 main 的场景
```

要求：

- Node.js 20.19+（用到内置 WebSocket 与 fetch）；
- Microsoft Edge（默认 `C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe`，
  可用 `--edge=<路径>` 指定）；
- 端口 1420 空闲。脚本发现 Vite 没跑会自己起 `npm run dev`，跑完一起收掉。

产物在仓库根目录 `target/temp/screens/`（`target/` 已在 `.gitignore`）：

- `<场景 id>.png`：每个场景一张，视口尺寸由场景的 `size` 决定；
- `report.json`：每个场景的失败原因、页面错误、未 mock 的命令。

## 它怎么工作

```
run.mjs
  ├─ 起 Vite（1420）
  ├─ 起 Edge headless（CDP）
  └─ 每个场景：
       Target.createTarget → 注入 mock.js + window.__SHOT__ 假数据
       → Page.navigate(?window=<kind>) → 等 #app 有内容
       → 执行 actions（点击页签 / 等待）
       → Page.captureScreenshot
```

- **mock.js** 在页面脚本之前替换 `window.__TAURI_INTERNALS__`：所有 IPC 命令都走
  `window.__SHOT__.commands` 里的假数据，事件、对话框、窗口 API 给的是无副作用兜底值。
- **data.mjs** 是假数据：实例 / 账户 / 模组 / 设置 / 更新记录……只覆盖界面读到的字段。
- **scenarios.mjs** 是场景表：窗口 kind + 窗口标签 + 视口 + 数据覆盖 + 操作。

窗口标签决定挂哪个窗口（`windows/windowManager.ts` 的 `resolveKind`）：普通窗口是
`mml-<kind>`，窗口族（resource / config / log / add_resource）是 `mml-<kind>-<实例uuid>`，
实例 uuid 同时放在 URL 的 `uuid` 参数上。

## 加一个页面 / 状态

1. 在 `data.mjs` 里补假数据（优先加导出常量，被多个场景复用）；
2. 在 `scenarios.mjs` 里加一条：

```js
{
  id: "resource-mods",
  kind: "resource",
  label: "mml-resource-inst-1",
  size: [1280, 800],
  query: "uuid=inst-1",                 // 窗口族要带实例
  commands: base({ ... }),              // 覆盖公共命令
  actions: [{ eval: clickTab("Java"), after: 500 }],
}
```

常用选择器：

- 设置页签：`.tab-rail .tab-item`（按文字找，见 `clickTab`）；
- 资源分类：`.cat-nav .cat-item`；
- 窗口级按钮 / 弹窗：直接在 `actions` 里写 `document.querySelector(...).click()`。

## 注意

- 假数据是**界面级**的：不跑网络、不读磁盘，列表之外的交互（安装、启动、下载）不会真的发生。
- 截图关掉了动画（`html.no-anim`），但滚动位置、系统字体与真实运行环境可能有细微差异。
- 只用于界面走查与视觉回归；不要作为功能测试。
- 新增场景后先看 `report.json` 里的 `unknown`：那里是还没准备假数据的命令。
