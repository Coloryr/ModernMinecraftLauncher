<script setup lang="ts">
// 使用调试器启动：JVM 远程调试（JDWP）与渲染调试（RenderDoc / Nsight Graphics）两个开关。
//
// 「生成调试参数」不启动游戏，只给出调试器界面要填的四项；「使用 xxx」才真的按当前开关拉起。
// 生成结果含本次登录令牌，所以只在这个弹窗里给复制，不写进日志。
import { computed, ref, watch } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import { api } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { copyText } from "../../lib/clipboard";
import type { DebugLaunchDto } from "../../lib/bindings";

const props = defineProps<{
  /** 是否显示 */
  open: boolean;
  /** 选中的实例 uuid，生成参数要用 */
  uuid: string;
  /** 本机装没装 Nsight Graphics；装了才多出这个渲染调试选项 */
  nsight: boolean;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 按当前开关启动：`kind` 为 null 表示不做渲染调试 */
  (e: "launch", payload: { kind: "renderdoc" | "nsight" | null; jvmDebug: number | null }): void;
}>();

/** 当前步骤：选参数 → 生成中 → 展示结果 */
const step = ref<"choice" | "loading" | "result">("choice");

/** JVM 远程调试开关与端口；端口默认 JDWP 的惯例值 */
const jvmOn = ref(false);
const jvmPort = ref("5005");

/** 渲染调试开关与选中的调试器 */
const renderOn = ref(true);
const picked = ref<"renderdoc" | "nsight">("renderdoc");

/** 生成出来的四项 */
const info = ref<DebugLaunchDto | null>(null);

/** 生成失败的文案，显示在选项下面 */
const error = ref("");

/** 刚复制过的那一项，只用来短暂改按钮文字 */
const copied = ref("");

// 每次打开都回到默认状态：上一次生成的结果带着令牌，不能留着
watch(
  () => props.open,
  (open) => {
    if (!open) return;
    step.value = "choice";
    jvmOn.value = false;
    jvmPort.value = "5005";
    renderOn.value = true;
    picked.value = "renderdoc";
    info.value = null;
    error.value = "";
    copied.value = "";
  },
);

/** 端口号；没开 JVM 调试或不合法时为 null */
const jvmDebug = computed(() => {
  if (!jvmOn.value) return null;
  const port = Number(jvmPort.value.trim());
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  return port;
});

/** 端口格式对不对；没开 JVM 调试时不用校验 */
const portValid = computed(() => !jvmOn.value || jvmDebug.value !== null);

/** 主按钮文案：跟着渲染调试的选择走 */
const launchLabel = computed(() => {
  if (!renderOn.value) return t("debugger.launchPlain");
  return picked.value === "nsight" ? t("debugger.useNsight") : t("debugger.useRenderdoc");
});

/** 渲染调试的说明行 */
const renderHint = computed(() =>
  picked.value === "nsight" ? t("debugger.nsightHint") : t("debugger.renderdocHint"),
);

/** 四项参数的展示顺序；环境变量为空时给一行占位 */
const fields = computed(() => {
  const data = info.value;
  if (!data) return [];
  return [
    { key: "executable", label: t("debugger.executable"), value: data.executable },
    { key: "workingDir", label: t("debugger.workingDir"), value: data.workingDir },
    { key: "arguments", label: t("debugger.arguments"), value: data.arguments },
    {
      key: "environment",
      label: t("debugger.environment"),
      value: data.environment.length ? data.environment.join("\n") : t("debugger.emptyEnv"),
    },
  ];
});

/** 整份复制：带上字段名，粘到哪里都看得出哪行是什么 */
const allText = computed(() =>
  fields.value.map((item) => `${item.label}:\n${item.value}`).join("\n\n"),
);

/**
 * 复制一项
 *
 * - `value`: 要复制的文本
 * - `key`: 这一项的标识，用来把按钮短暂改成「已复制」
 */
async function copy(value: string, key: string) {
  if (!(await copyText(value))) return;
  copied.value = key;
  window.setTimeout(() => {
    if (copied.value === key) copied.value = "";
  }, 1500);
}

/** 按当前开关启动，交给主窗口走正常启动流程 */
function onLaunch() {
  if (!portValid.value) return;
  emit("launch", {
    kind: renderOn.value ? picked.value : null,
    jvmDebug: jvmDebug.value,
  });
  emit("close");
}

/** 选「生成调试参数」：走一次真实启动的参数生成，只是不拉起进程 */
async function generate() {
  if (!portValid.value) return;
  step.value = "loading";
  error.value = "";
  try {
    info.value = await api.debugLaunchInfo(props.uuid, jvmDebug.value ?? undefined);
    step.value = "result";
  } catch (e) {
    error.value = tErr(e);
    step.value = "choice";
  }
}
</script>

<template>
  <BaseModal v-if="open" :title="t('debugger.title')" :width="560" @close="emit('close')">
    <template v-if="step === 'choice'">
      <p class="modal-text">{{ t("debugger.desc") }}</p>

      <!-- 第一行：JVM 远程调试，开了才让填端口 -->
      <div class="dbg-row">
        <div class="dbg-row-head">
          <BaseSwitch v-model="jvmOn" />
          <span class="dbg-row-label">{{ t("debugger.jvm") }}</span>
        </div>
        <input v-if="jvmOn" v-model="jvmPort" class="field-input dbg-port" inputmode="numeric"
          maxlength="5" :placeholder="t('debugger.jvmPort')" :aria-label="t('debugger.jvmPort')" />
      </div>
      <p v-if="jvmOn" class="dbg-hint">{{ t("debugger.jvmHint") }}</p>
      <p v-if="jvmOn && !portValid" class="dbg-error">{{ t("debugger.portInvalid") }}</p>

      <!-- 第二行：渲染调试，开了才让选调试器 -->
      <div class="dbg-row">
        <div class="dbg-row-head">
          <BaseSwitch v-model="renderOn" />
          <span class="dbg-row-label">{{ t("debugger.render") }}</span>
        </div>
        <select v-if="renderOn" v-model="picked" class="field-select dbg-select">
          <option value="renderdoc">RenderDoc</option>
          <option v-if="nsight" value="nsight">Nsight Graphics</option>
        </select>
      </div>
      <p v-if="renderOn" class="dbg-hint">{{ renderHint }}</p>

      <p v-if="error" class="dbg-error">{{ t("debugger.genFailed", { msg: error }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="emit('close')">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton :disabled="!portValid" @click="generate">{{ t("debugger.gen") }}</BaseButton>
        <BaseButton variant="primary" :disabled="!portValid" @click="onLaunch">{{ launchLabel }}</BaseButton>
      </div>
    </template>

    <template v-else-if="step === 'loading'">
      <p class="modal-text">{{ t("debugger.generating") }}</p>
      <div class="modal-actions">
        <BaseButton @click="step = 'choice'">{{ t("debugger.back") }}</BaseButton>
      </div>
    </template>

    <template v-else>
      <p class="modal-text">{{ t("debugger.tokenHint") }}</p>
      <div v-for="item in fields" :key="item.key" class="dbg-field">
        <div class="dbg-field-head">
          <span class="field-label">{{ item.label }}</span>
          <BaseButton size="sm" variant="ghost" @click="copy(item.value, item.key)">
            {{ copied === item.key ? t("debugger.copied") : t("debugger.copy") }}
          </BaseButton>
        </div>
        <pre class="dbg-value">{{ item.value }}</pre>
      </div>
      <div class="modal-actions">
        <BaseButton @click="step = 'choice'">{{ t("debugger.back") }}</BaseButton>
        <BaseButton variant="primary" @click="copy(allText, 'all')">
          {{ copied === "all" ? t("debugger.copied") : t("debugger.copyAll") }}
        </BaseButton>
      </div>
    </template>
  </BaseModal>
</template>

<style scoped>
.dbg-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
}

.dbg-row-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dbg-row-label {
  font-size: 13px;
}

.dbg-port {
  width: 88px;
}

.dbg-select {
  width: 168px;
}

.dbg-hint {
  margin: 4px 0 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

.dbg-error {
  margin: 4px 0 0;
  color: var(--red);
  font-size: 11.5px;
}

.dbg-field {
  margin-top: 12px;
}

.dbg-field-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

/* 值按原样展示：等宽字、保留换行，长了在框内滚动，不把弹窗撑宽 */
.dbg-value {
  max-height: 120px;
  margin: 0;
  padding: 8px 10px;
  overflow: auto;
  border-radius: 8px;
  background: var(--bg-raised, var(--bg-modal-raised));
  color: var(--text);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
