<script setup lang="ts">
// 设置窗口 · 游戏启动标签：游戏窗口 / 内存 / JVM / 游戏参数 / 启动命令
//
// 内存冲突即最小 > 最大时，把两个步进器标红并给出提示，保存也会被拦住，见 useSettingsLaunch；
// 冲突出现时把焦点落到"要改小"的最小内存上；NumberStepper 的输入框在组件内部，取 $el 里的 input。
import { nextTick, ref, watch } from "vue";
import { t } from "../../../../lib/i18n";
import { COMMON_MEMORY, COMMON_RESOLUTIONS } from "../../../../lib/resolutions";
import BaseSwitch from "../../../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import MemoryHint from "../../../../components/MemoryHint.vue";
import NumberStepper from "../../../../components/ui/NumberStepper.vue";
import SegmentedTabs from "../../../../components/ui/SegmentedTabs.vue";
import SettingsGroup from "../SettingsGroup.vue";
import SaveStateText from "../SaveStateText.vue";
import type { useSettingsLaunch } from "../../composables/useSettingsLaunch";

const props = defineProps<{
  /** 游戏启动标签的状态与保存方法，来自 useSettingsLaunch */
  settings: ReturnType<typeof useSettingsLaunch>;
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

/** 预设按钮上的文字：整数 GB 显示 `8 GB`，否则按 MB 显示 */
function memoryLabel(mib: number): string {
  return mib % 1024 === 0 ? `${mib / 1024} GB` : `${mib} MB`;
}

/** 应用一个内存预设：填最大内存；最小内存高于它时一并顶上去 */
function applyMemory(mib: number) {
  // run 在设置加载完成前可能为空；模板用 v-if="run && win" 挡住渲染
  if (!run.value) return;
  run.value.maxMemory = mib;
  if (run.value.minMemory > mib) run.value.minMemory = mib;
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
      <div class="switch-row">
        <span>{{ t("winSettings.fullScreen") }}</span>
        <BaseSwitch v-model="win!.fullScreen" />
      </div>
      <div class="grid-2">
        <div>
          <label class="field-label">{{ t("winSettings.width") }}</label>
          <NumberStepper v-model="win!.width" :min="100" :max="65535" />
        </div>
        <div>
          <label class="field-label">{{ t("winSettings.height") }}</label>
          <NumberStepper v-model="win!.height" :min="100" :max="65535" />
        </div>
      </div>
      <!-- 常用分辨率：一点即填宽 / 高（当前值匹配时高亮），与实例设置里那排共用同一份预设 -->
      <div class="res-row">
        <span class="res-label">{{ t("args.commonRes") }}</span>
        <button v-for="r in COMMON_RESOLUTIONS" :key="`${r.w}x${r.h}`" type="button" class="res-btn"
          :class="{ on: win!.width === r.w && win!.height === r.h }" @click="((win!.width = r.w), (win!.height = r.h))">
          {{ r.w }}×{{ r.h }}
        </button>
      </div>
    </SettingsGroup>

    <SettingsGroup id="memory" title-key="winSettings.secMemory" :flash="flashGroup === 'memory'">
      <div class="grid-2">
        <div :class="{ 'field-invalid': memoryConflict }">
          <label class="field-label">{{ t("winSettings.minMemory") }}</label>
          <NumberStepper ref="minMemoryRef" v-model="run!.minMemory" :min="256" :max="65536" :step="256" />
        </div>
        <div :class="{ 'field-invalid': memoryConflict }">
          <label class="field-label">{{ t("winSettings.maxMemory") }}</label>
          <NumberStepper v-model="run!.maxMemory" :min="256" :max="65536" :step="256" />
        </div>
      </div>
      <!-- 本机内存参考值：给设最大内存一个依据（文字 + 可用占比的条，拿不到会写明原因） -->
      <MemoryHint class="mem-hint" />
      <!-- 常用内存：一点即填「最大内存」（当前值匹配时高亮）。
           最小内存跟着顶上去，免得点了预设反而触发"最小 > 最大"的冲突 -->
      <div class="res-row">
        <span class="res-label">{{ t("args.commonMemory") }}</span>
        <button v-for="m in COMMON_MEMORY" :key="m" type="button" class="res-btn" :class="{ on: run!.maxMemory === m }"
          @click="applyMemory(m)">
          {{ memoryLabel(m) }}
        </button>
      </div>
      <!-- 冲突时明确说清"没保存"，避免用户以为改上了 -->
      <p v-if="memoryConflict" class="lock-server-hint">{{ t("winSettings.memoryConflict") }}</p>
      <div class="set-save-row right">
        <SaveStateText :state="saveState" />
      </div>
    </SettingsGroup>

    <SettingsGroup id="jvm" title-key="winSettings.secJvm" :flash="flashGroup === 'jvm'">
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

    <SettingsGroup id="gameArgs" title-key="winSettings.secGameArgs" :flash="flashGroup === 'gameArgs'">
      <div class="switch-row">
        <span>{{ t("winSettings.removeGameArg") }}</span>
        <BaseSwitch v-model="run!.removeGameArg" />
      </div>
      <label class="field-label">{{ t("winSettings.gameArgs") }}</label>
      <textarea v-model="run!.gameArgs" class="field-input args-input" spellcheck="false" />
    </SettingsGroup>

    <SettingsGroup id="launchCmd" title-key="winSettings.secLaunchCmd" :flash="flashGroup === 'launchCmd'">
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
