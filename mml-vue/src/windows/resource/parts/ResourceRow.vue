<script setup lang="ts">
// 资源列表的通用行：图标（有才显示）+ 名称 + 徽标 + 副标题 + 右侧操作
//
// 七个分类的行长得一模一样，差别只在徽标、副标题与操作按钮上，所以那三处走插槽；
// 行的样式在窗口级 resource.css 里，非 scoped，拆开的子组件共用，见那里的头部说明。
//
// 另有「展开箭头」插槽与缩进：模组列表要把 jar-in-jar 的内置模组**缩进**显示在同一列里
// （见 ModList），层级与列表合在一处；其它分类不用这两个口子，不传就是原来的样子。
//
// 插槽：`select` 最左侧的多选勾选框；`lead` 图标之前的展开箭头；`name-lead` 名字之前；
// 默认插槽是名字本身，需要过 `§` 格式码时由调用方换成 FormattedText；`badges` 名字之后；
// `sub` 副标题；`actions` 右侧操作按钮。
withDefaults(
  defineProps<{
    /** 图标地址；**空 / null 时整块不渲染**（不留占位框，见模板注释） */
    icon?: string | null;
    /** 显示名；调用方给了 name 插槽时以插槽为准 */
    name?: string;
    /**
     * 缩进层级（0 = 顶层）
     *
     * 每一级缩进 22px，与原来树形视图的缩进步长一致，见 `resource.css` 的 `--row-indent`。
     */
    depth?: number;
  }>(),
  { icon: "", name: "", depth: 0 },
);
</script>

<template>
  <div class="item-row" :class="{ 'mod-nested': depth > 0 }" :style="{ '--row-depth': depth }">
    <!--
      多选勾选框，模组列表用，见 ModList：在**最左边**，箭头与图标之前。
      用户要求"右键用左边加勾选" —— 右键进入多选后，左边出现勾选框继续加选。
      其它分类不传这个插槽，什么都不渲染
    -->
    <slot name="select" />
    <!-- 展开箭头（有下级的行才给）：放在图标之前 -->
    <slot name="lead" />
    <!--
      图标：**没有就什么都不画**
      以前这里会补一个占位，先是 M / P / S 首字母，后来换成"无图"图标，
      但两种都是在给一个本来就不存在的东西占地方 —— 列表里大片灰框/灰图标反而更花。
      没有图标就让名字直接靠左，版面更干净。图标列的对齐由 `.item-name` 自己保证，
      不依赖这一格存在，见 resource.css 里的说明。
    -->
    <img v-if="icon" class="item-icon" :src="icon" alt="" />
    <div class="item-main">
      <div class="item-name-line">
        <!-- 名字**之前**的位置：模组列表把备注放这儿，用户要求"备注放在名字前面"。
             单独开一个插槽而不是复用 badges：badges 在名字**之后**，
             读起来是"名字 + 一串标记"，与"先看到备注"的诉求相反 -->
        <slot name="name-lead" />
        <!-- 名字默认按纯文本渲染；需要过 `§` 格式码的（存档名）走 name 插槽，
             换 FormattedText 铺进来 —— 与 sub 插槽同一套做法 -->
        <span class="item-name">
          <slot name="name">{{ name }}</slot>
        </span>
        <slot name="badges" />
      </div>
      <span class="item-sub">
        <slot name="sub" />
      </span>
    </div>
    <div class="item-actions">
      <slot name="actions" />
    </div>
  </div>
</template>
