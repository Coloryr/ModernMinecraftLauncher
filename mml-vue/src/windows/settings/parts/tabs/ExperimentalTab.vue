<script setup lang="ts">
// 设置窗口 · 实验性功能页：还在验证中的开关集中在这里，行为都没接线，只落 gui_config.experimental。
// 单独成栏（侧栏在客户端设置下面），不再挂在客户端设置页末尾。
import { t } from "../../../../lib/i18n";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsClient } from "../../composables/useSettingsClient";

const props = defineProps<{
  /** 客户端设置的状态与保存方法，实验性开关挂在它上面 */
  settings: ReturnType<typeof useSettingsClient>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const { experimental, applyExperimental } = props.settings;
</script>

<template>
  <SettingsGroup id="experimental" title-key="winSettings.secExperimental"
    :flash="flashGroup === 'experimental'">
    <p class="field-desc">{{ t("winSettings.experimentalDesc") }}</p>

    <!-- 管理员启动：启动器自身与它启动的实例各一个开关 -->
    <label class="field-label">{{ t("winSettings.adminLaunch") }}</label>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.launcherAdmin") }}</span>
        <span class="switch-state">{{ t("winSettings.launcherAdminDesc") }}</span>
      </div>
      <BaseSwitch v-model="experimental.launcherAdmin" @update:model-value="applyExperimental" />
    </div>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.instanceAdmin") }}</span>
        <span class="switch-state">{{ t("winSettings.instanceAdminDesc") }}</span>
      </div>
      <BaseSwitch v-model="experimental.instanceAdmin" @update:model-value="applyExperimental" />
    </div>

    <!-- URI 启动与沙盒启动 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.uriLaunch") }}</span>
        <span class="switch-state">{{ t("winSettings.uriLaunchDesc") }}</span>
      </div>
      <BaseSwitch v-model="experimental.uriLaunch" @update:model-value="applyExperimental" />
    </div>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.sandboxLaunch") }}</span>
        <span class="switch-state">{{ t("winSettings.sandboxLaunchDesc") }}</span>
      </div>
      <BaseSwitch v-model="experimental.sandboxLaunch" @update:model-value="applyExperimental" />
    </div>

    <p class="field-desc">{{ t("winSettings.experimentalWip") }}</p>
  </SettingsGroup>
</template>
