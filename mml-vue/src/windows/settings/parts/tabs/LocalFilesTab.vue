<script setup lang="ts">
// 设置窗口 · 本地文件标签
//
// 列出运行目录下各文件夹的占用：缓存类可以就地清理，界面数据与游戏实例只统计不删。
// 统计只在第一次打开本标签时做，之后靠「重新统计」刷新。
import { computed, onMounted, ref } from "vue";
import { t } from "../../../../lib/i18n";
import { formatSize } from "../../../../lib/format";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import VirtualList from "../../../../components/ui/VirtualList.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { LocalFileChildDto } from "../../../../lib/bindings";
import type { useSettingsLocalFiles } from "../../composables/useSettingsLocalFiles";

const props = defineProps<{
  /** 本地文件标签的状态与操作，来自 useSettingsLocalFiles */
  files: ReturnType<typeof useSettingsLocalFiles>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const { entries, loading, downloading, clearing, total, load, clear, open } = props.files;

/** 等待确认清理的项 id；空串表示没有弹窗 */
const pending = ref("");

/** 展开了明细的行 id；目前只有「游戏实例」这一行带 children */
const expanded = ref<Set<string>>(new Set());

/** 明细行高，单位 px；与 .local-file-child 的 height 一致 */
const CHILD_ROW_H = 34;

/** 确认框里要显示的名字，取当前语言的文案 */
const pendingName = computed(() => {
  const entry = entries.value.find((e) => e.id === pending.value);
  return entry ? t(`winSettings.localFile.${entry.id}`) : "";
});

/** 某一行的明细是否展开 */
function isExpanded(id: string) {
  return expanded.value.has(id);
}

/**
 * 明细行的显示名
 *
 * 实例用实例名；下载内容下的 mods / resourcepacks 按类型翻译。
 *
 * - `child`: 明细行
 *
 * # 返回值
 *
 * 已翻译的显示名
 */
function childName(child: LocalFileChildDto): string {
  return child.kind === "instance"
    ? child.name
    : t(`winSettings.localFile.${child.kind}`);
}

/**
 * 展开 / 收起某一行的实例明细
 *
 * - `id`: 行的 id，取自 `entries`
 */
function toggle(id: string) {
  const next = new Set(expanded.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expanded.value = next;
}

// 统计一次就够：切走再回来沿用上次结果，要最新的点「重新统计」
onMounted(() => {
  if (entries.value.length === 0) {
    void load();
  }
});

/** 确认清理：先关弹窗，再清那一项并等它重新统计 */
async function onConfirmClear() {
  const id = pending.value;
  pending.value = "";
  if (id) {
    await clear(id);
  }
}
</script>

<template>
  <SettingsGroup id="localFiles" title-key="winSettings.secLocalFiles" :flash="flashGroup === 'localFiles'">
    <div class="local-files-head">
      <span class="local-files-total" v-tip="t('winSettings.localFilesLinkHint')">
        {{ t("winSettings.localFilesTotal") }}
        <b :class="{ 'is-scanning': loading }">
          {{ loading && entries.length === 0 ? t("winSettings.localFilesScanning") : formatSize(total) }}
        </b>
      </span>
      <BaseButton size="sm" :disabled="loading" @click="load">
        <GlyphIcon name="refresh" :size="13" :class="{ 'is-spinning': loading }" />
        {{ t("winSettings.localFilesRefresh") }}
      </BaseButton>
    </div>

    <!-- 首次统计：骨架行占位，形状贴着真行，数完直接换内容不跳版。
         行数固定为后端那六项，高度一致，换内容时整页不会上下跳 -->
    <div v-if="loading && entries.length === 0" class="local-files-list">
      <div v-for="n in 6" :key="n" class="local-file-row">
        <div class="local-file-main">
          <span class="sk sk-name" />
          <span class="sk sk-path" />
        </div>
        <span class="sk sk-size" />
        <div class="local-file-actions">
          <span class="sk sk-btn" />
          <span class="sk sk-btn" />
        </div>
      </div>
    </div>
    <!-- 重新统计：旧数据压暗不可点，避免拿着过期结果去清理。
         清理流程里不压暗 —— 那一行自己有转圈，压暗只会把它一起糊掉 -->
    <div v-else class="local-files-list" :class="{ 'is-loading-dim': loading && !clearing }">
      <div v-for="e in entries" :key="e.id" class="local-file-block">
        <div class="local-file-row">
          <div class="local-file-main">
            <span class="local-file-title">
              <!-- 游戏实例与下载内容带明细，可展开查看各子项占用 -->
              <button v-if="e.children.length" class="local-file-expand" :class="{ flip: isExpanded(e.id) }"
                :aria-expanded="isExpanded(e.id)"
                v-tip="isExpanded(e.id) ? t('winSettings.localFileChildrenCollapse') : t('winSettings.localFileChildrenExpand')"
                :aria-label="isExpanded(e.id) ? t('winSettings.localFileChildrenCollapse') : t('winSettings.localFileChildrenExpand')"
                @click="toggle(e.id)">
                <GlyphIcon name="chevron-down" :size="14" :weight="2.4" />
              </button>
              <span class="local-file-name">{{ t(`winSettings.localFile.${e.id}`) }}</span>
            </span>
            <span class="local-file-path" :title="e.path">{{ e.path }}</span>
          </div>
          <!-- 清理中：数字让位给转圈，删完且重新统计完才换回新数字 -->
          <span class="local-file-size">
            <span v-if="clearing === e.id" class="loading-spin sm" />
            <template v-else>{{ formatSize(e.size) }}</template>
          </span>
          <div class="local-file-actions">
            <BaseButton size="sm" @click="open(e.id)">{{ t("winSettings.localFileOpen") }}</BaseButton>
            <BaseButton v-if="e.deletable" size="sm" variant="danger"
              :disabled="!!clearing || loading || downloading || e.size === 0"
              :title="downloading ? t('winSettings.localFileClearBlocked') : undefined" @click="pending = e.id">
              {{ t("winSettings.localFileClear") }}
            </BaseButton>
          </div>
        </div>

        <!-- 每条明细：名字贴左、大小靠右；实例多时只渲染视口附近的行 -->
        <VirtualList v-if="e.children.length && isExpanded(e.id)" class="local-file-children" :count="e.children.length"
          :row-h="CHILD_ROW_H" :row-gap="0">
          <template #default="{ start, end }">
            <div v-for="child in e.children.slice(start, end)" :key="child.id" class="local-file-child">
              <span class="local-file-child-name" :title="child.path">{{ childName(child) }}</span>
              <span class="local-file-size">{{ formatSize(child.size) }}</span>
            </div>
          </template>
        </VirtualList>
      </div>
    </div>
  </SettingsGroup>

  <BaseModal v-if="pending" :title="t('winSettings.localFileClearTitle')" @close="pending = ''">
    <p class="modal-text">
      {{ t("winSettings.localFileClearConfirm", { name: pendingName }) }}
    </p>
    <div class="modal-actions">
      <BaseButton @click="pending = ''">{{ t("actions.cancel") }}</BaseButton>
      <BaseButton variant="danger" :disabled="downloading" @click="onConfirmClear">
        {{ t("winSettings.localFileClear") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 展开后的实例明细与展开键：只属于这一屏，跟组件走 */
.local-file-block {
  display: flex;
  flex-direction: column;
}

.local-file-title {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

/* 24px 的可点区域，但不画外框；悬停只给一点底色 */
.local-file-expand {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: transform 0.15s ease, color 0.12s ease, background 0.12s ease;
}

.local-file-expand:hover {
  color: var(--accent);
  background: var(--bg-hover);
}

.local-file-expand.flip {
  transform: rotate(180deg);
}

/* 明细是一条条的行：外框只负责分组，行与行之间用分隔线；超过上限自己滚 */
.local-file-children {
  display: flex;
  flex-direction: column;
  max-height: 240px;
  margin: 6px 12px 0 42px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
  overflow-y: auto;
  overscroll-behavior: contain;
}

.local-file-child {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 34px;
  flex-shrink: 0;
  padding: 0 10px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
}

.local-file-child:hover {
  background: var(--bg-hover);
}

.local-file-child-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
