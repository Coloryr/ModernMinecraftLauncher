# custom-ui-demo —— 最小窗口替换主题

只保留这几个源文件：

| 文件 | 作用 |
| --- | --- |
| `src/ReplaceWindow.vue` | 直接渲染启动器原始 `MainWindow`，右下角额外加「主题按钮」和面板 |
| `src/replace.ts` | 窗口页面回调：`main` 返回 `ReplaceWindow`，其它窗口返回 `null` |
| `src/theme.json` | 包清单（`type` 为 `page`），开发者手写，打包时复制到包根 |

没有 `App.vue`、`main.ts`、`bridge.ts`、`index.html`，也不再有 iframe 整页。

## 行为

- 回调由 mml-vue 在 `createApp().mount()` 之前加载并注册，窗口第一帧就是替换后的。
- `ReplaceWindow` 从 `window.__mmlWindows.main` 取宿主里的内置主窗口直接渲染，
  不把 MainWindow 再打一份进主题包；`custom.js` 约 26 KB。
- 右下角按钮和面板是主题新增的内容；主窗口的数据、启动、标题栏拖动都还是内置实现。
- 「主题按钮」通过 `getCommands()` 调 `window_get_gui_config` / `window_save_gui_config`
  切换亮 / 暗主题，按钮本身不是直接写 `window.mml.invoke`。
- 只替换 `main`，设置 / 下载 / 资源管理等窗口返回 `null`，继续用内置页面。

## 构建

```bash
npm install
npm run build     # vue-tsc + vite lib 构建 + 打包
```

产物：

```
custom-ui-demo.zip
├─ theme.json      主题清单，type 为 page
├─ custom.js      窗口组件回调（IIFE，Vue 走 __mmlVue）
└─ custom.css     ReplaceWindow 的样式
```

## 依赖从哪来

`mml-vue` 与 `mml-ipc` 没有发布到 npm，`package.json` 里写的是 `file:.mml-deps/<名字>`，
由 `scripts/deps.mjs` 在 npm 解析依赖之前铺好：

- 上级目录有本仓库（示例在 `custom/<示例>/` 下）→ 直接链到 `../../mml-vue`、`../../mml-ipc`，
  改仓库里的源码立即生效；
- 上级目录没有 → 浅克隆仓库到 `.mml-deps/repo`，只检出这两个包，再链过去。

`.mml-deps/` 是生成物，已忽略；要手动重建跑 `npm run deps`。克隆地址可用
`MML_REPO_URL` 覆盖，指向内网镜像或自己的 fork。

`custom.js` 由 `vite.theme.config.ts` 打成 IIFE；`scripts/pack.mjs` 把
`dist-theme/` 里的 `custom.js` 与 CSS 复制成包根的两个文件，并把 `src/theme.json`
原样复制成包根的清单。

`dist-theme/` 与 `dist/` 都是构建产物，已忽略、不进版本库：前者是 Vite 的输出目录，
后者是打包时组装包根的暂存目录，打完就删。

## 导入

启动器 → 设置 → 界面 → 主题包 → 导入主题包。

包根没有 `index.html` 也认：只要存在 `custom.js`，启动器就走窗口组件回调；
`customPageOn` 开启后主窗口直接换成 `ReplaceWindow`。
