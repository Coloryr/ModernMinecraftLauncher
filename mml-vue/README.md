# mml-vue

M²L 启动器的前端界面（Vue 3 + Vite + TypeScript）。构建产物由 `mml-gui`（Tauri 壳）作为
`frontendDist` 载入，开发时经 `devUrl` 连本项目的 Vite 服务器。

## 运行

```bash
npm install
npm run dev      # http://localhost:1420（与 mml-gui 的 devUrl 一致，端口被占用会直接失败）
npm run build    # vue-tsc --noEmit && vite build → dist/
```

可以脱离 Tauri 在浏览器里预览单个窗口，用 `?window=<kind>` 指定：

```
http://localhost:1420/?window=settings
```

`<kind>` 取 `src/windows/registry.ts` 里的 `WindowKind`。此时没有 IPC 数据，界面按空状态渲染。

## 目录结构

| 路径 | 说明 |
| --- | --- |
| `src/main.ts`、`src/App.vue` | 入口与根组件；按当前窗口标识挂载对应窗口 |
| `src/windows/registry.ts` | 窗口注册表（kind / 标题）。宽高的唯一来源是后端，前端不维护 |
| `src/windows/windowManager.ts` | 开窗 / 关窗 / 聚焦、`?window=` 解析、多窗口与单窗口两种模式 |
| `src/windows/<kind>/` | 各窗口页面；复杂窗口再分 `parts/`、`composables/`、`views/`、`modes/` |
| `src/components/` | 跨窗口通用组件；`src/components/ui/` 是基础控件（按钮 / 开关 / 弹窗 / 骨架屏等） |
| `src/lib/` | 命令包装（`api.ts`）、`bindings.ts`、`i18n/`、`theme.ts`、`storage.ts`、`toast.ts`、`customHomeBridge.ts` 等 |
| `src/composables/` | 通用组合式函数（拖拽、多选、文件拖放、事件退订等） |
| `src/styles/` | 全局样式与主题变量 |

## IPC 契约：来自 mml-ipc

跨 IPC 的类型、命令包装与事件名由 `ipc-gen` 扫描 Rust 源码生成，放在独立的 npm 包
[`mml-ipc`](../mml-ipc)。`src/lib/bindings.ts` 与 `src/lib/listens.ts` 只是兼容层：
前者把 Tauri 的 `invoke` 绑到 `createCommands`，后者再导出事件名，既有导入路径不变。
前端缺命令或字段时，先在 Rust 侧补 `#[gui_macros::command]` 或改 DTO，再重新生成：

```bash
cd ../mml-gui && npm run gen
```

- `src/lib/api.ts` 是**手写**的便捷包装层，其中的类型一律 `import` 自 `./bindings`；
  不要在这里另写一套类型（下次生成会被覆盖，也与 Rust 侧脱节）。
- 命令名 = 函数名 = `窗口名_方法名`（如 `main_get_instances`）；事件名由 `emit_xxx_yyy`
  去掉 `emit_` 前缀、下划线换成 `-` 推导。完整约定见仓库根 [`ipc-gen/`](../ipc-gen/) 的扫描实现。

## 界面约定

- 窗口宽高不在前端维护，唯一来源是后端 `WINDOWS_INFO`（经 `window_get_window_sizes` 下发）。
- 主题、皮肤与头像显示等 GUI 配置走 `window_get_gui_config` / `window_save_gui_config`，
  变更通过事件跨窗口同步。
- 标题栏自绘：窗口先以原生装饰创建并隐藏，前端量出按钮矩形交给 `tauri-plugin-decoration`
  激活，以保留 Win11 贴靠布局（见 `src/lib/decoration.ts`）。
- 自定义主页面走 iframe + `src/lib/customHomeBridge.ts` 的 `postMessage` 桥接，
  事件有白名单限制。
