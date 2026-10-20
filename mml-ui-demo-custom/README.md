# mml-ui-demo-custom —— 最小窗口替换主题

只保留两个源文件：

| 文件 | 作用 |
| --- | --- |
| `src/ReplaceWindow.vue` | 直接渲染启动器原始 `MainWindow`，右下角额外加「主题按钮」和面板 |
| `src/replace.ts` | 窗口页面回调：`main` 返回 `ReplaceWindow`，其它窗口返回 `null` |

没有 `App.vue`、`main.ts`、`bridge.ts`、`index.html`，也不再有 iframe 整页。

## 行为

- 回调由 mml-vue 在 `createApp().mount()` 之前加载并注册，窗口第一帧就是替换后的。
- `ReplaceWindow` 从 `window.__mmlWindows.main` 取宿主里的内置主窗口直接渲染，
  不把 MainWindow 再打一份进主题包；`custom.js` 约 3 KB。
- 右下角按钮和面板是主题新增的内容；主窗口的数据、启动、标题栏拖动都还是内置实现。
- 只替换 `main`，设置 / 下载 / 资源管理等窗口返回 `null`，继续用内置页面。

## 构建

```bash
npm install
npm run build     # vue-tsc + vite lib 构建 + 打包
```

产物：

```
mml-ui-demo-custom.zip
├─ theme.json      主题清单，type 为 page
├─ custom.js      窗口组件回调（IIFE，Vue 走 __mmlVue）
└─ custom.css     ReplaceWindow 的样式
```

`custom.js` 由 `vite.theme.config.ts` 打成 IIFE；`scripts/pack.mjs` 把
`dist-theme/` 里的 `custom.js` 与 CSS 复制成包根的两个文件，并写入声明 `type` 的
`theme.json`。

## 导入

启动器 → 设置 → 界面 → 主题包 → 导入主题包。

包根没有 `index.html` 也认：只要存在 `custom.js`，启动器就走窗口组件回调；
`customPageOn` 开启后主窗口直接换成 `ReplaceWindow`。
