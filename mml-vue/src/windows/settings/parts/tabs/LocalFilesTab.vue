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

const { entries, loading, clearing, total, load, clear, open } = props.files;

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
        <b>{{ loading && entries.length === 0 ? t("winSettings.localFilesScanning") : formatSize(total) }}</b>
      </span>
      <BaseButton size="sm" :disabled="loading" @click="load">
        <GlyphIcon name="refresh" :size="13" />
        {{ t("winSettings.localFilesRefresh") }}
      </BaseButton>
    </div>

    <p v-if="loading && entries.length === 0" class="field-desc">
      {{ t("winSettings.localFilesScanning") }}
    </p>
    <div v-else class="local-files-list">
      <div v-for="e in entries" :key="e.id" class="local-file-row">
        <div class="local-file-main">
          <span class="local-file-name">{{ t(`winSettings.localFile.${e.id}`) }}</span>
          <span class="local-file-path" :title="e.path">{{ e.path }}</span>
        </div>
        <span class="local-file-size">{{ formatSize(e.size) }}</span>
        <div class="local-file-actions">
          <BaseButton size="sm" @click="open(e.id)">{{ t("winSettings.localFileOpen") }}</BaseButton>
          <BaseButton v-if="e.deletable" size="sm" variant="danger"
            :disabled="!!clearing || loading || e.size === 0" @click="pending = e.id">
            {{ t("winSettings.localFileClear") }}
          </BaseButton>
        </div>
      </div>
    </div>

    <p class="field-desc">{{ t("winSettings.localFilesHint") }}</p>
  </SettingsGroup>

  <BaseModal v-if="pending" :title="t('winSettings.localFileClearTitle')" @close="pending = ''">
    <p class="modal-text">
      {{ t("winSettings.localFileClearConfirm", { name: pendingName }) }}
    </p>
    <div class="modal-actions">
      <BaseButton @click="pending = ''">{{ t("actions.cancel") }}</BaseButton>
      <BaseButton variant="danger" @click="onConfirmClear">
        {{ t("winSettings.localFileClear") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>
