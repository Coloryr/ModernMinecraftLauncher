# mml-ui-demo-custom-css —— 自定义 CSS 示例

启动器「自定义界面」的第一种模式：**不重写页面，只注入一段 CSS**。适合改配色、圆角、
间距，或者用选择器只动某一类控件。

## 快速预览

```bash
# 1. 启动器前端（任一窗口都行，这里看主窗口）
cd mml-vue && npm run dev

# 2. 另开一个终端，起本示例的静态服务（端口 1521）
cd mml-ui-demo-custom-css && npm run dev
```

然后打开：

```
http://localhost:1420/?window=main&customCss=http://127.0.0.1:1521/custom.css
```

`?customCss=` 只在开发时用：它让前端用 `<link>` 直接引这份 CSS，不写本地配置。
正式使用是「设置 → 界面 → 自定义界面 → 自定义 CSS」，把 `custom.css` 的内容贴进去。

## 这份 CSS 演示了什么

| 位置 | 作用 |
| --- | --- |
| `html[data-theme="Dark" / "Light"]` | 改主题变量：`--bg` / `--accent` / `--border` …，所有窗口一起变 |
| `.window-frame` / `.frame-head` | 整窗背景、标题栏毛玻璃、底部描边 |
| `.ui-btn` / `.ui-btn.v-accent` | 只改按钮：胶囊圆角、悬停上浮、强调按钮换渐变 |
| `.field-input` / `.field-select` | 只改输入框 / 下拉的圆角与聚焦光圈 |
| `.sidebar-slot` / `.side-row` | 只改主窗口侧栏 |
| `.frame-head h1::after` | 用伪元素做局部装饰，验证“选择器能只改一处” |

## 写自定义 CSS 的注意点

- 注入的 `<style>` 在页面最后，正常规则后写覆盖先写；但 Vue 的 `<style scoped>` 会带
  `[data-v-xxx]`，优先级高于单类选择器，必要时加 `!important` 或写更具体的选择器。
- 颜色优先用变量（见 `styles/themes.css`），这样暗色 / 亮色两套都不用手写两份。
- 选择器直接对着启动器的 DOM 写，升级后类名可能变；尽量挑 `styles/` 里登记为“唯一来源”的
  类（`.ui-btn` / `.field-*` / `.window-frame` 等）。
- 不要用 `*` 或 `body > *` 这类大范围重置，会把内置布局搞坏。

## 包格式（后端接入后）

自定义 CSS 压缩包目前约定为：**zip 根目录放 `custom.css`**，图片等资源可放子目录。
导入、落盘、多窗口广播还没接，见仓库根 [task1.md](../task1.md)。
