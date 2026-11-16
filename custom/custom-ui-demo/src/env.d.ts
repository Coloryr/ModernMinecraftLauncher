/// <reference types="vite/client" />

// 主题里会 import 自己的样式入口（theme.css），Vite 的客户端类型给了
// *.css 这类副作用 import 的声明，缺了它 vue-tsc 会报 TS2882。
