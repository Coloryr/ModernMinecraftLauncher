<script setup lang="ts">
// 设置窗口 · 实验性功能页：管理员启动与资源映射已接线；URI / 沙盒启动尚未实现，开关禁用。
// 客户端设置被锁定时，开关同样隐藏，这一页只剩解锁入口。
import { computed, onMounted, ref } from "vue";
import { t } from "../../../../lib/i18n";
import {
  attachMapping,
  mapping as mappingResources,
  progress as mapProgress,
  result as mapResult,
  startMapping,
} from "../../../../lib/resourceMap";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import LinkResourcesModal from "../LinkResourcesModal.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsClient } from "../../composables/useSettingsClient";

const props = defineProps<{
  /** 客户端设置的状态与保存方法，实验性开关与锁定状态都在它上面 */
  settings: ReturnType<typeof useSettingsClient>;
  /** 本次设置窗口是否已解锁客户端设置 */
  clientUnlocked: boolean;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const emit = defineEmits<{
  /** 请求输入密码解锁客户端设置，弹框由设置窗口统一管 */
  (e: "unlock"): void;
}>();

const { client, experimental, applyExperimental } = props.settings;

/** 开关属于锁定范围：锁着且本次未解锁时整组隐藏，只留解锁入口 */
const locked = computed(() => client.value.lockClient && !props.clientUnlocked);

/** 已下载资源列表弹窗是否打开 */
const showResources = ref(false);

// 存量映射的状态放在 lib/resourceMap 里：切到别的设置页再回来，组件会重建，
// 状态留在组件里的话进度条就没了

/** 处理进度百分比 */
const mapPercent = computed(() => {
  const total = mapProgress.value?.total ?? 0;
  if (total <= 0) return 0;
  return Math.min(100, ((mapProgress.value?.done ?? 0) / total) * 100);
});

/** 总数还不知道（0 / 0）：进度条走无限滚动 */
const mapRolling = computed(() => (mapProgress.value?.total ?? 0) <= 0);

/** 处理中的 done/total 文字 */
const mapProgressText = computed(() =>
  t("winSettings.resourceLinkProgress", {
    done: mapProgress.value?.done ?? 0,
    total: mapProgress.value?.total ?? 0,
  }),
);

// 窗口重新打开时任务可能还在跑：把状态接回来，进度条立刻续上
onMounted(() => void attachMapping());

/** 上一次处理的统计文字 */
const mapState = computed(() =>
  mapResult.value
    ? t("winSettings.resourceLinkMapDone", {
        instances: mapResult.value.instances,
        mapped: mapResult.value.mapped,
        failed: mapResult.value.failed,
        skipped: mapResult.value.skipped,
      })
    : "",
);
</script>

<template>
  <SettingsGroup v-if="!locked" id="experimental" title-key="winSettings.secExperimental"
    :flash="flashGroup === 'experimental'">
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.launcherAdmin") }}</span>
      </div>
      <BaseSwitch v-model="experimental.launcherAdmin" @update:model-value="applyExperimental" />
    </div>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.instanceAdmin") }}</span>
      </div>
      <BaseSwitch v-model="experimental.instanceAdmin" @update:model-value="applyExperimental" />
    </div>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.resourceLink") }}</span>
        <span class="switch-state">{{ t("winSettings.resourceLinkDesc") }}</span>
      </div>
      <BaseSwitch v-model="experimental.resourceLink" @update:model-value="applyExperimental" />
    </div>
    <template v-if="experimental.resourceLink">
      <div class="map-actions">
        <BaseButton size="sm" variant="accent" :disabled="mappingResources" @click="startMapping">
          {{ mappingResources ? t("winSettings.resourceLinkMapping") : t("winSettings.resourceLinkMap") }}
        </BaseButton>
        <BaseButton size="sm" @click="showResources = true">{{ t("winSettings.resourceLinkList") }}</BaseButton>
        <span v-if="mapState" class="map-state">{{ mapState }}</span>
      </div>
      <div v-if="mappingResources" class="map-progress">
        <div class="map-progress-track">
          <div class="map-progress-fill" :class="{ 'is-roll': mapRolling }"
            :style="mapRolling ? undefined : { width: mapPercent + '%' }" />
        </div>
        <span class="map-state">{{ mapProgressText }}</span>
      </div>
      <p class="field-desc">{{ t("winSettings.resourceLinkMapHint") }}</p>
    </template>

    <!-- 还没实现的开关：只置灰，不写额外说明 -->
    <div class="switch-row dim">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.uriLaunch") }}</span>
      </div>
      <BaseSwitch v-model="experimental.uriLaunch" disabled />
    </div>
    <div class="switch-row dim">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.sandboxLaunch") }}</span>
      </div>
      <BaseSwitch v-model="experimental.sandboxLaunch" disabled />
    </div>
  </SettingsGroup>

  <!-- 已下载资源列表：只读，用于核对公共目录里到底有哪些文件 -->
  <LinkResourcesModal v-if="showResources" @close="showResources = false" />

  <!-- 锁定客户端设置：那一页在锁定时隐藏，解锁入口放这里 -->
  <SettingsGroup v-if="client.lockClient" id="clientLock" title-key="winSettings.secClientLock"
    :flash="flashGroup === 'clientLock'">
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.clientLockOn") }}</span>
        <span v-if="clientUnlocked" class="switch-state">{{ t("winSettings.clientLockUnlocked") }}</span>
      </div>
      <BaseButton v-if="!clientUnlocked" size="sm" @click="emit('unlock')">
        {{ t("winSettings.clientLockUnlock") }}
      </BaseButton>
    </div>
  </SettingsGroup>
</template>

<style scoped>
/* 存量映射按钮与结果文字同一行；结果太长时换行，不挤按钮 */
.map-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.map-state {
  font-size: 11.5px;
  color: var(--text-dim);
}

.map-progress {
  display: flex;
  align-items: center;
  gap: 8px;
}

.map-progress-track {
  flex: 1;
  height: 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 999px;
  overflow: hidden;
}

.map-progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.15s;
}
</style>
