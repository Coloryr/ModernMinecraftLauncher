<script setup lang="ts">
// 方块列表 · 详情弹窗：大图预览 + ID（可复制）/ 分类 + 设为实例图标
//
// `BaseModal` 不处理 Esc，且内容被 Teleport 到 body，所以键盘在这里自己接管（只有 Esc）。
import { computed, ref } from "vue";
import { t } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { copyText } from "../../../lib/clipboard";
import { useModalKeys } from "../../../composables/useModalKeys";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import AsyncImage from "../../../components/ui/AsyncImage.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import type { BlockItemDto } from "../../../lib/bindings";
import type { BlockGroup } from "../composables/useBlockGroups";

const props = defineProps<{
  /** 要展示的方块条目 */
  block: BlockItemDto;
  /** 分类显示名（父级已翻译） */
  category: string;
  /** 搜索词：详情里同样高亮，方便看清为什么命中 */
  keyword: string;
  /** 玩家皮肤（可删除） */
  removable: boolean;
  /** 全部自建分组 */
  groups: BlockGroup[];
  /** 这个方块所属的分组 uuid；未分组时为 null */
  group: string | null;
  /** 正在设图标（父级控制，禁用按钮防连点） */
  busy?: boolean;
}>();

/**
 * 弹窗自己的预览方式
 *
 * 只换这张大图，不动列表；每次打开按 3D 起步。没有 2D 贴图的条目（物品、头颅）
 * 不给切，见模板里的 v-if
 */
const previewView = ref("3d");

/**
 * 一个分组里都有哪些面，拼成选项文案
 *
 * 上下共用同一张贴图时后端已经并成一组，这里显示成「上下」；
 * 其它组合按面名逐个翻，如「顶面/侧面」。
 *
 * - `faces`: 逗号分隔的面名，如 `top,bottom`
 *
 * # 返回值
 *
 * 已翻译好的选项文案
 */
function faceLabel(faces: string): string {
  const parts = faces.split(",");
  if (parts.length === 2 && parts.includes("top") && parts.includes("bottom")) {
    return t("blocks.faceTopBottom");
  }
  return parts.map((part) => t(`blocks.face.${part}`)).join("/");
}

/** 这个方块有哪些 2D 面可选：后端已按贴图分组，上下同图只算一组 */
const faceOptions = computed(() =>
  props.block.faces2d.map((face) => ({
    value: face.faces,
    label: faceLabel(face.faces),
    url: face.image,
  })),
);

/** 有没有 2D 贴图：物品、头颅没有，整组切换都不出现 */
const has2d = computed(() => faceOptions.value.length > 0);

/** 当前在看哪一组；默认带侧面的那组，没有侧面的取第一组 */
const face = ref(
  faceOptions.value.find((each) => each.value.split(",").includes("side"))?.value ??
    faceOptions.value[0]?.value ??
    "",
);

/** 当前选中的分组；找不到时回落到第一组 */
const currentFace = computed(
  () => faceOptions.value.find((each) => each.value === face.value) ?? faceOptions.value[0] ?? null,
);

/**
 * 当前该显示的图标地址
 *
 * # 返回值
 *
 * 3D 模式给渲染图；2D 模式给所选面的平面贴图，该面缺失时回退 3D
 */
function imgSrc(): string {
  if (previewView.value === "2d") {
    const picked = currentFace.value;
    if (picked) return picked.url;
  }
  return props.block.image;
}

const emit = defineEmits<{
  /** 请求关闭弹窗 */
  (e: "close"): void;
  /** 请求删除该玩家皮肤，仅在 `removable` 为真时可用 */
  (e: "remove"): void;
  /**
   * 请求把它设为某个实例的图标，父级接着打开选实例弹窗
   *
   * - `face`: 当前预览的 2D 面，空串表示 3D
   */
  (e: "set-icon", face: string): void;
  /** 换分组：uuid 为 null 表示未分组 */
  (e: "set-group", uuid: string | null): void;
}>();

useModalKeys((e) => {
  if (e.key === "Escape") emit("close");
});

/**
 * 设为实例图标时该记哪个 2D 面
 *
 * 一组面共用一个文件，取组内第一个面名即可；3D 预览返回空串。
 *
 * # 返回值
 *
 * 面名，如 `top`；3D 时是空串
 */
function iconFace(): string {
  if (previewView.value !== "2d") return "";
  return currentFace.value?.value.split(",")[0] ?? "";
}

/** 复制方块 id，成败都给一句提示 */
async function copyId() {
  showToast((await copyText(props.block.id)) ? t("blocks.copied") : t("blocks.copyFail"));
}
</script>

<template>
  <BaseModal :title="block.name" :width="500" :closable="false" @close="emit('close')">
    <div class="detail-body">
      <div class="detail-stage-col">
        <div class="detail-stage">
          <AsyncImage class="detail-img" :src="imgSrc()" :alt="block.name" />
        </div>
        <!-- 预览贴图：3D 等轴测 / 2D 平面，只换这张大图。物品、头颅这类没有 2D 贴图的不给切 -->
        <SegmentedTabs v-if="has2d" :model-value="previewView" :options="[
          { value: '3d', label: t('blocks.view3d') },
          { value: '2d', label: t('blocks.view2d') },
        ]" @update:model-value="previewView = $event" />
        <!-- 2D 下再选看哪个面；只有一个面时不出现 -->
        <SegmentedTabs v-if="previewView === '2d' && faceOptions.length > 1" :model-value="face"
          :options="faceOptions.map((each) => ({ value: each.value, label: each.label }))"
          @update:model-value="face = $event" />
      </div>
      <!-- 右列只在预览图那块高度里居中：有没有 3D / 2D 切换都停在同一个位置 -->
      <div class="detail-info-slot">
        <div class="detail-info">
          <div class="detail-row">
            <span class="field-label">{{ t("blocks.detailId") }}</span>
            <div class="detail-id">
              <code class="detail-code"><HighlightText :text="block.id" :query="keyword" /></code>
              <BaseButton class="detail-copy" size="sm" variant="ghost" v-tip="t('blocks.copyId')"
                @click="copyId">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round">
                  <rect x="9" y="9" width="12" height="12" rx="2" />
                  <path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1" />
                </svg>
              </BaseButton>
            </div>
          </div>
          <div class="detail-row">
            <span class="field-label">{{ t("blocks.detailCat") }}</span>
            <span class="cat-chip">{{ category }}</span>
          </div>
          <div class="detail-row">
            <span class="field-label">{{ t("blocks.detailGroups") }}</span>
            <select class="field-select" :value="group ?? ''"
              @change="emit('set-group', ($event.target as HTMLSelectElement).value || null)">
              <option value="">{{ t("blocks.groupNone") }}</option>
              <option v-for="g in groups" :key="g.uuid" :value="g.uuid">{{ g.name }}</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <div class="modal-actions">
      <!-- 删除单独靠左：它是危险操作，不能和「关闭 / 设为实例图标」挤在一起 -->
      <BaseButton v-if="removable" class="detail-remove" variant="danger" @click="emit('remove')">
        {{ t("blocks.remove") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="busy" @click="emit('set-icon', iconFace())">
        {{ t("blocks.setIcon") }}
      </BaseButton>
      <BaseButton @click="emit('close')">{{ t("blocks.close") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
.detail-body {
  display: flex;
  gap: 16px;
  align-items: flex-start;
  /* 预览图边长，右列的居中高度与它对齐用同一个值 */
  --detail-stage: 160px;
}

/* 预览图 + 3D/2D 切换：切换贴着图放，换完立刻能看出差别 */
.detail-stage-col {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.detail-stage {
  flex-shrink: 0;
  width: var(--detail-stage);
  height: var(--detail-stage);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-side);
}

.detail-img {
  width: 132px;
  height: 132px;
  /* 同 BlockGrid 的 .block-img：256×256 的渲染图缩到 132px，用平滑插值，
     不要 pixelated，最近邻会把方块斜边采成锯齿 */
}

/* 右列的居中容器：高度锚在预览图上，下面有没有 3D / 2D 切换都不改变表单的位置 */
.detail-info-slot {
  flex: 1;
  min-width: 0;
  min-height: var(--detail-stage);
  display: flex;
  align-items: center;
}

.detail-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 表单行：标签定宽占左列，值占剩下的宽度 —— 三行的控件左缘才对得齐 */
.detail-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.detail-row .field-label {
  margin: 0;
  flex: none;
  width: 56px;
  white-space: nowrap;
}

/* 下拉吃掉剩下的宽度：全局 .field-select 是 width: 100%，在 flex 行里要显式让它可以收缩 */
.detail-row .field-select {
  flex: 1;
  min-width: 0;
}

/* ID 行：代码块 + 复制按钮 */
.detail-id {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.detail-code {
  flex: 1;
  min-width: 0;
  /* 与同一行的下拉 / 按钮同高；ID 太长换行时跟着长高，不裁字 */
  min-height: var(--ctl-h);
  display: flex;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-side);
  font-family: ui-monospace, Consolas, "Courier New", monospace;
  font-size: 11.5px;
  color: var(--text);
  word-break: break-all;
}

/* 复制键：跟框同一档高度，框变长时不跟着拉宽 */
.detail-copy {
  flex: none;
}

/* 删除靠左：`.modal-actions` 是右对齐的一行，这里把后面的按钮整体推回右边 */
.detail-remove {
  margin-right: auto;
}

.cat-chip {
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11.5px;
  font-weight: 600;
}
</style>
