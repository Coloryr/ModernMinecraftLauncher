<script setup lang="ts">
// 资源窗口各分类列表的**通用加载占位**：几行骨架行，呼吸动画在 `styles/skeleton.css`
//
// 只画行、不套容器：滚动区 `.item-list` 由调用方给，它是 `flex: 1; overflow-y: auto` ——
// 六个分类是整块替换，把本组件放进 `<div class="item-list">`，
// 数据包子页是在同一个 `.item-list` 里只换掉行那一块，两种都得能用。
//
// 行形状按**资源行**来，即 `.item-row` = 36px 图标 + 名字 + 副标题，见 resource.css 里对
// `.sk-row` / `.sk-icon` 的微调；所以骨架与真内容逐行等高，加载完切换时列表不跳。
//
// 行数**按最小窗口高度定死 5 行**，不再"铺过一屏"：资源窗口最小客户区 480，
// 即 `MIN_HEIGHT`，见 src-tauri/windows/mod.rs；减标题栏 64、窗口内边距上下各 22、
// 内容区顶栏 35、行间距 8 —— 列表可见高度约 325px，5 行 × 58 + 4 × 8 = 322px。
// 再多一行就会**把滚动条撑出来**：加载占位一出现右边就多一条，数据到手又消失。
//
// `done` / `total` 给的是**扫描进度**（模组那一档才有）：给了就在骨架上方显示一条
// 带横向进度条的"读取中 x/x"，复用通用组件 LoaderQueryProgress；否则只显示骨架。
import LoaderQueryProgress from "../../../components/LoaderQueryProgress.vue";
import { t } from "../../../lib/i18n";

withDefaults(
  defineProps<{
    /** 已完成数；与 total 一起给才显示进度 */
    done?: number;
    /** 总数；0 表示还没数出来，此时不显示进度 */
    total?: number;
  }>(),
  { done: 0, total: 0 },
);
</script>

<template>
  <!--
    加载进度：旋转圈 + "读取中" + 横向进度条 + x / x
    用全仓通用的 LoaderQueryProgress（添加实例窗口那套），不再自己拼一行数字 ——
    只有数字看不出在干什么，也没有进度条的"走了多少"的观感
  -->
  <LoaderQueryProgress :visible="total > 0" kind="query" :step="done" :total="total"
    :label="t('resource.readingMods')" />
  <div v-for="n in 5" :key="n" class="sk-row">
    <div class="sk sk-icon"></div>
    <div class="sk-lines">
      <div class="sk sk-line w60"></div>
      <div class="sk sk-line w90"></div>
    </div>
  </div>
</template>
