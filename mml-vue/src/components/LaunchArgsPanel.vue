<script setup lang="ts">
// 实例启动参数面板：
//   启动参数：窗口大小（分辨率预设 / 全屏 / 自定义）与内存（最小 / 最大）走预设下拉，
//            数值框一直可改，改出下拉里没有的值会自动落到「自定义」；
//            下拉各占一行、数值续一行；内存行带本机总量与可用量；
//            Java 选「自定义」时显示路径 + 选择文件
//   扩展参数：GC / 自定义主类 / 附加 JVM 参数 / 附加游戏参数 / 附加 classpath（多行列表）
//            / 附加环境变量（键值对列表）
import { computed, ref, watch } from "vue";
import { t } from "../lib/i18n";
import { COMMON_MEMORY, COMMON_MIN_MEMORY, COMMON_RESOLUTIONS } from "../lib/resolutions";
import type { InstanceArgsDto, JavaInfoDto } from "../lib/bindings";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import MemoryHint from "./MemoryHint.vue";
import NumberStepper from "./ui/NumberStepper.vue";
import CollapsePanel from "./ui/CollapsePanel.vue";
import { memoryLabel } from "../lib/format";

const props = defineProps<{
  /** 当前实例的启动参数 */
  args: InstanceArgsDto;
  /** 可选的 Java 列表 */
  javas: JavaInfoDto[];
  /** 实例锁定生效：只留内存 / 窗口大小 / Java，扩展参数整块隐藏 */
  locked?: boolean;
}>();

const emit = defineEmits<{
  /** 回传改动后的整份启动参数 */
  (e: "update:args", v: InstanceArgsDto): void;
}>();

const advancedOpen = ref(false);

// 预设下拉与手输是同一件事的两种口径：**下拉**的手输态由「用户刚选过自定义」或「当前值不在预设里」
// 决定。选了自定义但值恰好等于某档预设时（如从 1920×1080 起手微调），必须靠前者才停得住
const resManual = ref(false);
const memManual = ref(false);
const minMemManual = ref(false);

/** 窗口大小下拉的当前项：全屏 / 预设的 `宽x高` / `custom`，三者互斥 */
const resPick = computed(() => {
  if (props.args.fullscreen) return "fullscreen";
  const hit = COMMON_RESOLUTIONS.find((r) => r.w === props.args.width && r.h === props.args.height);
  return !resManual.value && hit ? `${hit.w}x${hit.h}` : "custom";
});

/** 是否展开宽 / 高手输：非全屏一直可改（照着预设微调就不必先点一次「自定义」），全屏时尺寸无效 */
const resInputs = computed(() => !props.args.fullscreen);

/** 最大内存下拉的当前项：预设的 MB 字符串，手输或没匹配上预设都是 `custom` */
const memPick = computed(() =>
  !memManual.value && COMMON_MEMORY.includes(props.args.memory) ? String(props.args.memory) : "custom",
);

/** 最小内存下拉的当前项：口径与最大内存相同，预设表多出 256 / 512 两档 */
const minMemPick = computed(() =>
  !minMemManual.value && COMMON_MIN_MEMORY.includes(props.args.minMemory) ? String(props.args.minMemory) : "custom",
);

/** 当前选的 Java 不在候选列表里（被删了 / 还没扫到）：下拉要补一条同名的，否则显示成空白 */
const missingJava = computed(
  () =>
    !!props.args.javaName &&
    props.args.javaName !== "custom" &&
    !props.javas.some((j) => j.name === props.args.javaName),
);

// 只在值真的变了时退出手输态：切实例 / 重置同样落回预设显示，
// 而改别的字段（GC、扩展参数）会把整份 args 换新对象，那时不能把正在手输的框收走
watch(() => `${props.args.width}x${props.args.height}`, () => {
  resManual.value = false;
});
watch(() => props.args.memory, () => {
  memManual.value = false;
});
watch(() => props.args.minMemory, () => {
  minMemManual.value = false;
});

/** 应用一个内存预设：填最大内存；最小内存高于它时一并顶上去 */
function applyMemory(mib: number) {
  update({ memory: mib, minMemory: Math.min(props.args.minMemory, mib) });
}

/** 分辨率下拉：全屏 / 预设 / 自定义 三选一；后两者都要退出全屏，否则选项与状态对不上 */
function onResPick(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  if (v === "fullscreen") {
    resManual.value = false;
    update({ fullscreen: true });
    return;
  }
  if (v === "custom") {
    resManual.value = true;
    update({ fullscreen: false });
    return;
  }
  const [w, h] = v.split("x").map(Number);
  resManual.value = false;
  update({ width: w, height: h, fullscreen: false });
}

/** 最小内存下拉：选预设就填最小内存；预设高于当前上限时压到上限，与上限侧的夹取对称 */
function onMinMemPick(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  if (v === "custom") {
    minMemManual.value = true;
    return;
  }
  minMemManual.value = false;
  update({ minMemory: Math.min(Number(v), props.args.memory) });
}

/** 内存下拉：与上面同一口径，选预设就是应用一个最大内存预设 */
function onMemPick(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  if (v === "custom") {
    memManual.value = true;
    return;
  }
  memManual.value = false;
  applyMemory(Number(v));
}

function update(patch: Partial<InstanceArgsDto>) {
  emit("update:args", { ...props.args, ...patch });
}

/**
 * 选自定义 Java 的可执行文件
 *
 * 必须走 Tauri 对话框：浏览器 `<input type="file">` 拿不到完整路径，
 * 后端拼启动命令时只拿到一个文件名，等于不可用。
 */
async function pickJava() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("args.customJava"),
      multiple: false,
      directory: false,
    });
    if (typeof picked === "string" && picked) {
      update({ javaPath: picked });
    }
  } catch {
    // 浏览器预览没有对话框：保留手动输入路径
  }
}

// ---- 多行字符串参数 ----
type LineKey = "jvmArgs" | "gameArgs" | "classPath";

function setLine(key: LineKey, idx: number, value: string) {
  const arr = [...props.args[key]];
  arr[idx] = value;
  update({ [key]: arr } as Partial<InstanceArgsDto>);
}

function removeLine(key: LineKey, idx: number) {
  const arr = [...props.args[key]];
  arr.splice(idx, 1);
  update({ [key]: arr } as Partial<InstanceArgsDto>);
}

function addLine(key: LineKey) {
  update({ [key]: [...props.args[key], ""] } as Partial<InstanceArgsDto>);
}

// ---- 环境变量（键值对） ----
function setEnvKey(idx: number, value: string) {
  const arr = props.args.envVars.map((l, i) => (i === idx ? { ...l, key: value } : l));
  update({ envVars: arr });
}

function setEnvValue(idx: number, value: string) {
  const arr = props.args.envVars.map((l, i) => (i === idx ? { ...l, value } : l));
  update({ envVars: arr });
}

function removeEnv(idx: number) {
  const arr = [...props.args.envVars];
  arr.splice(idx, 1);
  update({ envVars: arr });
}

function addEnv() {
  update({ envVars: [...props.args.envVars, { key: "", value: "" }] });
}

// 放进 computed：切换语言时下拉文案要跟着变，setup 里取一次会停在旧语言
const gcOptions = computed(() => [
  { value: "auto", label: t("args.gcAuto") },
  { value: "g1gc", label: t("args.gcG1") },
  { value: "zgc", label: t("args.gcZ") },
  { value: "none", label: t("args.gcNone") },
  { value: "custom", label: t("args.gcCustom") },
]);
</script>

<template>
  <div class="args-panel">
    <!-- 启动参数 -->
    <div class="args-block">
      <!-- 窗口大小：第一行是预设下拉（含全屏），第二行是宽 / 高；改成下拉里没有的值会自动
           落到「自定义」。全屏时尺寸无效，第二行整行收起 -->
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
      <div v-if="resInputs" class="args-row sub-row">
        <span class="sub-tag">{{ t("args.width") }}</span>
        <NumberStepper :model-value="args.width" :min="0" :max="9999" :step="10"
          @update:model-value="(v: number) => update({ width: v })" />
        <span class="sub-tag">{{ t("args.height") }}</span>
        <NumberStepper :model-value="args.height" :min="0" :max="9999" :step="10"
          @update:model-value="(v: number) => update({ height: v })" />
      </div>

      <!-- 内存：最小一行、最大续一行，各自预设下拉 + 数值框，口径与窗口大小相同 -->
      <div class="args-row">
        <span class="args-label">{{ t("args.memory") }}</span>
        <span class="sub-tag">{{ t("args.minMemory") }}</span>
        <select class="field-select pick-select" :value="minMemPick" @change="onMinMemPick">
          <option v-for="m in COMMON_MIN_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
          <option value="custom">{{ t("args.custom") }}</option>
        </select>
        <NumberStepper :model-value="args.minMemory" :min="256" :max="args.memory" :step="256"
          @update:model-value="(v: number) => update({ minMemory: v })" />
        <span class="mem-unit">MB</span>
      </div>
      <div class="args-row sub-row">
        <span class="sub-tag">{{ t("args.maxMemory") }}</span>
        <select class="field-select pick-select" :value="memPick" @change="onMemPick">
          <option v-for="m in COMMON_MEMORY" :key="m" :value="String(m)">{{ memoryLabel(m) }}</option>
          <option value="custom">{{ t("args.custom") }}</option>
        </select>
        <NumberStepper :model-value="args.memory" :min="args.minMemory" :max="16384" :step="256"
          @update:model-value="(v: number) => update({ memory: v })" />
        <span class="mem-unit">MB</span>
      </div>
      <!-- 本机内存参考值：给设最大内存一个依据（已用 / 共 · 剩余 + 左占用右剩余的条，拿不到会写明原因） -->
      <MemoryHint class="mem-line" />

      <div class="args-row">
        <span class="args-label">{{ t("args.java") }}</span>
        <!-- 自动 = jvm_name 留空，启动时按版本要求的 Java 主版本挑（见 core 的取 Java 逻辑） -->
        <select class="field-select grow" :value="args.javaName"
          @change="update({ javaName: ($event.target as HTMLSelectElement).value })">
          <option value="">{{ t("args.javaAuto") }}</option>
          <option v-for="j in javas" :key="j.name" :value="j.name">
            {{ t("args.javaEntry", { name: j.name, major: j.major }) }}
          </option>
          <!-- 选中的 Java 已不在列表里（删了 / 还没扫到）：补一条同名的，免得下拉一片空白 -->
          <option v-if="missingJava" :value="args.javaName">{{ args.javaName }}</option>
          <option value="custom">{{ t("args.customJava") }}</option>
        </select>
      </div>

      <!-- 自定义 Java 路径 -->
      <div v-if="args.javaName === 'custom'" class="args-row">
        <span class="args-label">{{ t("args.javaPath") }}</span>
        <input class="field-input grow" :value="args.javaPath" placeholder="C:\Program Files\Java\jdk-21\bin\java.exe"
          spellcheck="false" @input="update({ javaPath: ($event.target as HTMLInputElement).value })" />
        <BaseButton size="sm" @click="pickJava">{{ t("args.browse") }}</BaseButton>
      </div>
    </div>

    <!-- 扩展参数：实例锁定时整块隐藏（含标题行与分隔线），只留上面的三项 -->
    <div v-if="!locked" class="args-block advanced">
      <button class="advanced-toggle" @click="advancedOpen = !advancedOpen">
        <span>{{ t("args.advanced") }}</span>
        <svg class="args-chevron" :class="{ flip: advancedOpen }" viewBox="0 0 24 24" width="13" height="13" fill="none"
          stroke="currentColor" stroke-width="2">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      <CollapsePanel :open="advancedOpen">
        <div class="advanced-body">
          <div class="args-row">
            <span class="args-label">{{ t("args.gc") }}</span>
            <select class="field-select grow" :value="args.gc"
              @change="update({ gc: ($event.target as HTMLSelectElement).value })">
              <option v-for="g in gcOptions" :key="g.value" :value="g.value">{{ g.label }}</option>
            </select>
          </div>

          <!-- 自定义 GC 参数 -->
          <div v-if="args.gc === 'custom'" class="args-row">
            <span class="args-label">{{ t("args.gcCustom") }}</span>
            <input class="field-input grow" :value="args.gcCustom" placeholder="-XX:+UseZGC -XX:ZCollectionInterval=30"
              spellcheck="false" @input="update({ gcCustom: ($event.target as HTMLInputElement).value })" />
          </div>

          <div class="args-row">
            <span class="args-label">{{ t("args.mainClass") }}</span>
            <input class="field-input grow" :value="args.mainClass" placeholder="net.minecraft.client.main.Main"
              spellcheck="false" @input="update({ mainClass: ($event.target as HTMLInputElement).value })" />
          </div>

          <!-- 附加 JVM 参数 -->
          <label class="args-label">{{ t("args.jvmExtra") }}</label>
          <div v-for="(line, i) in args.jvmArgs" :key="i" class="line-row">
            <input class="field-input grow" :value="line" placeholder="-XX:+UseG1GC" spellcheck="false"
              @input="setLine('jvmArgs', i, ($event.target as HTMLInputElement).value)" />
            <button class="line-del" v-tip="t('args.removeLine')" @click="removeLine('jvmArgs', i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addLine('jvmArgs')">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>

          <!-- 附加游戏参数 -->
          <label class="args-label">{{ t("args.gameExtra") }}</label>
          <div v-for="(line, i) in args.gameArgs" :key="i" class="line-row">
            <input class="field-input grow" :value="line" placeholder="--server 127.0.0.1:25565" spellcheck="false"
              @input="setLine('gameArgs', i, ($event.target as HTMLInputElement).value)" />
            <button class="line-del" v-tip="t('args.removeLine')" @click="removeLine('gameArgs', i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addLine('gameArgs')">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>

          <!-- 附加 classpath -->
          <label class="args-label">{{ t("args.classPath") }}</label>
          <div v-for="(line, i) in args.classPath" :key="i" class="line-row">
            <input class="field-input grow" :value="line" placeholder="libraries/xxx.jar" spellcheck="false"
              @input="setLine('classPath', i, ($event.target as HTMLInputElement).value)" />
            <button class="line-del" v-tip="t('args.removeLine')" @click="removeLine('classPath', i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addLine('classPath')">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>

          <!-- 隔离用户目录：HOME / APPDATA / XDG 等指到实例目录 -->
          <div class="args-row">
            <span class="args-label" v-tip="t('args.isolateHomeDesc')">{{ t("args.isolateHome") }}</span>
            <BaseSwitch :model-value="args.isolateHome"
              @update:model-value="update({ isolateHome: $event })" />
          </div>

          <!-- 附加环境变量（键值对） -->
          <label class="args-label">{{ t("args.env") }}</label>
          <div v-for="(line, i) in args.envVars" :key="i" class="line-row">
            <input class="field-input env-key" :value="line.key" :placeholder="t('args.envKey')" spellcheck="false"
              @input="setEnvKey(i, ($event.target as HTMLInputElement).value)" />
            <input class="field-input grow" :value="line.value" :placeholder="t('args.envValue')" spellcheck="false"
              @input="setEnvValue(i, ($event.target as HTMLInputElement).value)" />
            <button class="line-del" v-tip="t('args.removeLine')" @click="removeEnv(i)">
              <GlyphIcon name="close" :size="13" />
            </button>
          </div>
          <button class="line-add" @click="addEnv">
            <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("args.addLine") }}
          </button>
        </div>
      </CollapsePanel>
    </div>
  </div>
</template>

<style scoped>
.args-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
}

.args-block {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.advanced {
  /* 这块只有「折叠标题 + 面板」两个子项，间距不能走 gap：
     收起时 CollapsePanel 高度为 0，gap 照样占 10px，标题下面就多出一截空白。
     改成展开时才给上边距（与主窗口那三块折叠区同一个坑） */
  gap: 0;
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.advanced :deep(.collapse.open) {
  margin-top: 10px;
}

/* 扩展参数区内的独立标题（列布局）：flex-basis 会作用到高度，需重置，避免 84px 大空白 */
.advanced-body>.args-label {
  flex: none;
}

.grow {
  flex: 1;
  min-width: 120px;
}

.advanced-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  padding: 4px 0;
}

.advanced-toggle:hover {
  color: var(--text);
}

.advanced-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.args-chevron {
  transition: transform 0.15s;
}

.args-chevron.flip {
  transform: rotate(180deg);
}

/* 多行参数 */
.line-row {
  display: flex;
  align-items: center;
  /* 与 .args-row 同一档：同一个卡片里不该有两种行内间距 */
  gap: 12px;
}

.env-key {
  flex: 0 0 140px;
}

.line-del {
  width: var(--ctl-h);
  height: var(--ctl-h);
  /* 图标是 SVG，用 flex 精确居中 */
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--ctl-radius);
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.12s;
}

.line-del:hover {
  color: var(--red);
  border-color: var(--red);
  background: color-mix(in srgb, var(--red) 10%, transparent);
}

.line-add {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  /* 图标是 SVG，与文字用 gap 隔开 */
  gap: 6px;
  height: var(--ctl-h);
  padding: 0 22px;
  border-radius: var(--ctl-radius);
  border: 1px dashed var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
  font-size: var(--ctl-font);
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.12s;
}

.line-add:hover {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}
</style>

<!-- 参数行的行 / 标签 / 子标签 / 预设下拉：与设置窗口的「游戏启动」共用这一份 -->
<style scoped src="../styles/parts/args-row.css"></style>
