<script setup lang="ts">
// 小图标：原来用文字字形当图标的地方统一走这里，如 ＋ ✓ ✕ ‹ › ▾ − ×
//
// 为什么不留着字形：
// - 字形形状 / 粗细 / 位置随字体走：中文字体把 ＋ 画在 em 框偏上，盒子居中了笔画仍显偏上，
//   AccountSelector 里已经踩过一次，见那里的注释；
// - ▾ 这类三角按字号画得太小，粗细还随字号变形；
// - 字号一改图标就变形，而 SVG 的尺寸由 size 定死，线宽由 weight 调。
//
// 统一 24 视窗 + stroke=currentColor（跟随所在处的字体色）+ 圆头圆角，
// 与仓库里其它内联图标同一套画法，那边有 86 处 stroke="currentColor"。
export type GlyphName =
  | "plus"
  | "minus"
  | "check"
  | "close"
  | "chevron-left"
  | "chevron-right"
  | "chevron-down"
  | "play"
  | "gear"
  | "document"
  | "save"
  | "news"
  | "check-square"
  | "image"
  | "download"
  | "refresh"
  | "folder"
  | "folder-plus"
  // 以下为资源分类，见 windows/resource/types.ts 的 RESOURCE_CATEGORIES
  | "archive"
  | "package"
  | "palette"
  | "server"
  | "sun"
  | "cube";

withDefaults(
  defineProps<{
    /** 图标名 */
    name: GlyphName;
    /** 边长，单位 px */
    size?: number;
    /** 线宽 */
    weight?: number;
  }>(),
  {
    size: 14,
    weight: 2,
  },
);
</script>

<template>
  <svg class="glyph" :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    :stroke-width="weight" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path v-if="name === 'plus'" d="M12 5v14M5 12h14" />
    <path v-else-if="name === 'minus'" d="M5 12h14" />
    <path v-else-if="name === 'check'" d="m5 13 4 4L19 7" />
    <path v-else-if="name === 'close'" d="M6 6l12 12M18 6L6 18" />
    <path v-else-if="name === 'chevron-left'" d="m15 6-6 6 6 6" />
    <path v-else-if="name === 'chevron-right'" d="m9 6 6 6-6 6" />
    <path v-else-if="name === 'chevron-down'" d="m6 9 6 6 6-6" />
    <path v-else-if="name === 'play'" d="M7.5 5.2v13.6L19 12 7.5 5.2z" />
    <!-- 齿轮与顶栏功能按钮上那个是同一个画法，见 MainTopbar 的 gear -->
    <g v-else-if="name === 'gear'">
      <circle cx="12" cy="12" r="3.2" />
      <path
        d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.03-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.03 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03z" />
    </g>
    <!-- 文档（实例日志）：纸张轮廓与 FileTree 的文件图标同款，另加两行文字 -->
    <g v-else-if="name === 'document'">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z" />
      <path d="M14 2v6h6" />
      <path d="M8 13h8M8 17h5" />
    </g>
    <!-- 保存：软盘轮廓 + 下沿标签 + 上沿滑盖。原来拿 check 代过，一眼看不出是"存" -->
    <g v-else-if="name === 'save'">
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
      <path d="M17 21v-8H7v8" />
      <path d="M7 3v5h8" />
    </g>
    <g v-else-if="name === 'news'">
      <path d="M4 6a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v13H6a2 2 0 0 1-2-2V6z" />
      <path d="M17 9h2a1 1 0 0 1 1 1v6a2 2 0 0 1-2 2" />
      <path d="M7 8h7M7 12h7M7 16h4" />
    </g>
    <g v-else-if="name === 'check-square'">
      <rect x="3.5" y="3.5" width="17" height="17" rx="4" />
      <path d="m8.5 12.4 2.6 2.6 4.6-5.4" />
    </g>
    <!-- 无图占位：与 lib/imageFallback.ts 里那张破图占位同一套画法 -->
    <g v-else-if="name === 'image'">
      <rect x="3" y="3" width="18" height="18" rx="3" />
      <circle cx="8.5" cy="9" r="1.5" />
      <path d="m21 15-5-5L5 21" />
    </g>
    <g v-else-if="name === 'download'">
      <path d="M12 3v12" />
      <path d="m7 10 5 5 5-5" />
      <path d="M4 20h16" />
    </g>
    <!-- 刷新：一圈带缺口的圆 + 箭头（缺口在右上，箭头落在缺口处） -->
    <g v-else-if="name === 'refresh'">
      <path d="M20.5 12a8.5 8.5 0 1 1-2.5-6" />
      <path d="M20.5 4v5h-5" />
    </g>
    <!-- 文件夹（不带加号）：分组相关的"选一个分组"这类动作用它 -->
    <g v-else-if="name === 'folder'">
      <path d="M4 7a2 2 0 0 1 2-2h3.2l2 2.4H18a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7z" />
    </g>
    <!-- 新建分组：文件夹 + 一个加号，与「添加」系列的 plus 同一套画法 -->
    <g v-else-if="name === 'folder-plus'">
      <path d="M4 7a2 2 0 0 1 2-2h3.2l2 2.4H18a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7z" />
      <path d="M12 11v6M9 14h6" />
    </g>
    <!--
      资源分类的图标，见 resource/types.ts：
      都按"一眼能认出是什么"来画，且与上面那些同一套 24 视窗 + 描边画法。
    -->
    <!-- 存档：带盖的箱体 + 中间一道横线（世界存档） -->
    <g v-else-if="name === 'archive'">
      <rect x="3" y="4" width="18" height="4" rx="1.5" />
      <path d="M5 8v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8" />
      <path d="M10 12h4" />
    </g>
    <!-- 模组：立方体包裹（六边形 + 三条棱） -->
    <g v-else-if="name === 'package'">
      <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4z" />
      <path d="M4 7.4l8 4.6 8-4.6" />
      <path d="M12 12v9.2" />
    </g>
    <!-- 资源包：调色盘（画材 = 材质 / 贴图） -->
    <g v-else-if="name === 'palette'">
      <path
        d="M12 3a9 9 0 1 0 0 18c1 0 1.6-.7 1.6-1.5 0-.4-.2-.8-.5-1.1-.3-.3-.4-.6-.4-1 0-.8.7-1.5 1.5-1.5H16a5 5 0 0 0 5-5c0-4.4-4-8-9-8z" />
      <circle cx="7.5" cy="11.5" r="1.2" />
      <circle cx="10" cy="7.8" r="1.2" />
      <circle cx="15" cy="8.2" r="1.2" />
    </g>
    <!-- 服务器：两层机架 + 指示灯 -->
    <g v-else-if="name === 'server'">
      <rect x="3" y="4" width="18" height="7" rx="2" />
      <rect x="3" y="13" width="18" height="7" rx="2" />
      <path d="M7 7.5h.01M7 16.5h.01" />
    </g>
    <!-- 光影包：太阳（光照 / 阴影） -->
    <g v-else-if="name === 'sun'">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4" />
    </g>
    <!-- 结构文件：等距立方体（建筑结构） -->
    <g v-else-if="name === 'cube'">
      <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4z" />
      <path d="M4 7.4l8 4.6 8-4.6M12 12v9.2M8 5.1l8 4.6" />
    </g>
  </svg>
</template>

<style scoped>
.glyph {
  /* 与文字同行时（如"＋ 添加一行"）按文字中线对齐，别骑在基线上；
     当 flex 子项时 vertical-align 不参与，由父级的 align-items 居中 */
  display: inline-block;
  vertical-align: -0.14em;
  flex-shrink: 0;
}
</style>
