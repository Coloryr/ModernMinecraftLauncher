<script setup lang="ts">
import { computed, provide, ref } from "vue";
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
import { useSlidingThumb } from "../../composables/useSlidingThumb";
import { useStaggerEnter } from "../../composables/useStaggerEnter";
import { ServerInstanceKey } from "./context";

/** 关闭窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

const instance = useServerInstance();
provide(ServerInstanceKey, instance);

const { instances, currentUuid, busy, loading, section, sections, pickInstance, openGuide } = instance;

/** 左侧分页容器与选中项色块 */
const railEl = ref<HTMLElement | null>(null);
const activeIndex = computed(() => sections.value.findIndex((item) => item.id === section.value));
const { thumbStyle, thumbReady } = useSlidingThumb(
  railEl,
  ".si-rail-item",
  () => activeIndex.value,
);
/** 右侧分页：标题只渐变、不位移；面板里的字段按顺序落下 */
const { enter: enterPage, leave: leavePage } = useStaggerEnter(".si-section > *", {
  fade: ".si-head",
});
</script>
<template>
  <WindowFrame enter=":scope > *" :title="t('winTitle.serverInstance')" body-fill body-flush @close="emit('close')">
    <!-- 实例放标题栏：换实例不用回到总体设定 -->
    <template #head-right>
      <div class="si-head-actions">
        <BaseButton variant="ghost" size="sm" class="icon-btn-head" :disabled="loading"
          v-tip="t('serverInstance.guide')" :aria-label="t('serverInstance.guide')" @click="openGuide">
          <GlyphIcon name="document" :size="15" :weight="1.8" />
        </BaseButton>
        <InstanceSelect class="head-select" :instances="instances" :model-value="currentUuid"
          :placeholder="t('winExport.instance')" :disabled="busy || loading" :menu-min-width="260"
          @update:model-value="pickInstance" />
      </div>
    </template>

    <div class="si-layout">
      <!-- 左侧分页 -->
      <nav ref="railEl" class="si-rail" :class="{ ready: thumbReady }">
        <span class="si-thumb" :style="thumbStyle" aria-hidden="true"></span>
        <button v-for="item in sections" :key="item.id" class="si-rail-item" :class="{ active: section === item.id }"
          :disabled="loading || (busy && item.id === 'start')" @click="section = item.id">
          <span class="si-rail-icon">
            <GlyphIcon :name="item.icon" :size="16" :weight="1.8" />
          </span>
          <span class="si-rail-text">
            <span class="si-rail-title">{{ t(item.labelKey) }}</span>
            <span class="si-rail-desc">{{ t(`serverInstance.tabDesc.${item.id}`) }}</span>
          </span>
        </button>
      </nav>

      <div class="si-content" :inert="loading">
        <Transition :css="false" mode="out-in" @enter="enterPage" @leave="leavePage">
          <div v-if="!instances.length" key="__empty" class="si-page">
            <header class="si-head">
              <h2 class="si-title">{{ t(`serverInstance.tab.${section}`) }}</h2>
              <p class="si-sub">{{ t(`serverInstance.tabDesc.${section}`) }}</p>
            </header>
            <p class="empty-tip">{{ t("serverInstance.noInstance") }}</p>
          </div>
          <div v-else :key="section" class="si-page">
            <header class="si-head">
              <h2 class="si-title">{{ t(`serverInstance.tab.${section}`) }}</h2>
              <p class="si-sub">{{ t(`serverInstance.tabDesc.${section}`) }}</p>
            </header>
            <GeneralPanel v-if="section === 'general'" />
            <BasePanel v-else-if="section === 'base'" />
            <ModsPanel v-else-if="section === 'mods'" />
            <PacksPanel v-else-if="section === 'packs'" />
            <ConfigsPanel v-else-if="section === 'configs'" />
            <StartPanel v-else />
          </div>
        </Transition>

        <div v-if="loading" class="si-loading">
          <span class="si-spinner" />
          <span>{{ t("serverInstance.loading") }}</span>
        </div>
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
