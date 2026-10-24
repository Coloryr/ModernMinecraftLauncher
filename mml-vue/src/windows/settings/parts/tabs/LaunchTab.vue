<script setup lang="ts">
// 设置窗口 · 游戏启动标签：游戏窗口 / 内存 / JVM / 游戏参数 / 启动命令
//
// 内存冲突即最小 > 最大时，把两个步进器标红并给出提示，保存也会被拦住，见 useSettingsLaunch；
// 冲突出现时把焦点落到"要改小"的最小内存上；NumberStepper 的输入框在组件内部，取 $el 里的 input。
import { computed, nextTick, ref, watch } from "vue";
import { t } from "../../../../lib/i18n";
import { COMMON_MEMORY, COMMON_MIN_MEMORY, COMMON_RESOLUTIONS } from "../../../../lib/resolutions";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import MemoryHint from "../../../../components/MemoryHint.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import SettingsGroup from "../SettingsGroup.vue";
import SaveStateText from "../SaveStateText.vue";
import type { useSettingsLaunch } from "../../composables/useSettingsLaunch";
import { memoryLabel } from "../../../../lib/format";

const props = defineProps<{
  /** 游戏启动标签的状态与保存方法，来自 useSettingsLaunch */
  settings: ReturnType<typeof useSettingsLaunch>;
  /** 锁定时隐藏 JVM / 游戏参数 / 启动命令分组 */
  locked?: boolean;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();


const {
  run,
  win,
  envLines,
  gcOptions,
  memoryConflict,
  commitEnvLines,
  addEnvLine,
  removeEnvLine,
  saveState,
} = props.settings;

/** 应用一个内存预设：填最大内存；最小内存高于它时一并顶上去 */
function applyMemory(mib: number) {
  // run 在设置加载完成前可能为空；模板用 v-if="run && win" 挡住渲染
  if (!run.value) return;
  run.value.maxMemory = mib;
  if (run.value.minMemory > mib) run.value.minMemory = mib;
}

// ---------------- 预设下拉：与实例设置同一套（口径见 components/LaunchArgsPanel.vue） ----------------

/** 手输过（下拉选了「自定义」）就一直显示「自定义」，直到值真的变了 */
const resManual = ref(false);
const memManual = ref(false);
const minMemManual = ref(false);

/** 分辨率下拉的当前项：全屏 / 预设的 `宽x高` / `custom`，三者互斥 */
const resPick = computed(() => {
  const w = win.value;
  if (!w) return "custom";
  if (w.fullScreen) return "fullscreen";
  const hit = COMMON_RESOLUTIONS.find((r) => r.w === w.width && r.h === w.height);
  return !resManual.value && hit ? `${hit.w}x${hit.h}` : "custom";
});

/** 最大内存下拉的当前项：预设的 MB 字符串，手输或没匹配上预设都是 `custom` */
const memPick = computed(() => {
  const r = run.value;
  return r && !memManual.value && COMMON_MEMORY.includes(r.maxMemory) ? String(r.maxMemory) : "custom";
});

/** 最小内存下拉的当前项：口径与最大内存相同，预设表多出 256 / 512 两档 */
const minMemPick = computed(() => {
  const r = run.value;
  return r && !minMemManual.value && COMMON_MIN_MEMORY.includes(r.minMemory) ? String(r.minMemory) : "custom";
});

// 值真的变了才退出手输态：改别的字段（JVM、参数）不能把正在手输的框收走
watch(() => `${win.value?.width}x${win.value?.height}`, () => {
  resManual.value = false;
});
watch(() => run.value?.maxMemory, () => {
  memManual.value = false;
});
watch(() => run.value?.minMemory, () => {
  minMemManual.value = false;
});

/** 分辨率下拉：全屏 / 预设 / 自定义 三选一；后两者都要退出全屏，否则选项与状态对不上 */
function onResPick(e: Event) {
  const w = win.value;
  if (!w) return;
  const v = (e.target as HTMLSelectElement).value;
  if (v === "fullscreen") {
    resManual.value = false;
    w.fullScreen = true;
    return;
  }
  if (v === "custom") {
    resManual.value = true;
    w.fullScreen = false;
    return;
  }
  const [width, height] = v.split("x").map(Number);
  resManual.value = false;
  w.width = width;
  w.height = height;
  w.fullScreen = false;
}

/** 最大内存下拉：选预设就填最大内存，与实例设置那侧同一个 `applyMemory` */
function onMemPick(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  if (v === "custom") {
    memManual.value = true;
    return;
  }
  memManual.value = false;
  applyMemory(Number(v));
}

/** 最小内存下拉：选预设就填最小内存；高于上限时压到上限，与上限侧的夹取对称 */
function onMinMemPick(e: Event) {
  const r = run.value;
  if (!r) return;
  const v = (e.target as HTMLSelectElement).value;
  if (v === "custom") {
    minMemManual.value = true;
    return;
  }
  minMemManual.value = false;
  r.minMemory = Math.min(Number(v), r.maxMemory);
}

/** 内存冲突时聚焦最小内存（要改的就是它） */
const minMemoryRef = ref<InstanceType<typeof NumberStepper> | null>(null);
watch(memoryConflict, async (bad) => {
  if (!bad) return;
  await nextTick();
  const root = minMemoryRef.value?.$el as HTMLElement | undefined;
  root?.querySelector("input")?.focus();
});
</script>

<template>
  <template v-if="run && win">
    <SettingsGroup id="gameWindow" title-key="winSettings.secGameWindow" :flash="flashGroup === 'gameWindow'">
      <!-- 窗口大小：与实例设置同一套 —— 预设下拉（含全屏）定尺寸，续行给宽 / 高；
           全屏时尺寸无效，续行整行收起 -->
      <div class="args-row">
        <span class="args-label">{{ t("args.windowSize") }}</span>
        <select class="field-select pick-select res-pick" :value="resPick" @change="onResPick">
          <option v-for="r in COMMON_RESOLUTIONS" :key="`${r.w}x${r.h}`" :value="`${r.w}x${r.h}`">
            {{ r.w }}×{{ r.h }}
          </option>
          <option value="fullscreen">{{ t("args.fullscreen") }}</option>
          <option value="custom">{{ t("args.custom") }}</option>
        </select>
      </div>
      <div v-if="!win!.fullScreen" class="args-row sub-row">
        <span class="sub-tag">{{ t("args.width") }}</span>
        <NumberStepper v-model="win!.width" :min="100" :max="65535" />
        <span class="sub-tag">{{ t("args.height") }}</span>
        <NumberStepper v-model="win!.height" :min="100" :max="65535" />
      </div>
    </SettingsGroup>

    <SettingsGroup id="memory" title-key="winSettings.secMemory" :flash="flashGroup === 'memory'">
      <!-- 内存：最小一行、最大续一行，各自预设下拉 + 数值框 + MB，口径与窗口大小、与实例设置都相同 -->
      <div class="args-row" :class="{ 'field-invalid': memoryConflict }">
        <span class="args-label">{{ t("args.memory") }}</span>
        <span class="sub-tag">{{ t("args.minMemory") }}</span>
        <select class="field-select pick-select" :value="minMemPick" @change="onMinMemPick">
          <option v-for="m in COMMON_MIN_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
          <option value="custom">{{ t("args.custom") }}</option>
        </select>
        <NumberStepper ref="minMemoryRef" v-model="run!.minMemory" :min="256" :max="65536" :step="256" />
        <span class="mem-unit">MB</span>
      </div>
      <div class="args-row sub-row" :class="{ 'field-invalid': memoryConflict }">
        <span class="sub-tag">{{ t("args.maxMemory") }}</span>
        <select class="field-select pick-select" :value="memPick" @change="onMemPick">
          <option v-for="m in COMMON_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
          <option value="custom">{{ t("args.custom") }}</option>
        </select>
        <NumberStepper v-model="run!.maxMemory" :min="256" :max="65536" :step="256" />
        <span class="mem-unit">MB</span>
      </div>
      <!-- 本机内存参考值：给设最大内存一个依据（已用 / 共 · 剩余 + 左占用右剩余的条，拿不到会写明原因） -->
      <MemoryHint class="mem-line" />
      <!-- 冲突时明确说清"没保存"，避免用户以为改上了 -->
      <p v-if="memoryConflict" class="lock-server-hint">{{ t("winSettings.memoryConflict") }}</p>
      <div class="set-save-row right">
        <SaveStateText :state="saveState" />
      </div>
    </SettingsGroup>

    <SettingsGroup v-if="!locked" id="jvm" title-key="winSettings.secJvm" :flash="flashGroup === 'jvm'">
      <div class="switch-row">
        <div class="switch-text">
          <span class="switch-label">{{ t("winSettings.isolateHome") }}</span>
          <span class="switch-state">{{ t("winSettings.isolateHomeDesc") }}</span>
        </div>
        <BaseSwitch v-model="run!.isolateHome" />
      </div>

      <div class="grid-2">
        <div>
          <label class="field-label">{{ t("winSettings.gcMode") }}</label>
          <SegmentedTabs v-model="run!.gcMode" :options="gcOptions" />
        </div>
        <div class="switch-list">
          <div class="switch-row">
            <span>{{ t("winSettings.colorasm") }}</span>
            <BaseSwitch v-model="run!.colorasm" />
          </div>
          <div class="switch-row">
            <span>{{ t("winSettings.removeJvmArg") }}</span>
            <BaseSwitch v-model="run!.removeJvmArg" />
          </div>
        </div>
      </div>

      <label class="field-label">{{ t("winSettings.jvmEnv") }}</label>
      <div class="dns-lines">
        <div v-for="(_, i) in envLines" :key="i" class="line-row">
          <input v-model="envLines[i].key" class="field-input grow" spellcheck="false" autocomplete="off"
            :placeholder="t('winSettings.envKey')" @change="commitEnvLines" />
          <input v-model="envLines[i].value" class="field-input grow" spellcheck="false" autocomplete="off"
            :placeholder="t('winSettings.envValue')" />
          <button class="line-del" aria-label="remove" v-tip="t('args.removeLine')" @click="removeEnvLine(i)">
            <GlyphIcon name="close" :size="13" />
          </button>
        </div>
        <button class="line-add" @click="addEnvLine">
          <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
        </button>
      </div>

      <label class="field-label">{{ t("winSettings.jvmArgs") }}</label>
      <textarea v-model="run!.jvmArgs" class="field-input args-input" spellcheck="false" />
    </SettingsGroup>

    <SettingsGroup v-if="!locked" id="gameArgs" title-key="winSettings.secGameArgs" :flash="flashGroup === 'gameArgs'">
      <div class="switch-row">
        <span>{{ t("winSettings.removeGameArg") }}</span>
        <BaseSwitch v-model="run!.removeGameArg" />
      </div>
      <label class="field-label">{{ t("winSettings.gameArgs") }}</label>
      <textarea v-model="run!.gameArgs" class="field-input args-input" spellcheck="false" />
    </SettingsGroup>

    <SettingsGroup v-if="!locked" id="launchCmd" title-key="winSettings.secLaunchCmd" :flash="flashGroup === 'launchCmd'">
      <div class="switch-list">
        <div class="switch-row">
          <span>{{ t("winSettings.preLaunch") }}</span>
          <BaseSwitch v-model="run!.launchPreRun" />
        </div>
        <div class="switch-row" :class="{ dim: !run.launchPreRun }">
          <span>{{ t("winSettings.preSameTime") }}</span>
          <BaseSwitch v-model="run!.preRunWithGame" :disabled="!run.launchPreRun" />
        </div>
        <div class="switch-row">
          <span>{{ t("winSettings.postLaunch") }}</span>
          <BaseSwitch v-model="run!.launchPostRun" />
        </div>
      </div>
      <template v-if="run.launchPreRun">
        <label class="field-label">{{ t("winSettings.preCmd") }}</label>
        <input v-model="run!.preRunArg" class="field-input" spellcheck="false" autocomplete="off" />
      </template>
      <template v-if="run.launchPostRun">
        <label class="field-label">{{ t("winSettings.postCmd") }}</label>
        <input v-model="run!.postRunArg" class="field-input" spellcheck="false" autocomplete="off" />
      </template>
    </SettingsGroup>
  </template>
  <p v-else class="field-desc">{{ t("winSettings.tauriOnly") }}</p>
</template>

<!-- 参数行的行 / 标签 / 子标签 / 预设下拉：与实例设置（LaunchArgsPanel）共用这一份 -->
<style scoped src="../../../../styles/parts/args-row.css"></style>
