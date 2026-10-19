# 自定义界面（task1）

> 本轮先做**前端骨架 + 两个示例**，后端加载 / 导入 / 落盘留到下一轮。完成后删掉本文件，
> 长期约定写进 AGENTS.md。

## 本轮已完成（前端）

- 两种模式的前端状态与注入：
  - `mml-vue/src/lib/customUi.ts`
    - `off` / `css` / `page`
    - css 模式把 CSS 注入 `<head>` 的 `<style id="mml-custom-ui-css">`
    - 开发参数：`?customCss=<url>`（`<link>` 加载）、`?customPage=<url>`
    - 跨窗口同步：复用 `storage.ts` 的 `onStorageChange` / `emitChange`
  - `mml-vue/src/components/CustomUiPage.vue`：整页 iframe，复用 `customHomeBridge` 的
    `window.mml` 桥接；开发构建右下角有「退出整页界面」逃生按钮
  - `mml-vue/src/App.vue`：`page` 模式只换**主窗口**，设置等工具窗口保留内置界面
  - `mml-vue/src/main.ts`：启动时 `initCustomUi()`
- 设置界面：`设置 → 界面 → 自定义界面`
  - 模式三选一（关闭 / 自定义 CSS / 整页重写）
  - CSS 文本框 + 应用；整页地址输入 + 失焦应用
- 示例：
  - `mml-ui-demo-custom-css/`：`custom.css` + 零依赖静态服务器（1521）
  - `mml-ui-demo-custom/`：整页 Vue 示例（1522），含自绘标题栏、窗口按钮、实例列表、启动
- 预览入口：
  - css：`http://localhost:1420/?window=main&customCss=http://127.0.0.1:1521/custom.css`
  - page：`http://localhost:1420/?window=main&customPage=http://localhost:1522/`

## 待做：后端加载

1. **配置字段**
   - `GuiConfigDto.client` 增加 `customUiMode: "off" | "css" | "page"`（磁盘配置 snake_case）。
   - CSS 与整页包只存文件，不把整段 CSS 塞进 `gui_config.json`。
2. **文件与导入**
   - 目录：运行目录下 `custom_ui/`，放 `custom.css` 与 `page/<入口>` 或常驻 zip。
   - 命令（按 §4 走生成器）：
     - `custom_ui_status` -> 模式 / 是否已导入 / 文件数 / 入口地址
     - `custom_ui_import_css`（选 .css）
     - `custom_ui_import_page`（选 zip，校验根目录 index.html）
     - `custom_ui_remove` / `custom_ui_open_dir`
   - 事件：`custom-ui-change`，所有窗口重读并重新应用；主窗口据此换整页。
3. **页面协议**
   - 新增 `mml-ui://`（或复用 `mml-home://` 的读取逻辑），从 `custom_ui/page/` 现读，
     不整包解压；路径穿越防护与 `mml-home` 同口径。
   - 首屏：前端 `initCustomUi()` 之后要能拿到后端状态；启动时先读配置再挂载，
     避免内置界面闪一下。
4. **多窗口**
   - css 模式：每个 webview 各自注入；设置窗口改完广播，所有窗口立即生效。
   - page 模式：只换主窗口（当前前端已按此实现）；其它窗口保留内置界面作为退路。
5. **持久化**
   - 把 `storage.ts` 里的 `mml.customUi.*` 三个临时键迁到后端文件；迁移后前端只读后端 DTO。
6. **安全**
   - 整页 iframe 保持 `sandbox`、不给 `allow-same-origin`。
   - 自定义 CSS 能加载远程资源（`url()`），说明里要写清楚；是否拦截由后续决定。
   - 导入 zip 校验大小 / 条目数，防 zip 炸弹（与 custom_home 同口径）。

## 待完善（前端）

- 设置项搜索已加两条；「恢复默认」分组未接（`RESETTABLE_GROUPS` 未加 `customUi`）。
- CSS 导入目前只能粘贴文本；后端接入后加「选文件」按钮。
- 整页地址目前是手填 URL；后端接入后换成导入包 + 只读展示入口。
- 页面地址校验（必须 http(s) 或后端协议）未做，错了只能靠开发逃生按钮。
- `mml-ui-demo-custom-css` 只有 CSS + 静态服务，没有打包脚本；后端定下包格式后补 zip。
- 两个示例都没加自动化测试；至少补一条「配置变化后 style/iframe 跟着变」的单测。

## 已知限制（本轮）

- 自定义 UI 配置只存在 localStorage，换机器 / 清缓存会丢。
- 整页模式只在主窗口生效。
- 没有 zip 导入，整页示例靠开发参数 / 手填 URL。
- 自定义 CSS 直接作用于 DOM，升级后类名变化可能失效。
- 多 WebView 下 localStorage 的 storage 事件不保证跨窗口；CSS 模式目前只在改设置的
  那个窗口立即生效，其它窗口的实时同步要等后端 `custom-ui-change` 广播。
