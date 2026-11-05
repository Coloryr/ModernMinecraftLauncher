<script setup lang="ts">
import { provide } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import InstanceSelect from "../../components/InstanceSelect.vue";
import { t } from "../../lib/i18n";
import GeneralPanel from "./parts/GeneralPanel.vue";
import BasePanel from "./parts/BasePanel.vue";
import ModsPanel from "./parts/ModsPanel.vue";
import PacksPanel from "./parts/PacksPanel.vue";
import ConfigsPanel from "./parts/ConfigsPanel.vue";
import StartPanel from "./parts/StartPanel.vue";
import { useServerInstance } from "./useServerInstance";
import { ServerInstanceKey } from "./context";

/** 关闭窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

const instance = useServerInstance();
provide(ServerInstanceKey, instance);

const { instances, currentUuid, busy, section, sections, pickInstance, openGuide } = instance;
</script>
<template>
  <WindowFrame :title="t('winTitle.serverInstance')" body-fill body-flush @close="emit('close')">
    <!-- 实例放标题栏：换实例不用回到总体设定 -->
    <template #head-right>
      <div class="si-head-actions">
        <BaseButton variant="ghost" size="sm" class="icon-btn-head"
          v-tip="t('serverInstance.guide')" :aria-label="t('serverInstance.guide')" @click="openGuide">
          <GlyphIcon name="document" :size="15" :weight="1.8" />
        </BaseButton>
        <InstanceSelect class="head-select" :instances="instances" :model-value="currentUuid"
          :placeholder="t('winExport.instance')" :disabled="busy" :menu-min-width="260"
          @update:model-value="pickInstance" />
      </div>
    </template>

    <div class="si-layout">
      <!-- 左侧分页 -->
      <nav class="si-rail">
        <button v-for="item in sections" :key="item.id" class="si-rail-item" :class="{ active: section === item.id }"
          :disabled="busy && item.id === 'start'" @click="section = item.id">
          <span class="si-rail-icon">
            <GlyphIcon :name="item.icon" :size="16" :weight="1.8" />
          </span>
          <span class="si-rail-text">
            <span class="si-rail-title">{{ t(item.labelKey) }}</span>
            <span class="si-rail-desc">{{ t(`serverInstance.tabDesc.${item.id}`) }}</span>
          </span>
        </button>
      </nav>

      <div class="si-content">
        <!-- 当前页大标题 + 说明，与设置窗口同款 -->
        <header class="si-head">
          <h2 class="si-title">{{ t(`serverInstance.tab.${section}`) }}</h2>
          <p class="si-sub">{{ t(`serverInstance.tabDesc.${section}`) }}</p>
        </header>

        <p v-if="!instances.length" class="empty-tip">{{ t("serverInstance.noInstance") }}</p>

        <template v-else>
            <GeneralPanel v-if="section === 'general'" />
            <BasePanel v-else-if="section === 'base'" />
            <ModsPanel v-else-if="section === 'mods'" />
            <PacksPanel v-else-if="section === 'packs'" />
            <ConfigsPanel v-else-if="section === 'configs'" />
            <StartPanel v-else />
        </template>
      </div>
    </div>
  </WindowFrame>
</template>
<style src="./server-instance.css"></style>

<style scoped>
/* 实例下拉比标题栏标准的 170–280 宽一档：实例名长，要留够显示宽度 */
.head-select {
  min-width: 200px;
  max-width: 300px;
}

.si-head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
