<script setup lang="ts">
// 设置窗口 · 本地文件标签
//
// 列出运行目录下各文件夹的占用：缓存、下载内容与界面数据可清理，游戏实例和 Java 只统计。
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

/** 展开了明细的行 id；游戏实例与下载内容两行带 children */
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
 * 展开 / 收起某一行的明细
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
    <div class="local-files-head flex items-center justify-between gap-3">
      <span class="local-files-total text-body text-dim" v-tip="t('winSettings.localFilesLinkHint')">
        {{ t("winSettings.localFilesTotal") }}
        <b class="text-fg [font-variant-numeric:tabular-nums]" :class="{ 'is-scanning': loading }">
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
    <div v-if="loading && entries.length === 0" class="local-files-list flex flex-col gap-2">
      <div v-for="n in 6" :key="n" class="local-file-row flex items-center gap-3 py-2.5 px-3 bg-raised border border-border rounded-card">
        <div class="local-file-main flex-1 min-w-0 flex flex-col gap-[2px]">
          <span class="sk sk-name h-[17px] w-[42%]" />
          <span class="sk sk-path h-[14px] w-[62%]" />
        </div>
        <span class="sk sk-size shrink-0 w-[88px] h-3" />
        <div class="local-file-actions shrink-0 flex items-center gap-2">
          <span class="sk sk-btn" />
          <span class="sk sk-btn" />
        </div>
      </div>
    </div>
    <!-- 重新统计：旧数据压暗不可点，避免拿着过期结果去清理。
         清理流程里不压暗 —— 那一行自己有转圈，压暗只会把它一起糊掉 -->
    <div v-else class="local-files-list flex flex-col gap-2" :class="{ 'is-loading-dim': loading && !clearing }">
      <div v-for="e in entries" :key="e.id" class="local-file-block flex flex-col">
        <div class="local-file-row flex items-center gap-3 py-2.5 px-3 bg-raised border border-border rounded-card">
          <div class="local-file-main flex-1 min-w-0 flex flex-col gap-[2px]">
            <span class="local-file-title flex items-center gap-[6px] min-w-0">
              <!-- 游戏实例与下载内容带明细，可展开查看各子项占用 -->
              <button v-if="e.children.length"
                class="local-file-expand inline-flex items-center justify-center w-6 h-6 shrink-0 p-0 border-0 rounded-[6px] bg-transparent text-dim cursor-pointer transition-[transform,color,background-color] duration-150 hover:text-accent hover:bg-hover"
                :class="{ flip: isExpanded(e.id) }"
                :aria-expanded="isExpanded(e.id)"
                v-tip="isExpanded(e.id) ? t('winSettings.localFileChildrenCollapse') : t('winSettings.localFileChildrenExpand')"
                :aria-label="isExpanded(e.id) ? t('winSettings.localFileChildrenCollapse') : t('winSettings.localFileChildrenExpand')"
                @click="toggle(e.id)">
                <GlyphIcon name="chevron-down" :size="14" :weight="2.4" />
              </button>
              <span class="local-file-name text-title font-semibold">{{ t(`winSettings.localFile.${e.id}`) }}</span>
            </span>
            <span class="local-file-path text-[11px] text-dim [font-family:Cascadia_Code,Consolas,monospace] overflow-hidden text-ellipsis whitespace-nowrap" :title="e.path">{{ e.path }}</span>
          </div>
          <!-- 清理中：数字让位给转圈，删完且重新统计完才换回新数字 -->
          <span class="local-file-size shrink-0 flex items-center justify-end w-[88px] text-right text-body [font-variant-numeric:tabular-nums]">
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
        <VirtualList v-if="e.children.length && isExpanded(e.id)"
          class="local-file-children flex flex-col max-h-60 mt-[6px] mr-3 ml-[42px] border border-border rounded-card bg-page overflow-y-auto overscroll-contain"
          :count="e.children.length"
          :row-h="CHILD_ROW_H" :row-gap="0">
          <template #default="{ start, end }">
            <div v-for="child in e.children.slice(start, end)" :key="child.id"
              class="local-file-child flex items-center gap-3 h-[34px] shrink-0 px-2.5 border-b border-border text-body hover:bg-hover">
              <span class="local-file-child-name flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap" :title="child.path">{{ childName(child) }}</span>
              <span class="local-file-size shrink-0 flex items-center justify-end w-[88px] text-right text-body [font-variant-numeric:tabular-nums]">{{ formatSize(child.size) }}</span>
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
/* 排版都在模板的工具类上；这里只留展开键的翻转状态（绑定在 :class 上） */
.local-file-expand.flip {
  transform: rotate(180deg);
}
</style>
