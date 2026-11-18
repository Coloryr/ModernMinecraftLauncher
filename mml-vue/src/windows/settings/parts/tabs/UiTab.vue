<script setup lang="ts">
// 设置窗口 · 界面标签（含窗口设置）
//
// 状态与保存逻辑在 composables/useSettingsUi；窗口外观那组属于 gui_config.client（useSettingsClient），
// 只是显示在这一页。这里只负责排版与分组（SettingsGroup）。
import { onMounted } from "vue";
import { t } from "../../../../lib/i18n";
import { detectOs } from "../../../../lib/titlebar";
import { fontStack } from "../../../../lib/fonts";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
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
  /** 锁定时隐藏服主定制与外观分组 */
  locked?: boolean;
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
  gradientMode,
  setGradientMode,
  ACCENTS,
  bgImage,
  setBgImage,
  bgCopyAsk,
  answerBgCopy,
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
  bgFrosted,
  setBgFrosted,
} = props.settings;

/** 窗口材质（云母 / 亚克力）只有 Windows 提供；macOS / Linux 没有对应实现，整块藏掉 */
const supportsMaterial = detectOs() === "windows";

// 窗口外观与自定义界面都属于 gui_config.client，由 useSettingsClient 持有
const {
  client,
  applyClient,
  applyWindowEffect,
  applyWindowOpacity,
  customUi,
  importingCustomPage,
  customPageState,
  refreshCustomUi,
  addCustomCss,
  setCustomCssEnabled,
  removeCustomCss,
  moveCustomCss,
  importCustomPage,
  selectCustomPage,
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

/** 完全自定义界面总开关同上；关掉后主窗口回内置界面，主题包保留 */
function toggleCustomPage(on: boolean) {
  client.value.customPageOn = on;
  void applyClient();
}

/** 某个主题包的文件数；状态里还没有这一条时按 0 显示 */
function pageFileCount(id: string): number {
  return customUi.value?.pages.find((page) => page.id === id)?.fileCount ?? 0;
}
</script>

<template>
  <SettingsGroup id="general" title-key="winSettings.secGeneral" :flash="flashGroup === 'general'">
    <!-- 语言 / 字体：两个下拉并排一行 -->
    <div class="general-row flex gap-3.5">
      <div class="general-col flex-1 min-w-0">
        <label class="field-label">{{ t("winSettings.language") }}</label>
        <select class="field-select lang-select" :value="locale"
          @change="onLangChange(($event.target as HTMLSelectElement).value)">
          <option value="zh_cn">{{ t("winSettings.langZhCn") }}</option>
          <option value="en_us">{{ t("winSettings.langEnUs") }}</option>
        </select>
      </div>
      <div class="general-col flex-1 min-w-0">
        <label class="field-label">{{ t("winSettings.font") }}</label>
        <div class="font-row flex items-center gap-2.5">
          <select v-model="fontPick" class="field-select font-select flex-1" @change="setFontFamily(fontPick)">
            <option value="">{{ t("winSettings.fontDefault") }}</option>
            <!-- 每个名字用它自己的字体渲染：挑字体时直接看字形，不用先选中再试 -->
            <option v-for="f in fonts" :key="f" :value="f" :style="{ fontFamily: fontStack(f) }">{{ f }}</option>
          </select>
        </div>
      </div>
    </div>
    <p class="field-desc">{{ t("winSettings.fontDesc") }}</p>
    <p v-if="fontsLoading" class="hint">{{ t("winSettings.fontLoading") }}</p>
  </SettingsGroup>

  <SettingsGroup v-if="!locked" id="theme" title-key="winSettings.secTheme" :flash="flashGroup === 'theme'">
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
    <div class="accent-list flex flex-wrap gap-3">
      <button v-for="a in ACCENTS" :key="a.id"
        class="accent-swatch w-ctl h-ctl rounded-full border-2 border-border cursor-pointer flex items-center justify-center p-0"
        :class="{ active: accent === a.id }"
        :style="{ background: a.color }" :aria-label="t(`winSettings.accent.${a.id}`)"
        v-tip="t(`winSettings.accent.${a.id}`)" @click="setAccent(a.id)">
        <svg v-if="accent === a.id" viewBox="0 0 24 24" width="16" height="16" fill="none" :stroke="a.check"
          stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="m5 13 4 4L19 7" />
        </svg>
      </button>

      <!-- 自定义：彩虹色块，点击弹出系统取色器 -->
      <label class="accent-swatch accent-custom w-ctl h-ctl rounded-full border-2 border-border cursor-pointer flex items-center justify-center p-0"
        :class="{ active: accent === 'custom' }"
        v-tip="t('winSettings.accent.custom')">
        <svg v-if="accent === 'custom'" viewBox="0 0 24 24" width="16" height="16" fill="none"
          class="accent-icon accent-check" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="m5 13 4 4L19 7" />
        </svg>
        <input class="absolute inset-0 opacity-0 cursor-pointer" type="color" :value="customAccent"
          @input="setCustomAccent(($event.target as HTMLInputElement).value)" />
      </label>
    </div>

  </SettingsGroup>

  <SettingsGroup v-if="!locked" id="customUi" title-key="winSettings.secCustomUi"
    :flash="flashGroup === 'customUi'">
    <h4 class="custom-ui-title">{{ t("winSettings.secCustomCss") }}</h4>
    <p class="field-desc">{{ t("winSettings.customCssDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.customCssOn") }}</span>
        <span class="switch-state">{{ t("winSettings.customCssOnDesc") }}</span>
      </div>
      <BaseSwitch :model-value="client.customCssOn" @update:model-value="toggleCustomCss" />
    </div>

    <!-- 已安装的样式：一行一条，开关 / 文件名 / 上移下移 / 删除；列表顺序即注入顺序 -->
    <ul v-if="client.customCss.length" class="custom-css-list mt-2 p-0 list-none flex flex-col gap-1.5">
      <li v-for="(item, index) in client.customCss" :key="item.id" class="custom-css-item flex items-center gap-2.5">
        <BaseSwitch :model-value="item.enabled" @update:model-value="(v) => setCustomCssEnabled(item.id, v)" />
        <span class="custom-css-name flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-body" :title="item.name">{{ item.name }}</span>
        <!-- 上移 / 下移：排在第一个 / 最后一个时对应的按钮禁用 -->
        <button class="ui-btn-icon" :disabled="index === 0" v-tip="t('winSettings.customCssMoveUp')"
          :aria-label="t('winSettings.customCssMoveUp')" @click="moveCustomCss(item.id, -1)">
          <GlyphIcon name="arrow-up" :size="14" :weight="2" />
        </button>
        <button class="ui-btn-icon" :disabled="index === client.customCss.length - 1"
          v-tip="t('winSettings.customCssMoveDown')" :aria-label="t('winSettings.customCssMoveDown')"
          @click="moveCustomCss(item.id, 1)">
          <GlyphIcon name="arrow-down" :size="14" :weight="2" />
        </button>
        <BaseButton size="sm" variant="danger" @click="removeCustomCss(item.id)">
          {{ t("winSettings.customCssRemove") }}
        </BaseButton>
      </li>
    </ul>
    <p v-else class="field-desc">{{ t("winSettings.customCssEmpty") }}</p>

    <div class="custom-ui-actions flex items-center gap-2 mt-2.5 flex-wrap">
      <BaseButton size="sm" variant="accent" @click="addCustomCss">
        {{ t("winSettings.customCssImport") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCustomUiDir">{{ t("winSettings.customUiOpenDir") }}</BaseButton>
    </div>

    <div class="custom-ui-sep" />

    <h4 class="custom-ui-title">{{ t("winSettings.secCustomPage") }}</h4>
    <p class="field-desc">{{ t("winSettings.customPageDesc") }}</p>
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.customPageOn") }}</span>
        <span class="switch-state">{{ t("winSettings.customPageOnDesc") }}</span>
      </div>
      <BaseSwitch :model-value="client.customPageOn" @update:model-value="toggleCustomPage" />
    </div>

    <!-- 主题包可以装多份，但同一时间只选中一份；选中即启用 -->
    <ul v-if="client.customPages.length" class="custom-page-list mt-2 p-0 list-none flex flex-col gap-1.5">
      <li v-for="item in client.customPages" :key="item.id" class="custom-page-item flex items-center gap-2.5">
        <label class="custom-page-pick flex flex-1 min-w-0 items-center gap-2.5 cursor-pointer">
          <input type="radio" name="mml-custom-page" :checked="client.customPageId === item.id"
            @change="selectCustomPage(item.id)" />
          <span class="custom-page-name flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-body"
            :title="item.name">{{ item.name }}</span>
          <span class="custom-page-count text-meta text-dim">
            {{ t("winSettings.customPageFileCount", { count: pageFileCount(item.id) }) }}
          </span>
        </label>
        <BaseButton size="sm" variant="danger" @click="removeCustomPage(item.id)">
          {{ t("winSettings.customPageRemove") }}
        </BaseButton>
      </li>
    </ul>
    <p v-else class="field-desc">{{ t("winSettings.customPageEmpty") }}</p>

    <div class="custom-ui-actions flex items-center gap-2 mt-2.5 flex-wrap">
      <BaseButton size="sm" variant="accent" :disabled="importingCustomPage" @click="importCustomPage">
        {{ importingCustomPage ? t("winSettings.customUiImporting") : t("winSettings.customPageImport") }}
      </BaseButton>
      <BaseButton size="sm" @click="openCustomUiDir">{{ t("winSettings.customUiOpenDir") }}</BaseButton>
      <span class="custom-ui-state text-meta text-dim">{{ customPageState }}</span>
    </div>
  </SettingsGroup>

  <SettingsGroup id="window" title-key="winSettings.secWindow" :flash="flashGroup === 'window'">
    <!-- 窗口模式：与主窗口启动页同一套两态分段选择 -->
    <label class="field-label">{{ t("winSettings.windowMode") }}</label>
    <SegmentedTabs :model-value="windowMode" :options="[
      { value: 'Single', label: t('winSettings.single') },
      { value: 'Multi', label: t('winSettings.multi') },
    ]" @update:model-value="onModeChange" />
    <p v-if="inTauri" class="hint">{{ t("winSettings.windowModeRestart") }}</p>

    <!-- 界面动画开关 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.animations") }}</span>
        <span class="switch-state">{{ t("winSettings.animationsDesc") }}</span>
      </div>
      <BaseSwitch :model-value="animations" @update:model-value="setAnimations" />
    </div>

    <!-- 渐变色模式：关掉后强调色与实例图标改用纯色 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.gradient") }}</span>
        <span class="switch-state">{{ t("winSettings.gradientDesc") }}</span>
      </div>
      <BaseSwitch :model-value="gradientMode" @update:model-value="setGradientMode" />
    </div>

    <!-- 透明高斯模糊：叠在背景图上的表面改用背景模糊；没设背景图时置灰 -->
    <div class="switch-row">
      <div class="switch-text">
        <span class="switch-label">{{ t("winSettings.bgFrosted") }}</span>
        <span class="switch-state">{{ t("winSettings.bgFrostedDesc") }}</span>
      </div>
      <BaseSwitch :model-value="bgFrosted" :disabled="!bgImage" @update:model-value="setBgFrosted" />
    </div>
  </SettingsGroup>

  <!-- 窗口外观：材质由系统在窗口背后合成（Rust 侧 windows/effects.rs），落 gui_config.client.windowEffect -->
  <SettingsGroup v-if="!locked" id="window-look" title-key="winSettings.secWindowLook"
    :flash="flashGroup === 'window-look'">
    <template v-if="supportsMaterial">
      <label class="field-label">{{ t("winSettings.windowEffect") }}</label>
      <select v-model="client.windowEffect" class="field-select" @change="applyWindowEffect">
        <option value="none">{{ t("winSettings.windowEffectNone") }}</option>
        <option value="mica">{{ t("winSettings.windowEffectMica") }}</option>
        <option value="acrylic">{{ t("winSettings.windowEffectAcrylic") }}</option>
      </select>
      <!-- 窗口透明在建窗时定，切换材质要重开窗口才看得到 -->
      <p class="field-desc">{{ t("winSettings.windowEffectRestart") }}</p>
    </template>

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

  <SettingsGroup v-if="!locked" id="bgImage" title-key="winSettings.bgImage" :flash="flashGroup === 'bgImage'">
    <p class="field-desc">{{ t("winSettings.bgImageDesc") }}</p>
    <div class="bg-buttons flex items-stretch gap-2.5">
      <!-- 图片地址：本地文件路径或网址 -->
      <input v-model="bgSourceInput" class="field-input bg-url-input flex-1 min-w-0"
        :placeholder="t('winSettings.bgUrlPlaceholder')"
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
    <input :ref="(el) => (bgFileInput = el as HTMLInputElement | null)" class="bg-file hidden" type="file"
      accept="image/*"
      @change="onBgFile" />

    <template v-if="bgImage">
      <div class="range-row row-card is-plain flex items-center gap-3">
        <span class="range-label w-[72px] shrink-0 text-body text-dim">{{ t("winSettings.bgOpacity") }}</span>
        <input class="range" type="range" min="5" max="100" :value="bgOpacity"
          @input="setBgOpacity(Number(($event.target as HTMLInputElement).value))" />
        <span class="range-value w-[52px] shrink-0 text-right text-[12px] [font-variant-numeric:tabular-nums] text-dim">{{ bgOpacity }}%</span>
      </div>
      <div class="range-row row-card is-plain flex items-center gap-3">
        <span class="range-label w-[72px] shrink-0 text-body text-dim">{{ t("winSettings.bgBlur") }}</span>
        <input class="range" type="range" min="0" max="40" :value="bgBlur"
          @input="setBgBlur(Number(($event.target as HTMLInputElement).value))" />
        <span class="range-value w-[52px] shrink-0 text-right text-[12px] [font-variant-numeric:tabular-nums] text-dim">{{ bgBlur }}px</span>
      </div>
      <!-- 原始大小：后端把图片分辨率缩放到原图的百分之多少；点「应用」才生效，
           且只改分辨率，不改变图在窗口里的显示大小 -->
      <div class="range-row row-card is-plain flex items-center gap-3">
        <span class="range-label w-[72px] shrink-0 text-body text-dim">{{ t("winSettings.bgNativeSize") }}</span>
        <input class="range" type="range" min="10" max="100" :value="bgSizeDraft"
          @input="bgSizeDraft = Number(($event.target as HTMLInputElement).value)" />
        <span class="range-value w-[52px] shrink-0 text-right text-[12px] [font-variant-numeric:tabular-nums] text-dim">{{ bgSizeDraft }}%</span>
        <!-- 行内文字按钮：标准按钮 32px 会把这一行撑高，三条滑杆的 4px 行距就乱了 -->
        <button type="button"
          class="range-apply shrink-0 h-5 px-0.5 border-0 bg-transparent text-accent text-body [font-family:inherit] cursor-pointer transition-[color] duration-[120ms]"
          :disabled="bgLoading || bgSizeDraft === bgNativeSize"
          @click="applyBgSize">{{ bgLoading ? t("winSettings.bgLoading") : t("winSettings.bgApply") }}</button>
      </div>
    </template>
  </SettingsGroup>

  <!-- 选完本地图问一句：复制进启动器目录后来源改成相对路径，原图删掉、整个目录搬走都不影响 -->
  <BaseModal v-if="bgCopyAsk" :title="t('winSettings.bgCopyAskTitle')" @close="answerBgCopy(false)">
    <p class="modal-text">{{ t("winSettings.bgCopyAskText") }}</p>
    <div class="modal-actions">
      <BaseButton @click="answerBgCopy(false)">{{ t("winSettings.bgCopyAskNo") }}</BaseButton>
      <BaseButton variant="accent" :disabled="bgLoading" @click="answerBgCopy(true)">
        {{ t("winSettings.bgCopyAskYes") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 排版都在模板的工具类上，这里只留状态与两处非 token 的装饰 */

.range-apply:hover:not(:disabled) {
  text-decoration: underline;
}

.range-apply:disabled {
  color: var(--text-dim);
  cursor: default;
  opacity: 0.6;
}

/* 自定义强调色色块上的对勾：描边取深色块上的白 */
.accent-icon {
  stroke: var(--on-accent);
}

/* 合并后的自定义界面分组：两小节之间用一条细线分开 */
.custom-ui-title {
  margin-bottom: 6px;
  font-size: 13px;
  font-weight: 600;
}

.custom-ui-sep {
  margin: 16px 0;
  border-top: 1px solid var(--border);
}

.custom-page-pick input[type="radio"] {
  accent-color: var(--accent);
}
</style>
