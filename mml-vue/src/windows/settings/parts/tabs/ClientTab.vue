<script setup lang="ts">
// 设置窗口 · 客户端设置标签：服务器 / MOTD / 登录方式锁定 / 实例锁定 / 自定义主页面 / 游戏标题
//
// 游戏标题那组的数据属于 `window`，由 useSettingsLaunch 持有，所以这里额外接一个 `win`。
import { nextTick, ref, watch } from "vue";
import { t } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SettingsGroup from "../SettingsGroup.vue";
import SaveStateText from "../SaveStateText.vue";
import type { useSettingsClient } from "../../composables/useSettingsClient";
import type { WindowSettingDto } from "../../../../lib/bindings";

const props = defineProps<{
  /** 客户端设置标签的状态与保存方法，来自 useSettingsClient */
  settings: ReturnType<typeof useSettingsClient>;
  /** 游戏窗口设置（游戏标题三开关与标题文字） */
  win: WindowSettingDto | null;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();


const {
  client,
  applyClient,
  experimental,
  applyExperimental,
  serverAddr,
  pickMusicFile,
  loginTypeLabel,
  addLockType,
  addLockName,
  addLockServer,
  addLockHasServer,
  addLockOptions,
  addLock,
  removeLock,
  lockServerError,
  lockServerPlaceholder,
  lockInstances,
  lockMissing,
  onLockInstanceChange,
  customHome,
  importingCustomHome,
  customHomeState,
  importCustomHome,
  openCustomHomeDir,
  removeCustomHome,
  saveState,
} = props.settings;

/** 服务器信息校验失败时把焦点落到那个输入框（"就地标红并聚焦"） */
const lockServerInput = ref<HTMLInputElement | null>(null);
watch(lockServerError, async (msg) => {
  if (!msg) return;
  await nextTick();
  lockServerInput.value?.focus();
});
</script>

<template>
  <SettingsGroup id="servers" title-key="winSettings.secServers" :flash="flashGroup === 'servers'">
    <!-- 服务器地址：自动进服与 MOTD 显示共用 -->
    <label class="field-label">{{ t("winSettings.serverAddress") }}</label>
    <input v-model="serverAddr" class="field-input" spellcheck="false" autocomplete="off"
      placeholder="mc.example.com:25565" v-tip="t('winSettings.serverAddressHint')" />
    <p class="field-desc">{{ t("winSettings.serverAddressHint") }}</p>

    <!-- 自动进服：启动时自动进入上面配置的服务器 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.autoJoin") }}</span>
        <span class="switch-state">{{ t("winSettings.autoJoinDesc") }}</span>
      </div>
      <BaseSwitch v-model="client.autoJoin" @update:model-value="applyClient" />
    </div>

    <!-- MOTD 卡片显示与刷新间隔 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.motdCard") }}</span>
        <span class="switch-state">{{ t("winSettings.motdCardDesc") }}</span>
      </div>
      <BaseSwitch v-model="client.motdCard" @update:model-value="applyClient" />
    </div>
    <div class="switch-row" :class="{ dim: !client.motdCard }">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.motdInterval") }}</span>
        <span class="switch-state">{{ t("winSettings.motdIntervalDesc") }}</span>
      </div>
      <NumberStepper v-model="client.motdInterval" :min="5" :max="600" @update:model-value="applyClient" />
    </div>

    <div class="set-save-row right">
      <SaveStateText :state="saveState" />
    </div>
  </SettingsGroup>

  <SettingsGroup id="loginLock" title-key="winSettings.secLoginLock" :flash="flashGroup === 'loginLock'">
    <p class="field-desc">{{ t("winSettings.loginLockDesc") }}</p>
    <!-- 总开关：关闭时锁定列表不生效 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.loginLockOn") }}</span>
        <span class="switch-state">{{ t("winSettings.loginLockOnDesc") }}</span>
      </div>
      <BaseSwitch v-model="client.loginLockOn" @update:model-value="applyClient" />
    </div>
    <!-- 已锁定的条目列表（总开关打开时才可编辑） -->
    <template v-if="client.loginLockOn">
      <div v-if="client.loginLock.length" class="switch-list">
        <div v-for="(e, i) in client.loginLock" :key="e.ty" class="switch-row lock-item">
          <span class="lock-item-text">
            {{ loginTypeLabel(e.ty) }}<template v-if="e.name"> — {{ e.name }}</template><template v-if="e.server"> — {{
              e.server }}</template>
          </span>
          <BaseButton size="sm" variant="danger" @click="removeLock(i)">
            {{ t("winSettings.loginLockRemove") }}
          </BaseButton>
        </div>
      </div>
      <!-- 空状态：开着总开关但一条都没加时，别让下面直接跟"添加行"显得断片 -->
      <p v-else class="field-desc">{{ t("winSettings.loginLockEmpty") }}</p>
      <!-- 添加条目：选类型；外置登录 / 自定义皮肤站 / 统一通行证必须填服务器信息，可重复添加不同地址 -->
      <div class="lock-add">
        <select v-model="addLockType" class="field-select lock-add-type">
          <option v-for="ty in addLockOptions" :key="ty" :value="ty">
            {{ loginTypeLabel(ty) }}
          </option>
        </select>
        <input v-if="addLockHasServer" v-model="addLockName" class="field-input lock-add-name" spellcheck="false"
          autocomplete="off" :placeholder="t('winSettings.lockModelName')" />
        <input v-if="addLockHasServer" :ref="(el) => (lockServerInput = el as HTMLInputElement | null)"
          v-model="addLockServer" class="field-input lock-add-server" :class="{ 'lock-server-error': lockServerError }"
          spellcheck="false" autocomplete="off" :placeholder="lockServerPlaceholder" @input="lockServerError = ''" />
        <BaseButton size="sm" variant="accent" :disabled="!addLockOptions.length" @click="addLock">
          {{ t("winSettings.loginLockAdd") }}
        </BaseButton>
      </div>
      <p v-if="lockServerError" class="lock-server-hint">{{ lockServerError }}</p>
    </template>
  </SettingsGroup>

  <SettingsGroup id="instanceLock" title-key="winSettings.secInstanceLock" :flash="flashGroup === 'instanceLock'">
    <p class="field-desc">{{ t("winSettings.instanceLockDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.instanceLockOn") }}</span>
        <span class="switch-state">{{ t("winSettings.instanceLockOnDesc") }}</span>
      </div>
      <select class="field-select lock-select" :value="client.lockInstance"
        @change="onLockInstanceChange(($event.target as HTMLSelectElement).value)">
        <option value="">{{ t("winSettings.instanceLockNone") }}</option>
        <option v-for="i in lockInstances" :key="i.uuid" :value="i.uuid">{{ i.name }}</option>
        <!-- 锁定的实例已被删掉：补一条选中项，让用户能看到当前锁的是什么并切回「不锁定」 -->
        <option v-if="lockMissing" :value="client.lockInstance">
          {{ t("winSettings.instanceLockMissing") }}
        </option>
      </select>
    </div>
  </SettingsGroup>

  <SettingsGroup id="music" title-key="winSettings.secMusic" :flash="flashGroup === 'music'">
    <p class="field-desc">{{ t("winSettings.musicDesc") }}</p>

    <!-- 总开关：关掉就停播，标题栏那枚入口也不显示 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.musicOn") }}</span>
        <span class="switch-state">{{ t("winSettings.musicOnDesc") }}</span>
      </div>
      <BaseSwitch v-model="client.musicOn" @update:model-value="applyClient" />
    </div>

    <!-- 来源：本地文件或 http(s) 直链；填完回车 / 失焦才落盘，别每敲一个字存一次 -->
    <label class="field-label">{{ t("winSettings.musicSource") }}</label>
    <div class="path-row">
      <input v-model="client.musicSource" class="field-input" spellcheck="false" autocomplete="off"
        :placeholder="t('winSettings.musicSourcePlaceholder')" @change="applyClient" />
      <BaseButton size="sm" @click="pickMusicFile">{{ t("winSettings.musicPick") }}</BaseButton>
    </div>
    <p class="field-desc">{{ t("winSettings.musicSourceDesc") }}</p>

    <div class="set-save-row right">
      <SaveStateText :state="saveState" />
    </div>
  </SettingsGroup>

  <SettingsGroup id="customHome" title-key="winSettings.secCustomHome" :flash="flashGroup === 'customHome'">
    <p class="field-desc">{{ t("winSettings.customHomeDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.customHomeOn") }}</span>
        <span class="switch-state">{{ t("winSettings.customHomeOnDesc") }}</span>
      </div>
      <BaseSwitch v-model="client.customHome" @update:model-value="applyClient" />
    </div>
    <div class="custom-home-row">
      <BaseButton size="sm" :disabled="importingCustomHome" @click="importCustomHome">
        {{ t("winSettings.customHomeImport") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCustomHomeDir">{{ t("winSettings.customHomeOpenDir") }}</BaseButton>
      <BaseButton size="sm" variant="danger" :disabled="!customHome?.installed" @click="removeCustomHome">
        {{ t("winSettings.customHomeRemove") }}
      </BaseButton>
      <span class="custom-home-state">{{ customHomeState }}</span>
    </div>

    <!-- 导入中提示：导入只是校验 + 复制压缩包，通常瞬间完成，所以没有进度百分比 -->
    <div v-if="importingCustomHome" class="java-progress">
      <span class="java-progress-label">{{ t("winSettings.customHomeImporting") }}</span>
      <div class="java-progress-track">
        <div class="java-progress-fill rolling" />
      </div>
    </div>
  </SettingsGroup>

  <!-- 游戏标题：游戏窗口标题栏的自定义文字，全局默认值；数据属于 window 设置 -->
  <SettingsGroup v-if="win" id="gameTitle" title-key="winSettings.secGameTitle" :flash="flashGroup === 'gameTitle'">
    <div class="switch-list">
      <div class="switch-row">
        <span>{{ t("winSettings.editTitle") }}</span>
        <BaseSwitch v-model="win.editTitle" />
      </div>
      <div class="switch-row">
        <span>{{ t("winSettings.randomTitle") }}</span>
        <BaseSwitch v-model="win.randomTitle" />
      </div>
      <div class="switch-row">
        <span>{{ t("winSettings.cycleTitle") }}</span>
        <BaseSwitch v-model="win.cycleTitle" />
      </div>
    </div>
    <template v-if="win.editTitle">
      <label class="field-label">{{ t("winSettings.gameTitle") }}</label>
      <input v-model="win.gameTitle" class="field-input" spellcheck="false" autocomplete="off" />
    </template>
    <template v-if="win.cycleTitle">
      <label class="field-label">{{ t("winSettings.titleDelay") }}</label>
      <NumberStepper v-model="win.titleDelay" :min="100" :max="600000" :step="100" />
    </template>
  </SettingsGroup>

  <!-- 实验性功能：还在验证中的开关集中放这一组，位置固定在客户端设置的最后一组。
       开关只落盘、行为都没接线（见 gui_config 的 ExperimentalConfig）。
       加开关时同步 search.ts 的 GROUP_TITLES（已有）与索引项、必要时再进 RESETTABLE_GROUPS -->
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
