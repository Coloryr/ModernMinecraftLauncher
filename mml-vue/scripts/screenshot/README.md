# 前端页面截图

把启动器的每个窗口按固定视口渲染成 PNG，不需要启动 Rust 后端，也不需要真实数据。

## 用法

```bash
cd mml-vue
npm run shots                           # 全部场景
npm run shots -- main-grid              # 只跑一张，名字见 docs/screenshots.md
npm run shots -- --scene=main-grid      # 同上
npm run shots -- --scene=main-grid,settings-ui   # 一次跑多张
npm run shots -- --only=settings        # 只跑 id 含 settings 的场景
npm run shots -- --skip=lobby           # 排除 id 含 lobby 的场景
npm run shots -- --list                 # 只列场景 id
npm run shots -- --list=md              # 输出 Markdown 表
npm run shots -- --clean                # 先清空旧截图再跑
npm run shots -- --help                 # 全部参数
```

全部场景名与视口见仓库的[前端页面截图目录](../../../docs/screenshots.md)，
也可以直接 `npm run shots -- --list`。场景名就是产物文件名去掉 `.png`。

只跑部分场景时，输出目录里的其他截图与索引会保留，方便逐张补齐；要整批重来才加 `--clean`。

要求：

- Node.js 20.19+（用到内置 WebSocket 与 fetch）；
- Microsoft Edge（默认 `C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe`，
  可用 `--edge=<路径>` 指定）；
- 端口 1420 空闲。脚本发现 Vite 没跑会自己起 `npm run dev`，跑完一起收掉。

产物在仓库根目录 `target/screens/`（`target/` 已在 `.gitignore`）。

每次运行结束会删掉临时的 `target/temp/shot-edge-<pid>/`；如果进程异常退出，下次启动会清掉
已退出进程留下的目录。Vite 与 Edge 都按进程树关闭，不会留下占着 1420 端口的子进程。
`screens/` 是产物，保留：

- `<场景 id>.png`：每个场景一张，视口尺寸由场景的 `size` 决定；
- `index.html`：按场景 id 排列的索引页，失败与未 mock 的命令一并标出；
- `report.json`：每个场景的失败原因、页面错误、未 mock 的命令。

## 控件预览

`controls/` 是只给截图用的控件总览页，不进启动器本体。

```bash
npm run shots -- --only=control              # 全部控件图
npm run shots -- --scene=control-buttons     # 单张
```

也可以起 Vite 自己看：打开 `http://localhost:1420/scripts/screenshot/controls/index.html`，
加 `?section=buttons` 只看一个分区。分区有：`buttons` / `inputs` / `selects` /
`select-open` / `switches` / `segmented` / `steppers` / `search` / `form` / `date` /
`date-open` / `modal` / `collapse` / `tabs` / `icons`。

加控件时改 `controls/ControlsGallery.vue`，再在 `scenarios.mjs` 的「控件」段补一条带 `path` 的场景。

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
  带 `path` 的场景会直接打开那个页面，不拼 `?window=<kind>`，控件预览就是这么跑的。

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

3. 跑一次 `npm run shots -- --scene=<新场景 id>` 检查输出；
4. `npm run shots:docs` 重新生成 `docs/screenshots.md`，新场景若用了词元表里没有的生词，
   表格里会原样出现，补进 `catalog.mjs` 的 `TOKEN` 或 `OVERRIDE` 即可。

常用选择器：

- 设置页签：`.tab-rail .tab-item`（按文字找，见 `clickTab`）；
- 资源分类：`.cat-nav .cat-item`；
- 窗口级按钮 / 弹窗：直接在 `actions` 里写 `document.querySelector(...).click()`。

## 注意

- 假数据是**界面级**的：不跑网络、不读磁盘，列表之外的交互（安装、启动、下载）不会真的发生。
- 截图关掉了动画（`html.no-anim`），但滚动位置、系统字体与真实运行环境可能有细微差异。
- 只用于界面走查与视觉回归；不要作为功能测试。
- 新增场景后先看 `report.json` 里的 `unknown`：那里是还没准备假数据的命令。
