<script setup lang="ts">
// 设置窗口 · 网络与下载标签
//
// 非代理项即改即存；代理项走草稿 + 显式「保存」，见 composables/useSettingsNetwork 的说明。
import { t } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import SettingsGroup from "../SettingsGroup.vue";
import SaveStateText from "../SaveStateText.vue";
import type { useSettingsNetwork } from "../../composables/useSettingsNetwork";

const props = defineProps<{
  /** 网络与下载标签的状态与保存方法，来自 useSettingsNetwork */
  settings: ReturnType<typeof useSettingsNetwork>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();


const {
  network,
  dnsLines,
  proxy,
  proxyDetailVisible,
  proxyDirty,
  sourceOptions,
  proxyModeOptions,
  proxyTypeOptions,
  applyNetwork,
  saveProxy,
  commitDns,
  setDnsLine,
  removeDnsLine,
  addDnsLine,
  saveState,
} = props.settings;
</script>

<template>
  <template v-if="network">
    <SettingsGroup id="download" title-key="winSettings.secDownload" :flash="flashGroup === 'download'">
      <div class="grid-2 dl-grid">
        <div>
          <label class="field-label">{{ t("winSettings.downloadSource") }}</label>
          <SegmentedTabs :model-value="network.source" :options="sourceOptions"
            @update:model-value="(v) => { network!.source = v; applyNetwork(); }" />
        </div>
        <div>
          <label class="field-label">{{ t("winSettings.downloadThread") }}</label>
          <NumberStepper :model-value="network.downloadThread" :min="1" :max="64"
            @update:model-value="(v) => { network!.downloadThread = v; applyNetwork(); }" />
        </div>
      </div>

      <!-- 下载校验 / 自动下载：开关行，即改即存 -->
      <div class="grid-2">
        <div class="switch-row">
          <span>{{ t("winSettings.checkFile") }}</span>
          <BaseSwitch :model-value="network.checkFile"
            @update:model-value="(v) => { network!.checkFile = v; applyNetwork(); }" />
        </div>
        <div class="switch-row">
          <span>{{ t("winSettings.autoDownload") }}</span>
          <BaseSwitch :model-value="network.autoDownload"
            @update:model-value="(v) => { network!.autoDownload = v; applyNetwork(); }" />
        </div>
      </div>
    </SettingsGroup>

    <SettingsGroup id="proxy" title-key="winSettings.secProxy" :flash="flashGroup === 'proxy'">
      <div class="grid-2">
        <div>
          <label class="field-label">{{ t("winSettings.proxyWork") }}</label>
          <SegmentedTabs v-model="proxy.workProxy" :options="proxyModeOptions" />
        </div>
        <div>
          <label class="field-label">{{ t("winSettings.proxyLogin") }}</label>
          <SegmentedTabs v-model="proxy.loginProxy" :options="proxyModeOptions" />
        </div>
      </div>

      <!-- 任一路走手动代理才显示代理详情（改动只进草稿，点保存才落盘） -->
      <template v-if="proxyDetailVisible">
        <div class="grid-2">
          <div>
            <label class="field-label">{{ t("winSettings.proxyType") }}</label>
            <SegmentedTabs v-model="proxy.workProxyType" :options="proxyTypeOptions"
              @update:model-value="(v) => (proxy.loginProxyType = v)" />
          </div>
        </div>
        <div class="grid-2">
          <div>
            <label class="field-label">{{ t("winSettings.proxyIp") }}</label>
            <input v-model="proxy.proxyIp" class="field-input" spellcheck="false" autocomplete="off" />
          </div>
          <div>
            <label class="field-label">{{ t("winSettings.proxyPort") }}</label>
            <input :value="proxy.proxyPort" class="field-input" type="number"
              @change="proxy.proxyPort = Number(($event.target as HTMLInputElement).value) || 0" />
          </div>
          <div>
            <label class="field-label">{{ t("winSettings.proxyUsername") }}</label>
            <input v-model="proxy.proxyUser" class="field-input" spellcheck="false" autocomplete="off" />
          </div>
          <div>
            <label class="field-label">{{ t("winSettings.proxyPassword") }}</label>
            <input v-model="proxy.proxyPassword" class="field-input" type="password" autocomplete="off" />
          </div>
        </div>
      </template>

      <!-- 代理字段较多，保留显式保存按钮（其余网络设置即改即存） -->
      <div class="set-save-row right">
        <SaveStateText :state="saveState" :pending="proxyDirty" />
        <BaseButton variant="accent" size="sm" @click="saveProxy">{{ t("winSettings.save") }}</BaseButton>
      </div>
    </SettingsGroup>

    <SettingsGroup id="dns" title-key="winSettings.dns" :flash="flashGroup === 'dns'">
      <div class="switch-list">
        <div class="switch-row">
          <span>{{ t("winSettings.dnsEnable") }}</span>
          <BaseSwitch :model-value="network.dns.enable"
            @update:model-value="(v) => { network!.dns.enable = v; applyNetwork(); }" />
        </div>
        <div class="switch-row" :class="{ dim: !network.dns.enable }">
          <span>{{ t("winSettings.dnsProxy") }}</span>
          <BaseSwitch :model-value="network.dns.httpProxy" :disabled="!network.dns.enable"
            @update:model-value="(v) => { network!.dns.httpProxy = v; applyNetwork(); }" />
        </div>
      </div>
      <template v-if="network.dns.enable">
        <label class="field-label">{{ t("winSettings.dnsHttps") }}</label>
        <!-- 编辑中只改本地草稿，失焦 / 回车才落盘 -->
        <div class="dns-lines">
          <div v-for="(line, i) in dnsLines" :key="i" class="line-row">
            <input class="field-input grow" :value="line" spellcheck="false" autocomplete="off"
              @input="setDnsLine(i, ($event.target as HTMLInputElement).value)" @change="commitDns(); applyNetwork();"
              @keydown.enter="($event.target as HTMLInputElement).blur()" />
            <button class="line-del" aria-label="remove" v-tip="t('args.removeLine')" @click="removeDnsLine(i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addDnsLine">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>
        </div>
      </template>
    </SettingsGroup>

    <SettingsGroup id="gameCheck" title-key="winSettings.gameCheck" :flash="flashGroup === 'gameCheck'">
      <!-- 关闭"检查xx"后对应的 SHA1 校验一并禁用（没得查自然不用校验） -->
      <div class="grid-2">
        <div class="switch-list">
          <div class="switch-row">
            <span>{{ t("winSettings.checkCore") }}</span>
            <BaseSwitch :model-value="network.check.core"
              @update:model-value="(v) => { network!.check.core = v; applyNetwork(); }" />
          </div>
          <div class="switch-row">
            <span>{{ t("winSettings.checkLib") }}</span>
            <BaseSwitch :model-value="network.check.lib"
              @update:model-value="(v) => { network!.check.lib = v; applyNetwork(); }" />
          </div>
          <div class="switch-row">
            <span>{{ t("winSettings.checkAssets") }}</span>
            <BaseSwitch :model-value="network.check.assets"
              @update:model-value="(v) => { network!.check.assets = v; applyNetwork(); }" />
          </div>
          <div class="switch-row">
            <span>{{ t("winSettings.checkMod") }}</span>
            <BaseSwitch :model-value="network.check.gameMod"
              @update:model-value="(v) => { network!.check.gameMod = v; applyNetwork(); }" />
          </div>
        </div>
        <div class="switch-list">
          <div class="switch-row" :class="{ dim: !network.check.core }">
            <span>{{ t("winSettings.checkCoreSha1") }}</span>
            <BaseSwitch :model-value="network.check.coreSha1" :disabled="!network.check.core"
              @update:model-value="(v) => { network!.check.coreSha1 = v; applyNetwork(); }" />
          </div>
          <div class="switch-row" :class="{ dim: !network.check.lib }">
            <span>{{ t("winSettings.checkLibSha1") }}</span>
            <BaseSwitch :model-value="network.check.libSha1" :disabled="!network.check.lib"
              @update:model-value="(v) => { network!.check.libSha1 = v; applyNetwork(); }" />
          </div>
          <div class="switch-row" :class="{ dim: !network.check.assets }">
            <span>{{ t("winSettings.checkAssetsSha1") }}</span>
            <BaseSwitch :model-value="network.check.assetsSha1" :disabled="!network.check.assets"
              @update:model-value="(v) => { network!.check.assetsSha1 = v; applyNetwork(); }" />
          </div>
          <div class="switch-row" :class="{ dim: !network.check.gameMod }">
            <span>{{ t("winSettings.checkModSha1") }}</span>
            <BaseSwitch :model-value="network.check.modSha1" :disabled="!network.check.gameMod"
              @update:model-value="(v) => { network!.check.modSha1 = v; applyNetwork(); }" />
          </div>
        </div>
      </div>
    </SettingsGroup>
  </template>
  <p v-else class="field-desc">{{ t("winSettings.tauriOnly") }}</p>
</template>
