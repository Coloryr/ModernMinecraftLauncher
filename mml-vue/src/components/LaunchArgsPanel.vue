<script setup lang="ts">
// 实例启动参数面板：
//   启动参数：内存大小，最小 / 最大加右侧输入并显示本机总量与可用量；窗口大小带常用分辨率
//            / 使用的 Java，自定义时显示路径 + 选择文件
//   扩展参数：GC / 自定义主类 / 附加 JVM 参数 / 附加游戏参数 / 附加 classpath（多行列表）
//            / 附加环境变量（键值对列表）
import { ref } from "vue";
import { t } from "../lib/i18n";
import { COMMON_MEMORY, COMMON_RESOLUTIONS } from "../lib/resolutions";
import { useSystemMemory } from "../lib/systemMemory";
import type { InstanceArgsDto, JavaInfoDto } from "../lib/bindings";
import BaseButton from "./ui/BaseButton.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import NumberStepper from "./ui/NumberStepper.vue";
import CollapsePanel from "./ui/CollapsePanel.vue";

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
const fileInput = ref<HTMLInputElement | null>(null);

/** 本机内存参考值，与设置窗口的「游戏启动」共用同一份逻辑 */
const { text: memoryText } = useSystemMemory();

function applyResolution(w: number, h: number) {
  update({ width: w, height: h });
}

/** 预设按钮上的文字：整数 GB 显示 `8 GB`，否则按 MB 显示 */
function memoryLabel(mib: number): string {
  return mib % 1024 === 0 ? `${mib / 1024} GB` : `${mib} MB`;
}

/** 应用一个内存预设：填最大内存；最小内存高于它时一并顶上去 */
function applyMemory(mib: number) {
  update({ memory: mib, minMemory: Math.min(props.args.minMemory, mib) });
}

function update(patch: Partial<InstanceArgsDto>) {
  emit("update:args", { ...props.args, ...patch });
}

function onBrowseFile(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) {
    // 浏览器 input 只能拿到 fakepath；待改用 @tauri-apps/plugin-dialog，settings / add 窗口已在用
    update({ javaPath: file.name });
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

const gcOptions = [
  { value: "auto", label: t("args.gcAuto") },
  { value: "g1gc", label: t("args.gcG1") },
  { value: "zgc", label: t("args.gcZ") },
  { value: "none", label: t("args.gcNone") },
  { value: "custom", label: t("args.gcCustom") },
];
</script>

<template>
  <div class="args-panel">
    <!-- 启动参数 -->
    <div class="args-block">
      <!-- 窗口大小：宽 / 高 + 全屏（与内存行对齐） -->
      <div class="args-row">
        <span class="args-label">{{ t("args.windowSize") }}</span>
        <span class="sub-tag">{{ t("args.width") }}</span>
        <NumberStepper :model-value="args.width" :min="0" :max="9999" :step="10"
          @update:model-value="(v: number) => update({ width: v })" />
        <span class="sub-tag">{{ t("args.height") }}</span>
        <NumberStepper :model-value="args.height" :min="0" :max="9999" :step="10"
          @update:model-value="(v: number) => update({ height: v })" />
        <label class="chk">
          <input type="checkbox" :checked="args.fullscreen"
            @change="update({ fullscreen: ($event.target as HTMLInputElement).checked })" />
          {{ t("args.fullscreen") }}
        </label>
      </div>
      <!-- 常用分辨率：一点即填宽 / 高（当前值匹配时高亮） -->
      <div class="args-hint res-hint">
        <span class="res-label">{{ t("args.commonRes") }}</span>
        <button v-for="r in COMMON_RESOLUTIONS" :key="`${r.w}x${r.h}`" type="button" class="res-btn"
          :class="{ on: args.width === r.w && args.height === r.h }" @click="applyResolution(r.w, r.h)">
          {{ r.w }}×{{ r.h }}
        </button>
      </div>

      <!-- 内存：最小 / 最大（输入框，与窗口行对齐） -->
      <div class="args-row">
        <span class="args-label">{{ t("args.memory") }}</span>
        <span class="sub-tag">{{ t("args.minMemory") }}</span>
        <NumberStepper :model-value="args.minMemory" :min="512" :max="args.memory" :step="256"
          @update:model-value="(v: number) => update({ minMemory: v })" />
        <span class="sub-tag">{{ t("args.maxMemory") }}</span>
        <NumberStepper :model-value="args.memory" :min="args.minMemory" :max="16384" :step="256"
          @update:model-value="(v: number) => update({ memory: v })" />
        <span class="mem-unit">MB</span>
      </div>
      <!-- 本机内存参考值：给设最大内存一个依据（始终占一行，拿不到会写明原因） -->
      <div class="args-hint mem-hint">{{ memoryText }}</div>
      <!-- 常用内存：一点即填最大内存（当前值匹配时高亮），最小内存高于它时一并顶上去 -->
      <div class="args-hint">
        <span class="res-label">{{ t("args.commonMemory") }}</span>
        <button v-for="m in COMMON_MEMORY" :key="m" type="button" class="res-btn" :class="{ on: args.memory === m }"
          @click="applyMemory(m)">
          {{ memoryLabel(m) }}
        </button>
      </div>

      <div class="args-row">
        <span class="args-label">{{ t("args.java") }}</span>
        <select class="field-select grow" :value="args.javaName"
          @change="update({ javaName: ($event.target as HTMLSelectElement).value })">
          <option v-for="j in javas" :key="j.name" :value="j.name">
            {{ j.name }}（Java {{ j.major }}）
          </option>
          <option value="custom">{{ t("args.customJava") }}</option>
        </select>
      </div>

      <!-- 自定义 Java 路径 -->
      <div v-if="args.javaName === 'custom'" class="args-row java-custom">
        <span class="args-label">{{ t("args.javaPath") }}</span>
        <input class="field-input grow" :value="args.javaPath" placeholder="C:\Program Files\Java\jdk-21\bin\java.exe"
          spellcheck="false" @input="update({ javaPath: ($event.target as HTMLInputElement).value })" />
        <BaseButton size="sm" @click="fileInput?.click()">{{ t("args.browse") }}</BaseButton>
        <input ref="fileInput" type="file" class="hidden-file" @change="onBrowseFile" />
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
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.args-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.args-label {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-dim);
  flex: 0 0 84px;
}

/* 扩展参数区内的独立标题（列布局）：flex-basis 会作用到高度，需重置，避免 84px 大空白 */
.advanced-body>.args-label {
  flex: none;
}

/* 与内存行对齐用的小标签（最小/最大/宽/高） */
.sub-tag {
  flex: 0 0 28px;
  font-size: 12px;
  color: var(--text-dim);
  white-space: nowrap;
  text-align: right;
}

/* 字段下方的辅助行（内存参考值 / 常用分辨率），与 .args-label 的 84px 缩进对齐 */
.args-hint {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 6px;
  padding-left: 84px;
  font-size: 12px;
  color: var(--text-dim);
}

.mem-hint {
  font-variant-numeric: tabular-nums;
}

.res-label {
  flex: 0 0 auto;
}

/* 常用分辨率：小方块按钮，当前值匹配时高亮 */
.res-btn {
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-dim);
  font-family: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  padding: 3px 9px;
  border-radius: 7px;
  cursor: pointer;
}

.res-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.res-btn.on {
  border-color: var(--accent);
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.grow {
  flex: 1;
  min-width: 120px;
}

.mem-unit {
  font-size: 12px;
  color: var(--text-dim);
  white-space: nowrap;
}

.chk {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text);
  cursor: pointer;
  accent-color: var(--accent);
  margin-left: 6px;
  white-space: nowrap;
}

.java-custom {
  padding-left: 96px;
}

.hidden-file {
  display: none;
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
  gap: 8px;
}

.env-key {
  flex: 0 0 140px;
}

.line-del {
  width: 28px;
  height: 28px;
  /* 图标是 SVG：用 flex 精确居中（原来靠字形在行盒里居中） */
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.12s;
}

.line-del:hover {
  color: var(--red);
  border-color: var(--red);
  background: rgba(255, 95, 86, 0.1);
}

.line-add {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  /* 图标是 SVG（原来是字形，靠一个空格隔开），间距改由 gap 给 */
  gap: 6px;
  height: 28px;
  padding: 0 22px;
  border-radius: 9px;
  border: 1px dashed var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
  font-size: 13.5px;
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
