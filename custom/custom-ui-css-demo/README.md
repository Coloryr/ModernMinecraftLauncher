# custom-ui-css-demo —— 自定义 CSS 示例

启动器的「样式包」：导入一个 zip 给内置界面换皮。包根是 `custom.css`，可选再带一个
`custom.js` 入口脚本，另用 `theme.json` 声明 `type` 为 `css`；mml-vue 先挂样式、
再加载脚本，挂载完成后调用入口函数。

入口脚本可以只改 DOM，也可以直接挂一个 Vue 组件——示例里的启动按钮就是
[`src/ReplaceButton.vue`](src/ReplaceButton.vue)，由入口用 `ctx.replace` 挂到原按钮位置。

## 目录里有什么

| 文件 | 作用 |
| --- | --- |
| `custom.css` | 示例皮肤：换主题色 + 替换后启动按钮的外观 |
| `src/replace.ts` | 入口函数：选实例启动按钮，用 `ctx.replace` 挂 Vue 组件 |
| `src/ReplaceButton.vue` | 用 Vue 写的替换按钮 |
| `src/theme.json` | 包清单（`type` 为 `css`），开发者手写，打包时复制到包根 |
| `scripts/pack.ts` | 先 `vite build` 入口，再把 `custom.js` 与 `custom.css` 打成 zip |
| `scripts/controls.ts` | 开发期工具：列控件清单、生成 CSS 骨架 |
| `vite.config.ts` | 入口按 IIFE 打包，Vue 走页面全局 `__mmlVue` |

`scripts/` 与 `src/` 的源码不进 zip（`src/theme.json` 会复制成包根清单）。
`custom.js` 与 `dist/` 是 `npm run pack` 的临时产物，打完就删，不提交。

控件清单由 `mml-vue/scripts/gen-ui.ts` 生成，不在这里手抄：mml-vue 里的控件改名后，
重跑 `node mml-vue/scripts/gen-ui.ts`，这里跟着更新。

## 这个示例做了什么

1. **主题色**：覆盖 `--accent` / `--accent-hover` / `--accent-grad` / `--accent-soft` /
   `--accent-border` 这组变量，主页面与所有内置窗口一起从蓝换成青绿。
   默认蓝与预设强调色都覆盖得到（选择器带 `[data-accent]`，压过 `themes.css` 的预设）；
   用户自己挑的「自定义强调色」走内联变量，优先级更高，示例不覆盖它。
2. **实例启动按钮**：入口函数选到 `.launch-actions .launch-main-btn`，把
   `ReplaceButton.vue` 挂到它旁边：原按钮隐藏、组件显示，点击组件时转发给原按钮的
   `click()`，启动逻辑还是启动器自己的。

## 用 Vue 写替换内容

- `ReplaceButton.vue` 是普通 SFC：props 收文字，`emit("launch")` 抛点击。
- 入口这样替换：

  ```ts
  ctx.replace(btn, {
    component: ReplaceButton,
    props: {
      text: "开始冒险",
      onLaunch: () => btn.click(),
    },
  });
  ```

- mml-vue 用 `createApp` 把组件挂在一个 `display: contents` 的容器里，原控件
  `display: none`；关包时 `unmount`、删容器、恢复原控件。
- 包里的 Vue 不打进去：`vite.config.ts` 把 `vue` 标成 external，映射到页面全局
  `__mmlVue`，由 mml-vue 在 `main.ts` 暴露。这样包只有组件代码，跟页面共用同一份 Vue。
- 组件样式写在 `custom.css`（包根）里；如果写 `<style scoped>`，样式会被 Vite 抽成
  单独的文件，得一起打进包并注入，示例没走这条路。

## 入口脚本契约

包根有 `custom.js` 时，`custom_ui_status` 会给出对应的 `scriptUrl`；mml-vue 的
`lib/customUi.ts` 把它作为 classic script 插进页面。脚本执行时同步注册入口：

```js
window.__mmlCustomCss.push((ctx) => {
  ctx.loadCss();
  const btn = ctx.select(".launch-actions .launch-main-btn");
  if (btn) ctx.replace(btn, { text: " 开始冒险", className: "mml-custom-launch" });
});
```

入口函数在 `app.mount("#app")` 之后执行，拿到这些能力：

- `ctx.id`：包 id，与 `custom_ui/css/<id>.zip` 对应
- `ctx.loadCss()`：确保这个包的 `custom.css` 已挂上，重复调用无副作用
- `ctx.select(selector)`：`document.querySelector`
- `ctx.replace(target, options)`：
  - `text` / `className` / `style`：直接改原控件
  - `component` + `props`：隐藏原控件，用 Vue 挂载组件；此时 `text` 不再生效
- `ctx.onCleanup(fn)`：包被关闭 / 删除 / 换脚本时调用，用来还原

示例里的启动按钮随实例列表异步出现，所以入口用 `MutationObserver` 等它进 DOM。
注意脚本必须是 classic script，不要用 `import` / `export`；入口注册也要同步执行，
否则 `document.currentScript` 已经指不到脚本，注册表分不清是哪个包。

## 用法

```bash
npm install          # 链接 mml-vue，装 Vite 构建入口

npm run controls     # 列出全部可改控件
npm run controls -- --filter btn      # 只看按钮相关
npm run controls -- --variables       # 列出可用的 CSS 变量
npm run controls -- --pick            # 交互选控件，生成覆盖骨架

npm run pack         # vite build 入口 + 打出 custom-ui-css-demo.zip
```

`npm install` 之前会先跑 `scripts/deps.mjs`：上级目录有本仓库就直接链到
`../../mml-vue`、`../../mml-ipc`，没有就浅克隆到 `.mml-deps/repo` 再链；`.mml-deps/`
是生成物，要手动重建跑 `npm run deps`，克隆地址可用 `MML_REPO_URL` 覆盖。

`npm run pack` 的流程：Vite 把 `src/replace.ts` + `ReplaceButton.vue` 打成 IIFE
`dist/custom.js` → 复制到包根 → 从 `src/theme.json` 复制包清单 → 和 `custom.css` 一起压 zip
→ 删掉临时复制的 `custom.js`、`theme.json` 与 `dist`。

然后「设置 → 界面 → 样式包 → 导入样式包」。可以装多份，每份单独开关，启用的按列表顺序叠加。

## 选择控件

`--pick` 会按分类列出控件（控件 / 部件 / 标题栏 / 各窗口），输入编号即可生成骨架：

```css
/* 控件 · styles/controls.css */
.ui-btn {
  /* 在这里写覆盖样式 */
}
```

只列全局样式文件里的类；组件 `<style scoped>` 里的类是内部实现，升级后可能改名，
不在清单里。主题变量（`--bg` / `--accent` …）用 `--variables` 看。

## 包格式

zip 根目录：

- `custom.css`：必需，入口样式表
- `theme.json`：声明 `type` 为 `css`；源文件在 `src/theme.json`，由开发者手写，
  打包时复制到包根。带 `custom.js` 的包没有它会被当成类型不明
- `custom.js`：可选，入口脚本；IIFE，Vue 从页面全局 `__mmlVue` 取
- 图片 / 字体放子目录，CSS 里用相对路径引用：

```css
.brand-logo { background-image: url(assets/logo.png); }
```

整包只套一层文件夹也认。单份包有条目数与解压后大小上限，导入时校验。

## 写自定义 CSS 的注意点

- 注入的 `<link>` 在页面最后，正常规则后写覆盖先写；Vue 的 `<style scoped>` 带
  `[data-v-xxx]`，优先级高于单类选择器，必要时加 `!important` 或更具体的选择器。
- 颜色优先用变量（见 `styles/themes.css`），暗色 / 亮色两套都不用手写两份。
- 选择器对着启动器 DOM 写，升级后类名可能变；优先挑 `styles/` 里登记为"唯一来源"的类
  （`.ui-btn` / `.field-*` / `.window-frame` 等）。
- 不要用 `*` 或 `body > *` 这类大范围重置，会把内置布局搞坏。
- 只作用于启动器内置界面；自定义主页面 / 整页界面是独立文档，样式进不去。
- 入口脚本在页面主文档里跑，能改 DOM 也能挂 Vue 组件；清理函数里把改过的东西还原，
  否则用户关掉包之后界面会停在改过的状态。
