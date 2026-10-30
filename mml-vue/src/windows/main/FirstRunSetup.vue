<script setup lang="ts">
// 首次使用设置：旧数据（ColorMC / .minecraft）刚搬完时问一次主页面样式、背景图与透明模式
//
// 三项都是**选中即生效**（后面主窗口就在弹窗底下，能立刻看到效果），所以只有一个「完成」
// 按钮负责关窗；之后要改走设置窗口。弹窗只在导入真的搬过数据之后才出现，见 MainWindow。
import { computed, onMounted, ref } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { t } from "../../lib/i18n";
import { setViewMode, viewMode, type ViewMode } from "../../lib/settings";
import { applyWindowAlpha, bgImage, bgLoading, setBgImage, setBgImageFromOriginal } from "../../lib/appearance";
import { loadGuiConfig, saveGuiConfig } from "../../lib/guiConfig";
import { isTauri } from "../../lib/util";

const emit = defineEmits<{ (e: "close"): void }>();

/** 主页面样式的候选：分组 / 平铺 / 列表 */
const MODES: { value: ViewMode; icon: "folder" | "grid" | "list"; label: string; desc: string }[] = [
  { value: "group", icon: "folder", label: "mode.group", desc: "firstRun.modeGroupDesc" },
  { value: "grid", icon: "grid", label: "mode.grid", desc: "firstRun.modeGridDesc" },
  { value: "list", icon: "list", label: "mode.list", desc: "firstRun.modeListDesc" },
];

/** 透明模式的档位：窗口不透明度，单位 % */
const ALPHAS = [100, 85, 65] as const;

/** 当前档位，只用于高亮；真值在 gui_config 的 client.windowOpacity */
const alpha = ref<number>(85);

/** 透明模式的选项文案，语言切换后跟着重算 */
const alphaOptions = computed(() => [
  { value: "100", label: t("firstRun.alphaNone") },
  { value: "85", label: t("firstRun.alphaHalf") },
  { value: "65", label: t("firstRun.alphaGlass") },
]);

/** 先不画：配置回来之前不知道有没有被服主锁住，锁着的话首启设置也不该出现 */
const visible = ref(false);

// 打开时先确认没被锁，再按当前配置高亮透明档位（调过的话落在最近的一档）
onMounted(async () => {
  const cfg = await loadGuiConfig();
  if (cfg?.client.lockClient) {
    emit("close");
    return;
  }
  visible.value = true;
  if (!cfg) return;
  const current = cfg.client.windowOpacity;
  alpha.value = ALPHAS.reduce(
    (best, v) => (Math.abs(v - current) < Math.abs(best - current) ? v : best),
    ALPHAS[0],
  );
});

/**
 * 换主页面样式：写配置并广播，主窗口立刻换布局
 *
 * - `v`: 分组 / 平铺 / 列表
 */
function pickMode(v: ViewMode) {
  setViewMode(v);
}

/**
 * 换透明档位：写 CSS 变量再落盘，弹窗底下的主窗口当场就透
 *
 * - `v`: 选项值，字符串形态的百分比
 */
function pickAlpha(v: string) {
  const value = Number(v);
  alpha.value = value;
  applyWindowAlpha(value);
  void saveGuiConfig({ client: { windowOpacity: value } });
}

/** 缩略图的背景样式；没图时不给，交给 .bg-preview.empty 的斜纹占位 */
const bgStyle = computed(() =>
  bgImage.value ? { backgroundImage: "url('" + bgImage.value + "')" } : undefined,
);

/** 选一张本地图片当背景；后端负责缩放、落盘并记进配置 */
async function pickImage() {
  if (!isTauri()) return;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    title: t("firstRun.bgPick"),
    multiple: false,
    filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
  });
  if (typeof picked === "string" && picked) await setBgImageFromOriginal(picked);
}

/** 清除背景图：后端落盘的图与配置里的来源一起删掉 */
function clearImage() {
  void setBgImage("");
}
</script>

<template>
  <BaseModal v-if="visible" :title="t('firstRun.title')" :width="560" @close="emit('close')">
    <div class="first-run">
      <p class="run-intro">{{ t("firstRun.intro") }}</p>

      <!-- 主页面样式：三张卡并排，选中即生效 -->
      <section class="run-sec">
        <h4 class="run-title">{{ t("firstRun.modeTitle") }}</h4>
        <div class="mode-grid">
          <button v-for="m in MODES" :key="m.value" type="button" class="mode-card"
            :class="{ active: viewMode === m.value }" @click="pickMode(m.value)">
            <GlyphIcon :name="m.icon" :size="18" :weight="1.8" />
            <span class="mode-name">{{ t(m.label) }}</span>
            <span class="mode-desc">{{ t(m.desc) }}</span>
          </button>
        </div>
      </section>

      <!-- 背景图：缩略图 + 选择 / 清除 -->
      <section class="run-sec">
        <h4 class="run-title">{{ t("firstRun.bgTitle") }}</h4>
        <div class="bg-row">
          <div class="bg-preview" :class="{ empty: !bgImage }" :style="bgStyle"></div>
          <div class="bg-actions">
            <div class="bg-btns">
              <BaseButton size="sm" variant="accent" :disabled="bgLoading" @click="pickImage">
                {{ t("firstRun.bgPick") }}
              </BaseButton>
              <BaseButton size="sm" :disabled="!bgImage || bgLoading" @click="clearImage">
                {{ t("firstRun.bgClear") }}
              </BaseButton>
            </div>
            <p class="run-hint">{{ t("firstRun.bgHint") }}</p>
          </div>
        </div>
      </section>

      <!-- 透明模式：三档，越往右越透 -->
      <section class="run-sec">
        <h4 class="run-title">{{ t("firstRun.alphaTitle") }}</h4>
        <SegmentedTabs :model-value="String(alpha)" :options="alphaOptions"
          @update:model-value="pickAlpha" />
        <p class="run-hint">{{ t("firstRun.alphaHint") }}</p>
      </section>

      <div class="modal-actions">
        <BaseButton variant="accent" @click="emit('close')">{{ t("firstRun.done") }}</BaseButton>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
.run-intro {
  margin: 0 0 14px;
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--text-dim);
}

/* 三段之间用细分隔线断开，弹窗里不至于挤成一团 */
.run-sec {
  padding: 14px 0;
  border-top: 1px solid var(--border);
}

.run-title {
  margin: 0 0 10px;
  font-size: 13px;
  font-weight: 700;
  color: var(--text);
}

.run-hint {
  margin: 8px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* 主页面样式：三张等宽卡，图标在上、名字与说明在下 */
.mode-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.mode-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  color: var(--text-dim);
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, color 0.12s;
}

.mode-card:hover {
  border-color: var(--accent-border);
  background: var(--bg-hover);
  color: var(--text);
}

.mode-card.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
}

.mode-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.mode-card.active .mode-name {
  color: var(--accent);
}

.mode-desc {
  font-size: 11.5px;
  color: var(--text-dim);
  line-height: 1.5;
}

/* 背景图：左边缩略图，右边按钮与说明 */
.bg-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.bg-preview {
  width: 132px;
  height: 74px;
  flex-shrink: 0;
  border-radius: 10px;
  border: 1px solid var(--border);
  background-color: var(--bg-card);
  background-size: cover;
  background-position: center;
}

/* 还没选图时留一个斜纹占位，别是一块纯色 */
.bg-preview.empty {
  background-image: repeating-linear-gradient(45deg, var(--bg-hover) 0 8px, transparent 8px 16px);
}

.bg-actions {
  min-width: 0;
  flex: 1;
}

.bg-btns {
  display: flex;
  gap: 8px;
}
</style>
