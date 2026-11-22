<script setup lang="ts">
// 设置窗口 · 游戏启动标签：游戏窗口 / 内存 / JVM / 游戏参数 / 启动命令
//
// 内存冲突即最小 > 最大时，把两个步进器标红并给出提示，保存也会被拦住，见 useSettingsLaunch；
// 冲突出现时把焦点落到"要改小"的最小内存上；NumberStepper 的输入框在组件内部，取 $el 里的 input。
import { computed, nextTick, ref, watch } from "vue";
import { t } from "../../../../lib/i18n";
import { memoryLabel } from "../../../../lib/format";
import { COMMON_MEMORY, COMMON_MIN_MEMORY, COMMON_RESOLUTIONS } from "../../../../lib/resolutions";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import MemoryHint from "../../../../components/MemoryHint.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SettingsGroup from "../SettingsGroup.vue";
import { useRowFocus } from "../../../../composables/useRowFocus";
import type { useSettingsLaunch } from "../../composables/useSettingsLaunch";
import type { useSettingsClient } from "../../composables/useSettingsClient";

const props = defineProps<{
  /** 游戏启动标签的状态与保存方法，来自 useSettingsLaunch */
  settings: ReturnType<typeof useSettingsLaunch>;
  /** 锁定时隐藏 JVM / 游戏参数 / 启动命令分组 */
  locked?: boolean;
  /** 启动前检查开关，属 GUI 配置，由 useSettingsClient 持有 */
  look: ReturnType<typeof useSettingsClient>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const {
  run,
  win,
  envLines,
  envKeys,
  gcOptions,
  memoryConflict,
  commitEnvLines,
  addEnvLine,
  removeEnvLine,
  jvmArgLines,
  jvmArgKeys,
  commitJvmArgLines,
  addJvmArgLine,
  removeJvmArgLine,
  gameArgLines,
  gameArgKeys,
  commitGameArgLines,
  addGameArgLine,
  removeGameArgLine,
} = props.settings;

/** 点"添加"后新行自动聚焦（与实例设置的启动参数面板共用一份） */
const { focusNew, setRowRef } = useRowFocus();

/** 加一行环境变量并聚焦 */
function addEnv() {
  addEnvLine();
  focusNew("env", envLines.value.length - 1);
}

/** 加一行 JVM 参数并聚焦 */
function addJvmArg() {
  addJvmArgLine();
  focusNew("jvm", jvmArgLines.value.length - 1);
}

/** 加一行游戏参数并聚焦 */
function addGameArg() {
  addGameArgLine();
  focusNew("game", gameArgLines.value.length - 1);
}

/**
 * 失去焦点时删掉没填内容的行
 *
 * 与实例设置同一个口径：「添加」先插空行再让人填，没填就离开的留着只是占地方，
 * 还会把空行写进配置 —— 环境变量另外要求键非空，只填值拼不出 KEY=VALUE。
 *
 * - `kind`: 哪一组行
 * - `index`: 行下标
 */
function dropEmptyLine(kind: "env" | "jvm" | "game", index: number) {
  if (kind === "env") {
    if (envLines.value[index]?.key.trim()) return;
    removeEnvLine(index);
    return;
  }
  if (kind === "jvm") {
    if (jvmArgLines.value[index]?.trim()) return;
    removeJvmArgLine(index);
    return;
  }
  if (gameArgLines.value[index]?.trim()) return;
  removeGameArgLine(index);
}

const { client, applyClient, launchCheck, applyLaunchCheck } = props.look;

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
    <SettingsGroup id="launchCheck" title-key="winSettings.secLaunchCheck" :flash="flashGroup === 'launchCheck'">
      <div class="switch-row">
        <div class="switch-text">
          <span class="switch-label">{{ t("winSettings.checkLoader") }}</span>
          <span class="switch-state">{{ t("winSettings.checkLoaderDesc") }}</span>
        </div>
        <BaseSwitch v-model="launchCheck.checkLoader" @update:model-value="applyLaunchCheck" />
      </div>
      <div class="switch-row">
        <div class="switch-text">
          <span class="switch-label">{{ t("winSettings.checkMemory") }}</span>
          <span class="switch-state">{{ t("winSettings.checkMemoryDesc") }}</span>
        </div>
        <BaseSwitch v-model="launchCheck.checkMemory" @update:model-value="applyLaunchCheck" />
      </div>
      <!-- 自动进入存档：属服主定制，锁定时不显示 -->
      <template v-if="!locked">
        <!-- 开了下面的"直接进入"就不再询问，这条随之禁用 -->
        <div class="switch-row" :class="{ dim: client.autoEnterSave }">
          <div class="switch-text">
            <span class="switch-label">{{ t("winSettings.autoSaveNotice") }}</span>
            <span class="switch-state">{{ t("winSettings.autoSaveNoticeDesc") }}</span>
          </div>
          <BaseSwitch v-model="client.autoSaveNotice" :disabled="client.autoEnterSave"
            @update:model-value="applyClient" />
        </div>
        <div class="switch-row">
          <div class="switch-text">
            <span class="switch-label">{{ t("winSettings.autoEnterSave") }}</span>
            <span class="switch-state">{{ t("winSettings.autoEnterSaveDesc") }}</span>
          </div>
          <BaseSwitch v-model="client.autoEnterSave" @update:model-value="applyClient" />
        </div>
      </template>
    </SettingsGroup>
    <SettingsGroup id="gameWindow" title-key="winSettings.secGameWindow" :flash="flashGroup === 'gameWindow'">
      <!-- 窗口大小：与实例设置同一套 —— 标签固定第一列，右侧下拉一行、宽 / 高一行，
           标签在两者之间垂直居中；全屏时尺寸无效，宽 / 高那行整行收起 -->
      <div class="args-row size-row">
        <span class="args-label">{{ t("args.windowSize") }}</span>
        <div class="size-controls">
          <select class="field-select pick-select res-pick" :value="resPick" @change="onResPick">
            <option v-for="r in COMMON_RESOLUTIONS" :key="`${r.w}x${r.h}`" :value="`${r.w}x${r.h}`">
              {{ r.w }}×{{ r.h }}
            </option>
            <option value="fullscreen">{{ t("args.fullscreen") }}</option>
            <option value="custom">{{ t("args.custom") }}</option>
          </select>
          <div v-if="!win!.fullScreen" class="size-sub">
            <span class="sub-tag">{{ t("args.width") }}</span>
            <NumberStepper v-model="win!.width" :min="100" :max="65535" />
            <span class="sub-tag">{{ t("args.height") }}</span>
            <NumberStepper v-model="win!.height" :min="100" :max="65535" />
          </div>
        </div>
      </div>
    </SettingsGroup>

    <SettingsGroup id="memory" title-key="winSettings.secMemory" :flash="flashGroup === 'memory'">
      <!-- 内存：最小与最大并在一行，各自预设下拉 + 数值框 + MB；窄了自动换行 -->
      <div class="args-row mem-row" :class="{ 'field-invalid': memoryConflict }">
        <span class="args-label">{{ t("args.memory") }}</span>
        <div class="mem-groups">
          <div class="mem-group">
            <span class="sub-tag">{{ t("args.minMemory") }}</span>
            <select class="field-select pick-select" :value="minMemPick" @change="onMinMemPick">
              <option v-for="m in COMMON_MIN_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
              <option value="custom">{{ t("args.custom") }}</option>
            </select>
            <NumberStepper ref="minMemoryRef" v-model="run!.minMemory" :min="256" :max="65536" :step="256" />
            <span class="mem-unit">MB</span>
          </div>
          <div class="mem-group">
            <span class="sub-tag">{{ t("args.maxMemory") }}</span>
            <select class="field-select pick-select" :value="memPick" @change="onMemPick">
              <option v-for="m in COMMON_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
              <option value="custom">{{ t("args.custom") }}</option>
            </select>
            <NumberStepper v-model="run!.maxMemory" :min="256" :max="65536" :step="256" />
            <span class="mem-unit">MB</span>
          </div>
        </div>
      </div>
      <!-- 本机内存参考值：给设最大内存一个依据（已用 / 共 · 剩余 + 左占用右剩余的条，拿不到会写明原因） -->
      <MemoryHint class="mem-line" />
      <!-- 冲突时明确说清"没保存"，避免用户以为改上了 -->
      <p v-if="memoryConflict" class="lock-server-hint">{{ t("winSettings.memoryConflict") }}</p>
    </SettingsGroup>

    <!-- JVM 与游戏参数合成一组：开关统一放最上面（与实例设置的扩展参数同一顺序），
         下面依次是 GC、环境变量、JVM 参数、游戏参数 -->
    <SettingsGroup v-if="!locked" id="jvm" title-key="winSettings.secJvmGameArgs" :flash="flashGroup === 'jvm'">
      <div class="tight-stack">
        <div class="args-row args-switch-row">
          <div class="args-switch">
            <span class="args-label" v-tip="t('winSettings.isolateHomeDesc')">{{ t("winSettings.isolateHome") }}</span>
            <BaseSwitch v-model="run!.isolateHome" />
          </div>
          <div class="args-switch">
            <span class="args-label" v-tip="t('winSettings.mmlEnvDesc')">{{ t("winSettings.mmlEnv") }}</span>
            <BaseSwitch v-model="run!.mmlEnv" />
          </div>
          <div class="args-switch">
            <span class="args-label">{{ t("winSettings.colorasm") }}</span>
            <BaseSwitch v-model="run!.colorasm" />
          </div>
          <div class="args-switch">
            <span class="args-label">{{ t("winSettings.removeJvmArg") }}</span>
            <BaseSwitch v-model="run!.removeJvmArg" />
          </div>
          <div class="args-switch">
            <span class="args-label">{{ t("winSettings.removeGameArg") }}</span>
            <BaseSwitch v-model="run!.removeGameArg" />
          </div>
        </div>

        <div class="args-row">
          <span class="args-label">{{ t("winSettings.gcMode") }}</span>
          <!-- 与实例设置的扩展参数同一副控件：下拉选择，不再用分段按钮 -->
          <select v-model="run!.gcMode" class="field-select field-grow">
            <option v-for="g in gcOptions" :key="g.value" :value="g.value">{{ g.label }}</option>
          </select>
        </div>

        <label class="args-label">{{ t("winSettings.jvmEnv") }}</label>
        <div class="dns-lines">
          <div v-for="(_, i) in envLines" :key="envKeys.keys[i]" class="line-row"
            :ref="(el) => setRowRef('env', i, el)">
            <input v-model="envLines[i].key" class="field-input env-key" spellcheck="false" autocomplete="off"
              :placeholder="t('winSettings.envKey')" @change="commitEnvLines" @blur="dropEmptyLine('env', i)" />
            <input v-model="envLines[i].value" class="field-input field-grow" spellcheck="false" autocomplete="off"
              :placeholder="t('winSettings.envValue')" />
            <button class="line-del" :aria-label="t('args.removeLine')" v-tip="t('args.removeLine')"
              @click="removeEnvLine(i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addEnv">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>
        </div>

        <label class="args-label">{{ t("winSettings.jvmArgs") }}</label>
        <div class="dns-lines">
          <div v-for="(_, i) in jvmArgLines" :key="jvmArgKeys.keys[i]" class="line-row"
            :ref="(el) => setRowRef('jvm', i, el)">
            <input v-model="jvmArgLines[i]" class="field-input field-grow" spellcheck="false" autocomplete="off"
              :placeholder="t('winSettings.jvmArgPlaceholder')" @change="commitJvmArgLines"
              @blur="dropEmptyLine('jvm', i)" />
            <button class="line-del" :aria-label="t('args.removeLine')" v-tip="t('args.removeLine')"
              @click="removeJvmArgLine(i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addJvmArg">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>
        </div>

        <label class="args-label">{{ t("winSettings.gameArgs") }}</label>
        <div class="dns-lines">
          <div v-for="(_, i) in gameArgLines" :key="gameArgKeys.keys[i]" class="line-row"
            :ref="(el) => setRowRef('game', i, el)">
            <input v-model="gameArgLines[i]" class="field-input field-grow" spellcheck="false" autocomplete="off"
              :placeholder="t('winSettings.gameArgPlaceholder')" @change="commitGameArgLines"
              @blur="dropEmptyLine('game', i)" />
            <button class="line-del" :aria-label="t('args.removeLine')" v-tip="t('args.removeLine')"
              @click="removeGameArgLine(i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addGameArg">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>
        </div>
      </div>
    </SettingsGroup>

    <SettingsGroup v-if="!locked" id="launchCmd" title-key="winSettings.secLaunchCmd"
      :flash="flashGroup === 'launchCmd'">
      <!-- 每条命令就是「开关 + 它自己的输入框」：开关决定跑不跑，输入框一直显示、只在关着时变灰 -->
      <div class="switch-row">
        <span>{{ t("winSettings.commandNotice") }}</span>
        <BaseSwitch v-model="client.commandNotice" @update:model-value="applyClient" />
      </div>
      <div class="switch-row">
        <span>{{ t("winSettings.preLaunch") }}</span>
        <BaseSwitch v-model="run!.launchPreRun" />
      </div>
      <label class="field-label">{{ t("winSettings.preCmd") }}</label>
      <textarea v-model="run!.preRunArg" class="cmd-text" rows="2" spellcheck="false"
        :disabled="!run.launchPreRun"></textarea>
      <div class="switch-row" :class="{ dim: !run.launchPreRun }">
        <span>{{ t("winSettings.preSameTime") }}</span>
        <BaseSwitch v-model="run!.preRunWithGame" :disabled="!run.launchPreRun" />
      </div>
      <div class="switch-row">
        <span>{{ t("winSettings.postLaunch") }}</span>
        <BaseSwitch v-model="run!.launchPostRun" />
      </div>
      <label class="field-label">{{ t("winSettings.postCmd") }}</label>
      <textarea v-model="run!.postRunArg" class="cmd-text" rows="2" spellcheck="false"
        :disabled="!run.launchPostRun"></textarea>
    </SettingsGroup>

  </template>
  <p v-else class="field-desc">{{ t("winSettings.tauriOnly") }}</p>
</template>

<!-- 参数行的行 / 标签 / 子标签 / 预设下拉：与实例设置（LaunchArgsPanel）共用这一份 -->
<style scoped src="../../../../styles/parts/args-row.css"></style>

<style scoped>
/* 与实例设置的「扩展参数」同一档间距：内容按 6px 竖排，
   标签 / 列表 / 添加按钮之间不再各带一套外边距（设置页 16px 的块节奏放在这一组太散） */
.tight-stack {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tight-stack>* {
  margin-top: 0;
}

/* 列布局里 flex-basis 会作用到高度：标签必须重置，否则每行标题会被撑出 84px 的空白 */
.tight-stack>.args-label {
  flex: none;
}

/* 这两条在设置页各带上边距（列表 10px、添加按钮 6px），进容器后归零 */
.tight-stack>.dns-lines,
.tight-stack .line-add {
  margin-top: 0;
}
</style>
