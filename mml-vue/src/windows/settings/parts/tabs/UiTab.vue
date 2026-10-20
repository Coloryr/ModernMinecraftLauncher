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
  /** 窗口外观、自定义 CSS 与整页包的状态与保存方法，来自 useSettingsClient */
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

// 窗口外观与自定义界面都属于 gui_config.client，由 useSettingsClient 持有
const {
  client,
  applyClient,
  applyWindowEffect,
  applyWindowOpacity,
  customUi,
  importingCustomPage,
  customPageState,
  customPageEntry,
  refreshCustomUi,
  addCustomCss,
  setCustomCssEnabled,
  removeCustomCss,
  importCustomPage,
  removeCustomPage,
  openCustomUiDir,
} = props.look;

// 系统字体枚举是重活：进这个标签时才拉；整页包的导入状态也在这时重拉
onMounted(() => {
  void loadFonts();
  void refreshCustomUi();
});

/** 自定义 CSS 总开关即改即存，各窗口靠 client-config-change 重新挂样式 */
function toggleCustomCss(on: boolean) {
  client.value.customCssOn = on;
  void applyClient();
}

/** 完全自定义界面总开关同上；关掉后主窗口回内置界面，整页包保留 */
function toggleCustomPage(on: boolean) {
  client.value.customPageOn = on;
  void applyClient();
}
</script>

<template>
  <SettingsGroup id="general" title-key="winSettings.secGeneral" :flash="flashGroup === 'general'">
    <!-- 语言 / 字体：两个下拉并排一行 -->
    <div class="general-row">
      <div class="general-col">
        <label class="field-label">{{ t("winSettings.language") }}</label>
        <select class="field-select lang-select" :value="locale"
          @change="onLangChange(($event.target as HTMLSelectElement).value)">
          <option value="zh_cn">{{ t("winSettings.langZhCn") }}</option>
          <option value="en_us">{{ t("winSettings.langEnUs") }}</option>
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

  <SettingsGroup id="customCss" title-key="winSettings.secCustomCss" :flash="flashGroup === 'customCss'">
    <p class="field-desc">{{ t("winSettings.customCssDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.customCssOn") }}</span>
        <span class="switch-state">{{ t("winSettings.customCssOnDesc") }}</span>
      </div>
      <BaseSwitch :model-value="client.customCssOn" @update:model-value="toggleCustomCss" />
    </div>

    <!-- 已安装的样式：一行一条，开关 / 文件名 / 删除；顺序即注入顺序 -->
    <ul v-if="client.customCss.length" class="custom-css-list">
      <li v-for="item in client.customCss" :key="item.id" class="custom-css-item">
        <BaseSwitch :model-value="item.enabled"
          @update:model-value="(v) => setCustomCssEnabled(item.id, v)" />
        <span class="custom-css-name" :title="item.name">{{ item.name }}</span>
        <BaseButton size="sm" variant="danger" @click="removeCustomCss(item.id)">
          {{ t("winSettings.customCssRemove") }}
        </BaseButton>
      </li>
    </ul>
    <p v-else class="field-desc">{{ t("winSettings.customCssEmpty") }}</p>

    <div class="custom-ui-actions">
      <BaseButton size="sm" variant="accent" @click="addCustomCss">
        {{ t("winSettings.customCssImport") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCustomUiDir">{{ t("winSettings.customUiOpenDir") }}</BaseButton>
    </div>
    <p class="field-desc">{{ t("winSettings.customCssHint") }}</p>
  </SettingsGroup>

  <SettingsGroup id="customPage" title-key="winSettings.secCustomPage" :flash="flashGroup === 'customPage'">
    <p class="field-desc">{{ t("winSettings.customPageDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.customPageOn") }}</span>
        <span class="switch-state">{{ t("winSettings.customPageOnDesc") }}</span>
      </div>
      <BaseSwitch :model-value="client.customPageOn" @update:model-value="toggleCustomPage" />
    </div>

    <div class="custom-ui-actions">
      <BaseButton size="sm" variant="accent" :disabled="importingCustomPage" @click="importCustomPage">
        {{ importingCustomPage ? t("winSettings.customUiImporting") : t("winSettings.customPageImport") }}
      </BaseButton>
      <BaseButton size="sm" variant="danger" :disabled="!customUi?.pageInstalled" @click="removeCustomPage">
        {{ t("winSettings.customPageRemove") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCustomUiDir">{{ t("winSettings.customUiOpenDir") }}</BaseButton>
      <span class="custom-ui-state">{{ customPageState }}</span>
    </div>
    <p class="field-desc">{{ t("winSettings.customPageHint") }}</p>
    <label class="field-label">{{ t("winSettings.customPageEntry") }}</label>
    <p class="custom-ui-entry">{{ customPageEntry }}</p>
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
    <select v-model="client.windowEffect" class="field-select" @change="applyWindowEffect">
      <option value="auto">{{ t("winSettings.windowEffectAuto") }}</option>
      <option value="mica">{{ t("winSettings.windowEffectMica") }}</option>
      <option value="acrylic">{{ t("winSettings.windowEffectAcrylic") }}</option>
      <option value="none">{{ t("winSettings.windowEffectNone") }}</option>
    </select>

    <label class="field-label">{{ t("winSettings.windowOpacity") }}</label>
    <NumberStepper v-model="client.windowOpacity" :min="0" :max="100" :step="5"
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

<style scoped>
/* 已安装的样式：一行一条，开关 / 文件名 / 删除；顺序即注入顺序 */
.custom-css-list {
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.custom-css-item {
  display: flex;
  align-items: center;
  gap: 10px;
}

/* 文件名可能很长，占满剩余宽度并省略，不把删除按钮挤走 */
.custom-css-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
}

/* 自定义界面：按钮一行排开，状态跟在后面，窄窗口自动换行 */
.custom-ui-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  flex-wrap: wrap;
}

/* 状态行是按钮的补充说明，不抢按钮的视觉重量 */
.custom-ui-state {
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 入口地址是只读展示：URL 可能很长，用等宽字体并允许在任意位置断行 */
.custom-ui-entry {
  margin: 4px 0 0;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11.5px;
  color: var(--text-dim);
  word-break: break-all;
}
</style>
