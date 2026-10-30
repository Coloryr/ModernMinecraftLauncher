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
// 与仓库里其它内联图标同一套画法。
export type GlyphName =
  | "plus"
  | "minus"
  | "check"
  // 方块收藏：悬停露出的星框 / 已收藏的实心星
  | "star"
  // 模组顶栏的批量操作：启用 / 禁用 / 删除，见 windows/resource 的 ModPane
  | "ban"
  | "trash"
  // 依赖图的节点标记：装着了但版本合不上
  | "alert"
  | "close"
  | "search"
  | "chevron-left"
  | "chevron-right"
  | "chevron-down"
  // 自定义 CSS 的顺序调整：上移 / 下移
  | "arrow-up"
  | "arrow-down"
  | "play"
  // 设置窗口左侧导航：手柄（按键映射）
  | "gamepad"
  // 自定义音乐的标题栏入口：暂停与停止
  | "pause"
  | "stop"
  // 启动按钮运行中悬停的「关闭游戏」
  | "power"
  | "gear"
  | "document"
  | "terminal"
  | "wrench"
  // 主页面实例操作：日志窗口（一屏日志）与配置文件修改（可调参数）
  | "log"
  | "sliders"
  | "save"
  | "news"
  | "check-square"
  // 模组工具条的更新记录入口：时钟 + 回绕箭头，读作"历史"
  | "history"
  | "puzzle"
  | "image"
  | "download"
  // 导出：与「下载」互为镜像，表示把东西从这里送出去
  | "export"
  | "update"
  | "refresh"
  | "folder"
  | "folder-plus"
  // 以下为资源分类，见 mml-vue/src/windows/resource/types.ts 的 RESOURCE_CATEGORIES
  | "archive"
  | "package"
  | "palette"
  | "server"
  | "sun"
  | "cube"
  // 主窗口侧栏：向右占满 / 收回（四角外扩与内收的括号）
  | "maximize"
  | "restore"
  // 实验性功能：锥形瓶
  | "flask"
  // 设置窗口左侧导航：人像（皮肤与头像）、咖啡杯（Java）
  | "user"
  | "coffee";

withDefaults(
  defineProps<{
    /** 图标名 */
    name: GlyphName;
    /** 边长，单位 px */
    size?: number;
    /** 线宽，无单位，对应 SVG 的 stroke-width */
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
    <path v-else-if="name === 'star'"
      d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
    <path v-else-if="name === 'close'" d="M6 6l12 12M18 6L6 18" />
    <!-- 禁用：禁止符（圆圈 + 斜杠） -->
    <g v-else-if="name === 'ban'">
      <circle cx="12" cy="12" r="9" />
      <path d="m5.6 5.6 12.8 12.8" />
    </g>
    <!-- 删除：垃圾桶（桶盖 + 桶身 + 两道竖线） -->
    <g v-else-if="name === 'trash'">
      <path d="M4 7h16" />
      <path d="M9.5 7V5.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V7" />
      <path d="m6.5 7 1 12a2 2 0 0 0 2 2h5a2 2 0 0 0 2-2l1-12" />
      <path d="M10 11v6M14 11v6" />
    </g>
    <!-- 感叹号（圆圈 + 一竖一点）：依赖图里"装着了但版本合不上"那枚标记 -->
    <g v-else-if="name === 'alert'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7.5v5.5" />
      <path d="M12 16.6h.01" />
    </g>
    <!-- 放大镜：搜索框左端那枚 -->
    <g v-else-if="name === 'search'">
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </g>
    <path v-else-if="name === 'chevron-left'" d="m15 6-6 6 6 6" />
    <path v-else-if="name === 'chevron-right'" d="m9 6 6 6-6 6" />
    <path v-else-if="name === 'chevron-down'" d="m6 9 6 6 6-6" />
    <!-- 顺序调整：带杆的箭头，与只有折线的 chevron 区分开 -->
    <g v-else-if="name === 'arrow-up'">
      <path d="M12 19V5" />
      <path d="m6 11 6-6 6 6" />
    </g>
    <g v-else-if="name === 'arrow-down'">
      <path d="M12 5v14" />
      <path d="m6 13 6 6 6-6" />
    </g>
    <path v-else-if="name === 'play'" d="M7.5 5.2v13.6L19 12 7.5 5.2z" />
    <!-- 手柄：左右握把 + 十字键 + 两个按键；与其它图标同一套 24 视窗描边 -->
    <g v-else-if="name === 'gamepad'">
      <path d="M7 8h10a4 4 0 0 1 3.9 3.1l1 4.6a2.6 2.6 0 0 1-4.6 2.2L16 16H8l-1.3 1.9A2.6 2.6 0 0 1 2.1 15.7l1-4.6A4 4 0 0 1 7 8z" />
      <path d="M7.5 11v3M6 12.5h3" />
      <path d="M16 11.5h.01M17.5 13.5h.01" />
    </g>
    <!-- 暂停 / 停止：自定义音乐那两枚 -->
    <path v-else-if="name === 'pause'" d="M9.5 5v14M14.5 5v14" />
    <rect v-else-if="name === 'stop'" x="6.5" y="6.5" width="11" height="11" rx="1.6" />
    <!-- 电源：启动按钮运行中悬停的「关闭游戏」 -->
    <g v-else-if="name === 'power'">
      <path d="M12 3v9" />
      <path d="M18.36 6.64a9 9 0 1 1-12.72 0" />
    </g>
    <!-- 齿轮与顶栏功能按钮上那个是同一个画法，见 MainTopbar 的 gear -->
    <g v-else-if="name === 'gear'">
      <circle cx="12" cy="12" r="3.2" />
      <path
        d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.03-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.03 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.01a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.01a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03z" />
    </g>
    <!-- 文档：纸张轮廓与 FileTree 的文件图标同款，另加两行文字 -->
    <g v-else-if="name === 'document'">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z" />
      <path d="M14 2v6h6" />
      <path d="M8 13h8M8 17h5" />
    </g>
    <!-- 终端：>_ 提示符 + 一行输出 -->
    <g v-else-if="name === 'terminal'">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="m7 9 3 3-3 3" />
      <path d="M12 15h5" />
    </g>
    <!-- 扳手：调校 / 修复 -->
    <g v-else-if="name === 'wrench'">
      <path
        d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    </g>
    <!-- 日志窗口：带两行文字的窗口。行数不能再多 —— 15px 下三行会糊成一块 -->
    <g v-else-if="name === 'log'">
      <rect x="4" y="3.5" width="16" height="17" rx="2" />
      <path d="M8 9h8M8 14.5h5" />
    </g>
    <!-- 配置文件修改：两根滑杆，行距按图标满高给到 12 单位 —— 只留两根是为了 15px 下不糊，
         行距小了在 32px 的按钮里又扁又小 -->
    <g v-else-if="name === 'sliders'">
      <path d="M4 6h8M16 6h4" />
      <path d="M4 18h4M12 18h8" />
      <circle cx="14" cy="6" r="2" />
      <circle cx="10" cy="18" r="2" />
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
    <!-- 更新记录：时钟 + 回绕箭头，读作"历史"，与"刷新"那种纯圈箭头区分开 -->
    <g v-else-if="name === 'history'">
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M12 7v5l4 2" />
    </g>
    <!-- 依赖检测：拼图块（前置 / 依赖），与"确认"那类 check 区分开。
         transform 把带凸起的图形摆回 24 视窗正中 -->
    <g v-else-if="name === 'puzzle'" transform="translate(-1.25 .25)">
      <path
        d="M5 8a2 2 0 0 1 2-2h2.5a2.5 2.5 0 0 1 5 0H17a2 2 0 0 1 2 2v2.5a2.5 2.5 0 0 1 0 5V18a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V8z" />
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
    <!-- 导出：托盘 + 向上的箭头，与「下载」上下对称 -->
    <g v-else-if="name === 'export'">
      <path d="M12 15V3" />
      <path d="m7 8 5-5 5 5" />
      <path d="M4 20h16" />
    </g>
    <!-- 升级：向上的箭头（版本往上走），与"下载"那条向下的箭头区分开 -->
    <g v-else-if="name === 'update'">
      <path d="M12 20V5" />
      <path d="m6 11 6-6 6 6" />
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
    <!-- 存档：带盖的箱体 + 中间一道横线（游戏存档） -->
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
    <!-- 实验性功能：锥形瓶（试管 + 液面，实验品的意思） -->
    <g v-else-if="name === 'flask'">
      <path d="M10 3h4" />
      <path d="M10.8 3v5.4L5.6 17a2 2 0 0 0 1.7 3h9.4a2 2 0 0 0 1.7-3l-5.2-8.6V3" />
      <path d="M8.3 14.2h7.4" />
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
    <!-- 人像：皮肤与头像 -->
    <g v-else-if="name === 'user'">
      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </g>
    <!-- 咖啡杯：Java -->
    <g v-else-if="name === 'coffee'">
      <path d="M17 8h1a4 4 0 1 1 0 8h-1" />
      <path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z" />
      <path d="M6 2v2M10 2v2M14 2v2" />
    </g>
    <!-- 四角括号朝外：向右占满 -->
    <g v-else-if="name === 'maximize'">
      <path d="M4 9V4h5" />
      <path d="M20 9V4h-5" />
      <path d="M4 15v5h5" />
      <path d="M20 15v5h-5" />
    </g>
    <!-- 同一组括号朝内：收回 -->
    <g v-else-if="name === 'restore'">
      <path d="M9 4v5H4" />
      <path d="M15 4v5h5" />
      <path d="M9 20v-5H4" />
      <path d="M15 20v-5h5" />
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
