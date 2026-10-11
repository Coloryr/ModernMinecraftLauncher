<script setup lang="ts">
// 设置窗口 · 皮肤与头像标签
//
// 结构与其它标签一致：每个小节都是「分组标题（强调色竖条 + 分隔线）→ 说明 → 控件」，
// 所以拆成四组：头像显示模式 / 皮肤显示模式 / 披风显示模式 / 样例预览。
import { t } from "../../../../lib/i18n";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsSkin } from "../../composables/useSettingsSkin";

const props = defineProps<{
  /** 皮肤与头像标签的状态与选项，来自 useSettingsSkin */
  settings: ReturnType<typeof useSettingsSkin>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const {
  imageBase,
  headPreviewUrl,
  skinPreviewUrl,
  capePreviewUrl,
  headType,
  headX,
  headY,
  setHeadConfig,
  headTypeOptions,
  onHeadTypeChange,
  skinDisplay,
  skinPitch,
  skinYaw,
  setSkinConfig,
  skinDisplayOptions,
  onSkinDisplayChange,
  skinAngleAdjustable,
  capeDisplay,
  capePitch,
  capeYaw,
  setCapeConfig,
  capeDisplayOptions,
  onCapeDisplayChange,
  capeAngleAdjustable,
  offsetAdjustable,
} = props.settings;
</script>

<template>
  <SettingsGroup id="head" title-key="winSettings.headDisplay" :flash="flashGroup === 'head'">
    <p class="field-desc">{{ t("winSettings.headDisplayDesc") }}</p>
    <SegmentedTabs :model-value="headType" :options="headTypeOptions" @update:model-value="onHeadTypeChange" />
    <div v-if="offsetAdjustable" class="grid-2">
      <div>
        <label class="field-label">{{ t("winSettings.rotX") }}</label>
        <NumberStepper v-model="headX" :min="-90" :max="90"
          @update:model-value="(v) => setHeadConfig(headType, v, headY)" />
      </div>
      <div>
        <label class="field-label">{{ t("winSettings.rotY") }}</label>
        <NumberStepper v-model="headY" :min="-180" :max="180"
          @update:model-value="(v) => setHeadConfig(headType, headX, v)" />
      </div>
    </div>
  </SettingsGroup>

  <SettingsGroup id="skin" title-key="winSettings.skinDisplay" :flash="flashGroup === 'skin'">
    <p class="field-desc">{{ t("winSettings.skinDisplayDesc") }}</p>
    <SegmentedTabs :model-value="skinDisplay" :options="skinDisplayOptions" @update:model-value="onSkinDisplayChange" />
    <div v-if="skinAngleAdjustable" class="grid-2">
      <div>
        <label class="field-label">{{ t("winSettings.rotX") }}</label>
        <NumberStepper v-model="skinPitch" :min="-90" :max="90"
          @update:model-value="(v) => setSkinConfig(skinDisplay, v, skinYaw)" />
      </div>
      <div>
        <label class="field-label">{{ t("winSettings.rotY") }}</label>
        <NumberStepper v-model="skinYaw" :min="-180" :max="180"
          @update:model-value="(v) => setSkinConfig(skinDisplay, skinPitch, v)" />
      </div>
    </div>
  </SettingsGroup>

  <SettingsGroup id="cape" title-key="winSettings.capeDisplay" :flash="flashGroup === 'cape'">
    <p class="field-desc">{{ t("winSettings.capeDisplayDesc") }}</p>
    <SegmentedTabs :model-value="capeDisplay" :options="capeDisplayOptions" @update:model-value="onCapeDisplayChange" />
    <div v-if="capeAngleAdjustable" class="grid-2">
      <div>
        <label class="field-label">{{ t("winSettings.rotX") }}</label>
        <NumberStepper v-model="capePitch" :min="-90" :max="90"
          @update:model-value="(v) => setCapeConfig(capeDisplay, v, capeYaw)" />
      </div>
      <div>
        <label class="field-label">{{ t("winSettings.rotY") }}</label>
        <NumberStepper v-model="capeYaw" :min="-180" :max="180"
          @update:model-value="(v) => setCapeConfig(capeDisplay, capePitch, v)" />
      </div>
    </div>
  </SettingsGroup>

  <SettingsGroup id="preview" title-key="winSettings.secPreview">
    <!-- 样例预览：内置纤细皮肤按当前头像 / 皮肤配置实时渲染，配置变化经 URL 版本号重取 -->
    <div class="sample-row">
      <div class="sample-box">
        <img v-if="imageBase" class="sample-img" :src="headPreviewUrl" alt="" />
        <span class="sample-label">{{ t("winSettings.headSample") }}</span>
      </div>
      <div class="sample-box">
        <img v-if="imageBase" class="sample-img skin" :src="skinPreviewUrl" alt="" />
        <span class="sample-label">{{ t("winSettings.skinSample") }}</span>
      </div>
      <div class="sample-box">
        <img v-if="imageBase" class="sample-img cape" :src="capePreviewUrl" alt="" />
        <span class="sample-label">{{ t("winSettings.capeSample") }}</span>
      </div>
    </div>
  </SettingsGroup>
</template>
