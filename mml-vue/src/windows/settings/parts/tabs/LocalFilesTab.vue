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
import SettingsGroup from "../SettingsGroup.vue";
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

/** 确认框里要显示的名字，取当前语言的文案 */
const pendingName = computed(() => {
  const entry = entries.value.find((e) => e.id === pending.value);
  return entry ? t(`winSettings.localFile.${entry.id}`) : "";
});

// 统计一次就够：切走再回来沿用上次结果，要最新的点「重新统计」
onMounted(() => {
  if (entries.value.length === 0) {
    void load();
  }
});

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
      <span class="local-files-total">
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
      <div v-for="e in entries" :key="e.id" class="local-file-row">
        <div class="local-file-main">
          <span class="local-file-name">{{ t(`winSettings.localFile.${e.id}`) }}</span>
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
