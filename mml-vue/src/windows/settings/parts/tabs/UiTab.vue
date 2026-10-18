<script setup lang="ts">
// 设置窗口 · 界面标签（含窗口设置）
//
// 状态与保存逻辑在 composables/useSettingsUi；窗口外观那组属于 gui_config.client（useSettingsClient），
// 只是显示在这一页。这里只负责排版与分组（SettingsGroup）。
import { onMounted } from "vue";
import { t } from "../../../../lib/i18n";
import { fontStack } from "../../../../lib/fonts";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsUi } from "../../composables/useSettingsUi";
import type { useSettingsClient } from "../../composables/useSettingsClient";

const props = defineProps<{
  /** 界面标签的状态与保存方法，来自 useSettingsUi */
  settings: ReturnType<typeof useSettingsUi>;
  /** 窗口外观（材质 / 透明度）的状态与保存方法，来自 useSettingsClient */
  look: ReturnType<typeof useSettingsClient>;
  /** 需要高亮脉冲的分组 id（搜索命中 / 刚恢复默认） */
  flashGroup?: string;
}>();


const {
  inTauri,
  locale,
  onLangChange,
  windowMode,
  onModeChange,
  side,
  onSideChange,
  startup,
  onStartupChange,
  view,
  onViewModeChange,
  themeValue,
  onThemeChange,
  fonts,
  fontsLoading,
  loadFonts,
  fontPick,
  setFontFamily,
  animations,
  setAnimations,
  accent,
  setAccent,
  customAccent,
  setCustomAccent,
  ACCENTS,
  bgImage,
  setBgImage,
  bgLoading,
  bgFileInput,
  bgSourceInput,
  loadBgSource,
  pickBgImage,
  onBgFile,
  bgSizeDraft,
  applyBgSize,
  bgNativeSize,
  bgOpacity,
  setBgOpacity,
  bgBlur,
  setBgBlur,
} = props.settings;

// 窗口外观的数据属于 gui_config.client，由 useSettingsClient 持有
const { client, applyWindowEffect, applyWindowOpacity } = props.look;

// 系统字体枚举是重活：进这个标签时才拉
onMounted(() => void loadFonts());
</script>

<template>
  <SettingsGroup id="general" title-key="winSettings.secGeneral" :flash="flashGroup === 'general'">
    <!-- 语言 / 字体：两个下拉并排一行 -->
    <div class="general-row">
      <div class="general-col">
        <label class="field-label">{{ t("winSettings.language") }}</label>
        <select class="field-select lang-select" :value="locale"
          @change="onLangChange(($event.target as HTMLSelectElement).value)">
          <option value="zh_cn">简体中文</option>
          <option value="en_us">English</option>
        </select>
      </div>
      <div class="general-col">
        <label class="field-label">{{ t("winSettings.font") }}</label>
        <div class="font-row">
          <select v-model="fontPick" class="field-select font-select" @change="setFontFamily(fontPick)">
            <option value="">{{ t("winSettings.fontDefault") }}</option>
            <!-- 每个名字用它自己的字体渲染：挑字体时直接看字形，不用先选中再试 -->
            <option v-for="f in fonts" :key="f" :value="f" :style="{ fontFamily: fontStack(f) }">{{ f }}</option>
          </select>
        </div>
      </div>
    </div>
    <p class="field-desc">{{ t("winSettings.fontDesc") }}</p>
    <p v-if="fontsLoading" class="hint">{{ t("winSettings.fontLoading") }}</p>

    <!-- 界面动画开关 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.animations") }}</span>
        <span class="switch-state">{{ t("winSettings.animationsDesc") }}</span>
      </div>
      <BaseSwitch :model-value="animations" @update:model-value="setAnimations" />
    </div>
  </SettingsGroup>

  <SettingsGroup id="theme" title-key="winSettings.secTheme" :flash="flashGroup === 'theme'">
    <!-- 主题：跟随系统 / 浅色 / 深色 -->
    <label class="field-label">{{ t("winSettings.theme") }}</label>
    <SegmentedTabs :model-value="themeValue" :options="[
      { value: 'System', label: t('winSettings.themeSystem') },
      { value: 'Light', label: t('winSettings.themeLight') },
      { value: 'Dark', label: t('winSettings.themeDark') },
    ]" @update:model-value="onThemeChange" />

    <!-- 强调色 -->
    <label class="field-label">{{ t("winSettings.accent") }}</label>
    <p class="field-desc">{{ t("winSettings.accentDesc") }}</p>
    <div class="accent-list">
      <button v-for="a in ACCENTS" :key="a.id" class="accent-swatch" :class="{ active: accent === a.id }"
        :style="{ background: a.color }" :aria-label="t(`winSettings.accent.${a.id}`)"
        v-tip="t(`winSettings.accent.${a.id}`)" @click="setAccent(a.id)">
        <svg v-if="accent === a.id" viewBox="0 0 24 24" width="16" height="16" fill="none" :stroke="a.check"
          stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="m5 13 4 4L19 7" />
        </svg>
      </button>

      <!-- 自定义：彩虹色块，点击弹出系统取色器 -->
      <label class="accent-swatch accent-custom" :class="{ active: accent === 'custom' }"
        v-tip="t('winSettings.accent.custom')">
        <svg v-if="accent === 'custom'" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="#fff"
          stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" class="accent-check">
          <path d="m5 13 4 4L19 7" />
        </svg>
        <input type="color" :value="customAccent" @input="setCustomAccent(($event.target as HTMLInputElement).value)" />
      </label>
    </div>
  </SettingsGroup>

  <SettingsGroup id="window" title-key="winSettings.secWindow" :flash="flashGroup === 'window'">
    <!-- 两态设置用 Switch 开关 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.windowMode") }}</span>
        <span class="switch-state">{{ windowMode === "Multi" ? t("winSettings.multi") : t("winSettings.single")
        }}</span>
      </div>
      <BaseSwitch :model-value="windowMode === 'Multi'"
        @update:model-value="(v) => onModeChange(v ? 'Multi' : 'Single')" />
    </div>
    <p v-if="inTauri" class="hint">{{ t("winSettings.windowModeRestart") }}</p>
  </SettingsGroup>

  <!-- 窗口外观：材质由系统在窗口背后合成（Rust 侧 windows/effects.rs），落 gui_config.client.windowEffect -->
  <SettingsGroup id="window-look" title-key="winSettings.secWindowLook" :flash="flashGroup === 'window-look'">
    <label class="field-label">{{ t("winSettings.windowEffect") }}</label>
    <p class="field-desc">{{ t("winSettings.windowEffectDesc") }}</p>
    <select v-model="client.windowEffect" class="field-select" @change="applyWindowEffect">
      <option value="auto">{{ t("winSettings.windowEffectAuto") }}</option>
      <option value="mica">{{ t("winSettings.windowEffectMica") }}</option>
      <option value="acrylic">{{ t("winSettings.windowEffectAcrylic") }}</option>
      <option value="none">{{ t("winSettings.windowEffectNone") }}</option>
    </select>

    <label class="field-label">{{ t("winSettings.windowOpacity") }}</label>
    <p class="field-desc">{{ t("winSettings.windowOpacityDesc") }}</p>
    <NumberStepper v-model="client.windowOpacity" :min="60" :max="100" :step="5"
      @update:model-value="applyWindowOpacity" />
  </SettingsGroup>

  <SettingsGroup id="mainWindow" title-key="winSettings.secMainWindow" :flash="flashGroup === 'mainWindow'">
    <!-- 启动页：启动器启动后主窗口先落到哪一屏 -->
    <label class="field-label">{{ t("winSettings.startupPage") }}</label>
    <SegmentedTabs :model-value="startup" :options="[
      { value: 'Home', label: t('winSettings.startupHome') },
      { value: 'List', label: t('winSettings.startupList') },
    ]" @update:model-value="onStartupChange" />
    <p class="field-desc">{{ t("winSettings.startupPageDesc") }}</p>

    <!-- 实例列表显示模式：主窗口里不再有这个切换控件，只在这里改 -->
    <label class="field-label">{{ t("winSettings.viewMode") }}</label>
    <SegmentedTabs :model-value="view" :options="[
      { value: 'group', label: t('mode.group') },
      { value: 'grid', label: t('mode.grid') },
      { value: 'list', label: t('mode.list') },
    ]" @update:model-value="onViewModeChange" />
    <p class="field-desc">{{ t("winSettings.viewModeDesc") }}</p>

    <label class="field-label">{{ t("winSettings.sidebar") }}</label>
    <SegmentedTabs :model-value="side" :options="[
      { value: 'Left', label: t('winSettings.sidebarLeft') },
      { value: 'Right', label: t('winSettings.sidebarRight') },
    ]" @update:model-value="onSideChange" />
    <p class="field-desc">{{ t("winSettings.sidebarDesc") }}</p>
  </SettingsGroup>

  <SettingsGroup id="bgImage" title-key="winSettings.bgImage" :flash="flashGroup === 'bgImage'">
    <p class="field-desc">{{ t("winSettings.bgImageDesc") }}</p>
    <div class="bg-buttons">
      <!-- 图片地址：本地文件路径或网址 -->
      <input v-model="bgSourceInput" class="field-input bg-url-input" :placeholder="t('winSettings.bgUrlPlaceholder')"
        spellcheck="false" autocomplete="off" @keydown.enter="loadBgSource" />
      <BaseButton size="sm" variant="accent" :disabled="bgLoading" @click="loadBgSource">
        {{ bgLoading ? t("winSettings.bgLoading") : t("winSettings.bgLoad") }}
      </BaseButton>
      <BaseButton v-if="bgImage" size="sm" variant="danger" :disabled="bgLoading" @click="setBgImage('')">
        {{ t("winSettings.bgClear") }}
      </BaseButton>
      <BaseButton size="sm" variant="accent" :disabled="bgLoading" @click="pickBgImage">{{ t("winSettings.bgPick") }}
      </BaseButton>
    </div>
    <input :ref="(el) => (bgFileInput = el as HTMLInputElement | null)" class="bg-file" type="file" accept="image/*"
      @change="onBgFile" />

    <template v-if="bgImage">
      <div class="range-row row-card">
        <span class="range-label">{{ t("winSettings.bgOpacity") }}</span>
        <input class="range" type="range" min="5" max="100" :value="bgOpacity"
          @input="setBgOpacity(Number(($event.target as HTMLInputElement).value))" />
        <span class="range-value">{{ bgOpacity }}%</span>
      </div>
      <div class="range-row row-card">
        <span class="range-label">{{ t("winSettings.bgBlur") }}</span>
        <input class="range" type="range" min="0" max="40" :value="bgBlur"
          @input="setBgBlur(Number(($event.target as HTMLInputElement).value))" />
        <span class="range-value">{{ bgBlur }}px</span>
      </div>
      <!-- 原始大小：后端把图片分辨率缩放到原图的百分之多少；点「应用」才生效，
           且只改分辨率，不改变图在窗口里的显示大小 -->
      <div class="range-row row-card">
        <span class="range-label">{{ t("winSettings.bgNativeSize") }}</span>
        <input class="range" type="range" min="10" max="100" :value="bgSizeDraft"
          @input="bgSizeDraft = Number(($event.target as HTMLInputElement).value)" />
        <span class="range-value">{{ bgSizeDraft }}%</span>
        <BaseButton size="sm" variant="accent" :disabled="bgLoading || bgSizeDraft === bgNativeSize"
          @click="applyBgSize">{{ bgLoading ? t("winSettings.bgLoading") : t("winSettings.bgApply") }}</BaseButton>
      </div>
    </template>
  </SettingsGroup>
</template>
