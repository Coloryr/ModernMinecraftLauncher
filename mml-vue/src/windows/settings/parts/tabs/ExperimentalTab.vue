<script setup lang="ts">
// 设置窗口 · 实验性功能页：管理员启动与资源映射已接线；URI / 沙盒启动尚未实现，开关禁用。
// 客户端设置被锁定时，开关同样隐藏，这一页只剩解锁入口。
import { computed, ref } from "vue";
import { t, tErr } from "../../../../lib/i18n";
import { showToast } from "../../../../lib/toast";
import { commands, type ResourceMapResultDto } from "../../../../lib/bindings";
import BaseButton from "../../../../components/ui/BaseButton.vue";
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

/** 存量映射是否正在跑 */
const mappingResources = ref(false);
/** 上一次存量映射结果 */
const mapResult = ref<ResourceMapResultDto | null>(null);

/** 处理所有实例里带在线信息的模组 / 资源包 */
async function mapResources() {
  mappingResources.value = true;
  mapResult.value = null;
  try {
    mapResult.value = await commands.settings.mapResources();
    showToast(t("winSettings.resourceLinkMapDone", {
      instances: mapResult.value.instances,
      mapped: mapResult.value.mapped,
      failed: mapResult.value.failed,
      skipped: mapResult.value.skipped,
    }));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    mappingResources.value = false;
  }
}

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
        <BaseButton size="sm" variant="accent" :disabled="mappingResources" @click="mapResources">
          {{ mappingResources ? t("winSettings.resourceLinkMapping") : t("winSettings.resourceLinkMap") }}
        </BaseButton>
        <span v-if="mapState" class="map-state">{{ mapState }}</span>
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
</style>
