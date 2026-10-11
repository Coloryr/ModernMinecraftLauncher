<script setup lang="ts">
// 设置窗口 · Java 标签：手动添加 / 扫描 / 扫描目录 / 压缩包导入 / 下载 / 列表
import { t } from "../../../../lib/i18n";
import { useModalKeys } from "../../../../composables/useModalKeys";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import CollapsePanel from "../../../../components/ui/CollapsePanel.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsJava } from "../../composables/useSettingsJava";

const props = defineProps<{
  /** Java 标签的状态与操作方法，来自 useSettingsJava */
  settings: ReturnType<typeof useSettingsJava>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();


const {
  inTauri,
  javaList,
  javaGroups,
  toggleType,
  typeOpen,
  scanning,
  scanningDir,
  newJavaName,
  newJavaPath,
  addingJava,
  browseJava,
  addJava,
  removeJava,
  removeAllJava,
  scanJava,
  scanJavaDir,
  importJavaArchive,
  importingJava,
  importProgress,
  importPercent,
  confirmRemoveAll,
  downloadJava,
} = props.settings;

// 删除确认框：Esc = 取消；扫描中的加载弹窗不可关闭，忽略
useModalKeys((e) => {
  if (e.key !== "Escape") return;
  if (confirmRemoveAll.value) confirmRemoveAll.value = false;
});
</script>

<template>
  <template v-if="inTauri">
    <SettingsGroup id="javaAdd" title-key="winSettings.javaAdd" :flash="flashGroup === 'javaAdd'">
      <!-- 手动添加：输入名字和路径 -->
      <div class="java-add-row">
        <input v-model="newJavaName" class="field-input java-add-name" :placeholder="t('winSettings.javaName')"
          spellcheck="false" autocomplete="off" />
        <input v-model="newJavaPath" class="field-input grow" :placeholder="t('winSettings.javaPath')"
          spellcheck="false" autocomplete="off" />
        <BaseButton size="sm" @click="browseJava">{{ t("args.browse") }}</BaseButton>
        <BaseButton size="sm" variant="accent" :disabled="addingJava || !newJavaName.trim() || !newJavaPath.trim()"
          @click="addJava">
          {{ t("winSettings.javaAdd") }}
        </BaseButton>
      </div>
      <div class="set-save-row java-add-actions">
        <BaseButton size="sm" :disabled="scanning || scanningDir" @click="scanJava">
          {{ scanning ? t("winSettings.javaScanning") : t("winSettings.javaScan") }}
        </BaseButton>
        <BaseButton size="sm" :disabled="scanning || scanningDir" @click="scanJavaDir">
          {{ t("winSettings.javaScanDir") }}
        </BaseButton>
        <BaseButton size="sm" @click="importJavaArchive">{{ t("winSettings.javaImport") }}</BaseButton>
        <BaseButton size="sm" @click="downloadJava">{{ t("winSettings.javaDownload") }}</BaseButton>
        <BaseButton size="sm" variant="danger" :disabled="javaList.length === 0" @click="confirmRemoveAll = true">
          {{ t("winSettings.javaRemoveAll") }}
        </BaseButton>
      </div>

      <!-- 压缩包导入进度条：解包 / 识别阶段经 settings-java-progress 事件推进 -->
      <div v-if="importingJava || importProgress" class="java-progress">
        <span class="java-progress-label">{{ t("winSettings.javaImporting") }}</span>
        <div class="java-progress-track">
          <div class="java-progress-fill" :style="{ width: importPercent + '%' }" />
        </div>
        <span class="java-progress-text">{{ importProgress ? `${importProgress.now}/${importProgress.total}` : ""
          }}</span>
        <span v-if="importProgress?.subText" class="java-progress-sub" v-tip="importProgress.subText">{{
          importProgress.subText }}</span>
      </div>

      <div v-if="javaList.length === 0" class="java-empty">{{ t("winSettings.javaNone") }}</div>
      <!-- 按发行类型（JDK / JRE）分组：类型行展开后，该类型的 Java 以卡片网格平铺 -->
      <div v-for="g in javaGroups" :key="g.type" class="java-type">
        <button class="java-type-head" @click="toggleType(g.type)">
          <GlyphIcon class="chev" :class="{ up: typeOpen(g.type) }" name="chevron-down" :size="11" :weight="2.4" />
          <span class="java-type-name">{{ g.type }}</span>
          <span class="java-count">{{ g.items.length }}</span>
        </button>
        <CollapsePanel :open="typeOpen(g.type)">
          <div class="java-grid">
            <div v-for="j in g.items" :key="j.name" class="java-card">
              <div class="java-card-top">
                <span class="java-name" v-tip="j.name">{{ j.name }}</span>
                <BaseButton size="sm" variant="danger" @click="removeJava(j.name)">
                  {{ t("winSettings.javaRemove") }}
                </BaseButton>
              </div>
              <span class="java-meta">{{ j.version }} · {{ j.arch }}</span>
              <span class="java-path" v-tip="j.path">{{ j.path }}</span>
            </div>
          </div>
        </CollapsePanel>
      </div>
    </SettingsGroup>

    <!-- 删除全部确认 -->
    <BaseModal v-if="confirmRemoveAll" :title="t('winSettings.javaRemoveAll')" :overlay-close="false" below-titlebar
      :closable="false" @close="confirmRemoveAll = false">
      <p class="field-desc">{{ t("winSettings.javaRemoveAllConfirm") }}</p>
      <div class="set-save-row">
        <BaseButton size="sm" @click="confirmRemoveAll = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton size="sm" variant="danger" @click="removeAllJava">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 扫描 Java 加载弹窗：扫描期间显示，完成自动消失 -->
    <BaseModal v-if="scanning || scanningDir" :width="300" :closable="false" :overlay-close="false">
      <div class="loading-modal">
        <span class="loading-spin" />
        <span>{{ t("winSettings.javaScanning") }}</span>
      </div>
    </BaseModal>
  </template>
  <p v-else class="field-desc">{{ t("winSettings.tauriOnly") }}</p>
</template>
