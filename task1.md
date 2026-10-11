# task1：登录卡片直接拖入（HTML5 拖放 + WebView2 附加对象）

> 临时任务文件，做完即删（AGENTS.md §6.1）。只记这一件事，不写"已完成记录"。

## 1. 问题（已定位，逐条有据）

现状：从浏览器把网页上的"登录卡片"拖到启动器窗口，**鼠标显示"禁止"、窗口毫无反应**（用户实测）。

原因链：

1. Tauri 默认开启内部拖放：`drag_drop_enabled` 在 `tauri-runtime` 的 `WebviewAttributes::default()` 里是 `true`，
   本项目建窗（`windows/lifecycle.rs::create_window`）从没关过。
2. wry 的 Windows 实现（`wry/src/webview2/drag_drop.rs`，文件头自述
   `// A silly implementation of file drop handling for Windows!`）会 `EnumChildWindows` 找出 WebView2 的子窗口，
   逐个 `RevokeDragDrop` + `RegisterDragDrop` —— **顶掉 webview 自己的拖放目标**；而它只读 `CF_HDROP`（真实文件），
   拿不到就 `return Ok(())` 且不设 effect → 系统画"禁止"光标，webview 连 `dragenter` 都收不到。
3. wry 的 `DragDropEvent` 只有 `paths` / `position`，**没有文本字段** —— "文本 / 链接"这条数据链在 API 面就不存在。
4. 上游定性：[tauri#14373](https://github.com/tauri-apps/tauri/issues/14373) 正文说这个开关的实质是
   "internal, alternative, file-focused, **exclusive**, enabled by default"；维护者 FabianLars 回复
   "this is a technical limitation (**on Windows at least**) that we can't directly fix ourselves"。
5. 上游修法方向：[wry#1829](https://github.com/tauri-apps/wry/pull/1829)（macOS，未合并）——"文件走原生、**非文件转发给 webview**"。

结论：**在 wry 的 Windows 实现下"文件路径"与"HTML5 拖放"是排他的**，但这不是 Windows 的限制，是 wry 的实现选择。

## 2. 方案（现成手法，来自 tauri-plugin-windows-file-drop）

关键 API：WebView2 的 `window.chrome.webview.postMessageWithAdditionalObjects(msg, e.dataTransfer.files)`
+ 原生 `ICoreWebView2WebMessageReceivedEventArgs2::AdditionalObjects()` → `ICoreWebView2File::Path()`
—— **不占用 OLE 拖放目标也能拿到真实绝对路径**。

于是两者兼得：

- 关掉 wry 的拖放 → WebView2 自己的拖放目标保留 → **HTML5 拖放全功能**（卡片文本 / `text/uri-list` / `text/html` 都拿得到）；
- 注入的 JS 把 `dataTransfer.files` 交给原生 → 文件**真实路径**照旧。

### 依赖核验（已逐项确认，无需新版本）

| 需要 | 状态 |
| --- | --- |
| `webview2-com 0.39.1`（与 tauri 2.12.2 用的同一个版本，绑 windows 0.62 = 工作区版本） | 已在依赖图里 |
| `AdditionalObjects` / `ICoreWebView2File::Path` / `WebMessageReceivedEventHandler` / `ExecuteScriptCompletedHandler` | 全在 0.39.1 里 |
| `TauriPlugin::on_webview_ready`（挂在 plugin Builder 上）+ `PlatformWebview::controller() -> ICoreWebView2Controller` | tauri 2.12.2 都有 |
| `WebviewWindowBuilder::disable_drag_drop_handler()` | 存在 |

### 步骤

1. `mml-gui/src-tauri/src/windows/lifecycle.rs::create_window`：建窗时加 `.disable_drag_drop_handler()`
   （所有窗口统一关；路径改由我们自己的事件送，`tauri://drag-drop` 不再触发）。
2. 新增 `mml-gui/src-tauri/src/windows/file_drop.rs`（`#[cfg(windows)]`）：
   - `install(app, label, &ICoreWebView2)`：`ExecuteScript` 注入 JS（`drop` 且有文件 → `postMessageWithAdditionalObjects`）；
   - `add_WebMessageReceived` → 认消息串 → `AdditionalObjects` → `ICoreWebView2File::Path()` → `emit_file_drop(app, label, paths)`。
3. `mml-gui/src-tauri/src/lib.rs`：注册一个内联 plugin，用 `.on_webview_ready(|webview| ...)`
   → `webview.with_webview(|w| w.controller().CoreWebView2())` 调 `file_drop::install`。
4. DTO 与事件（命令与 DTO 的唯一来源是 Rust，AGENTS.md §4）：
   - `dtos/window_dto.rs`：`FileDropDto { paths: Vec<String> }`；
   - `windows/mod.rs`：`#[gui_macros::emit] fn emit_file_drop(app, label, payload)`，用 `emit_to` **只发给目标窗口**。
5. Cargo：`mml-gui` 加 `[target.'cfg(windows)'.dependencies] webview2-com = "~0.39"`、`windows.workspace = true`。
6. 前端 `mml-vue/src/composables/useFileDrop.ts` 重写为两个来源汇成一个回调 `onDrop({ paths, url })`：
   - HTML5（window 级 `dragenter` / `dragover` / `dragleave` / `drop`）：enter/leave 管遮罩；
     `drop` 里依次读 `text/uri-list` → `text/plain` → 解析地址（卡片）；
   - 新事件（路径）→ 文件流程；
   - `accept` 判定保留（资源窗口"目录 / 登录快捷方式不亮遮罩"那条）。
7. 消费端接线（沿用现有 `WindowDropParam` 管道）：
   - 主窗口：`url` → 开「账户管理」预填；`paths` → 现有整合包 / 文件夹流程；
   - 账户窗口：`url` → `applyAuthDrop()`；
   - 添加实例 / 资源窗口：只看 `paths`。
8. 消掉那条 console "JSON error"：Tauri 也会收到同一条 web message 并尝试按 IPC 解析 → 报错（插件作者直接放着）。
   可选：把消息串换成 Tauri 会忽略的形状，或接受并写进注释。

## 3. 现状（本文件写下时工作区里已有什么）

- 主窗口 / 添加实例 / 资源窗口的**文件拖入**（整合包、文件夹、资源、数据包）：已完成（原生路径那条链）。
- **登录快捷方式文件**（`.url` / `.webloc` / `.website` / `.txt` / `.json`）→ 账户窗口预填：已完成
  （`window_probe_drop` + `read_shortcut_url` / `single_url_of_text`，冲突弹窗见 `resource/parts/DropConflictModal.vue`）。
- **卡片直收**：未做（就是本任务）。
- `[TEMP]` 诊断日志（**做完本任务要删**）：
  - `windows/lifecycle.rs` 的 `WindowEvent::DragDrop` 打点；
  - `useFileDrop.ts` 的 HTML5 探针（`dragenter` / `drop` 打 types / files / text / uri / html）。

## 4. 验收

1. 从 LittleSkin 页面把卡片**直接拖到主窗口** → 光标变"复制"、亮遮罩、松手后打开「账户管理」且外置登录地址已填好。
2. 同样拖到**「账户管理」窗口** → 直接弹「添加账户」表单并预填。
3. 拖整合包 / 文件夹 / 资源文件：与现在完全一致（真实路径、原有流程与冲突弹窗）。
4. 拖目录到资源窗口：仍然不亮遮罩。
5. 浏览器预览（非 Tauri）不受影响。

## 5. 风险与回退

- 关掉 wry 拖放后 `tauri://drag-drop` **不再触发**：前端必须只走新事件，别留两套并存。
- `ExecuteScript` 注入失败时文件拖入会退化成"只有文件名"：要有日志兜底（`mml_log`），别静默。
- 回退方式：把 `disable_drag_drop_handler()` 去掉即可回到当前行为（`.url` 快捷方式那条路仍在）。

## 6. 验证命令

```powershell
cargo check -p mml-gui
cargo test -p mml-gui --lib
cd mml-vue; npm run build
```

实机按第 4 节 5 条逐条走。
