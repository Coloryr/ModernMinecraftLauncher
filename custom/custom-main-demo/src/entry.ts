// 自定义主页的组件入口：mml-vue 在挂载前加载本脚本并调用回调
//
// 返回 App 就替换主窗口的主页内容区；返回 null 时回落到包里的 index.html（iframe）。

import type { HomePageRegistry } from "mml-ipc";
import App from "./App.vue";
import "./style.css";
// 主题样式入口：Tailwind 工具类（见 theme.css）
import "./theme.css";

(window as unknown as { __mmlHomePages: HomePageRegistry }).__mmlHomePages.push(() => App);
