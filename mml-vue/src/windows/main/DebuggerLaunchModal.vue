<script setup lang="ts">
// 启动前的图形调试器选择：
// - 本机装了 Nsight Graphics 时，把「从调试器启动」拆成 RenderDoc / Nsight 两个子选项；
// - 没装就还是原来的 renderdoccmd 一项；
// - 另有一项「生成启动指令」，不启动游戏，只给出调试器界面要填的四项。
//
// 生成结果里带本次登录令牌，所以只在这个弹窗里给复制，不写进日志。
import { computed, ref, watch } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import { api } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { copyText } from "../../lib/clipboard";
import type { RenderDocLaunchDto } from "../../lib/bindings";

const props = defineProps<{
  /** 是否显示 */
  open: boolean;
  /** 选中的实例 uuid，生成参数要用 */
  uuid: string;
  /** 本机装没装 Nsight Graphics；装了才多出那个子选项 */
  nsight: boolean;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 用选中的调试器启动游戏 */
  (e: "launch", kind: "renderdoc" | "nsight"): void;
}>();

/** 当前步骤：选方式 → 生成中 → 展示结果 */
const step = ref<"choice" | "loading" | "result">("choice");

/** 生成出来的四项 */
const info = ref<RenderDocLaunchDto | null>(null);

/** 生成失败的文案，显示在选项下面 */
const error = ref("");

/** 刚复制过的那一项，只用来短暂改按钮文字 */
const copied = ref("");

// 每次打开都从选择开始：上一次生成的结果带着令牌，不能留着
watch(
  () => props.open,
  (open) => {
    if (!open) return;
    step.value = "choice";
    info.value = null;
    error.value = "";
    copied.value = "";
  },
);

/** 四项参数的展示顺序；环境变量为空时给一行占位 */
const fields = computed(() => {
  const data = info.value;
  if (!data) return [];
  return [
    { key: "executable", label: t("renderdoc.executable"), value: data.executable },
    { key: "workingDir", label: t("renderdoc.workingDir"), value: data.workingDir },
    { key: "arguments", label: t("renderdoc.arguments"), value: data.arguments },
    {
      key: "environment",
      label: t("renderdoc.environment"),
      value: data.environment.length ? data.environment.join("\n") : t("renderdoc.emptyEnv"),
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

/**
 * 选某个调试器启动，交给主窗口走正常启动流程
 *
 * - `kind`: renderdoc 或 nsight
 */
function useDebugger(kind: "renderdoc" | "nsight") {
  emit("launch", kind);
  emit("close");
}

/** 选「生成启动指令」：走一次真实启动的参数生成，只是不拉起进程 */
async function generate() {
  step.value = "loading";
  error.value = "";
  try {
    info.value = await api.renderdocLaunchInfo(props.uuid);
    step.value = "result";
  } catch (e) {
    error.value = tErr(e);
    step.value = "choice";
  }
}
</script>

<template>
  <BaseModal v-if="open" :title="nsight ? t('debugger.title') : t('renderdoc.title')" :width="560"
    @close="emit('close')">
    <template v-if="step === 'choice'">
      <p class="modal-text">{{ nsight ? t("debugger.desc") : t("renderdoc.desc") }}</p>

      <!-- 装了 Nsight：两种调试器收在「从调试器启动」下面 -->
      <div v-if="nsight" class="rd-group">
        <span class="rd-group-title">{{ t("debugger.group") }}</span>
        <BaseButton variant="primary" block class="rd-choice" @click="useDebugger('renderdoc')">
          <span class="rd-choice-title">RenderDoc</span>
          <span class="rd-choice-hint">{{ t("debugger.renderdocHint") }}</span>
        </BaseButton>
        <BaseButton variant="primary" block class="rd-choice" @click="useDebugger('nsight')">
          <span class="rd-choice-title">Nsight Graphics</span>
          <span class="rd-choice-hint">{{ t("debugger.nsightHint") }}</span>
        </BaseButton>
      </div>

      <!-- 没装：保持原来的一项 -->
      <div v-else class="rd-choices">
        <BaseButton variant="primary" block class="rd-choice" @click="useDebugger('renderdoc')">
          <span class="rd-choice-title">{{ t("renderdoc.useCmd") }}</span>
          <span class="rd-choice-hint">{{ t("renderdoc.useCmdHint") }}</span>
        </BaseButton>
      </div>

      <BaseButton variant="plain" block class="rd-choice rd-generate" @click="generate">
        <span class="rd-choice-title">{{ t("renderdoc.genInfo") }}</span>
        <span class="rd-choice-hint">{{ t("renderdoc.genInfoHint") }}</span>
      </BaseButton>

      <p v-if="error" class="rd-error">{{ t("renderdoc.genFailed", { msg: error }) }}</p>
      <div class="modal-actions">
        <BaseButton @click="emit('close')">{{ t("actions.cancel") }}</BaseButton>
      </div>
    </template>

    <template v-else-if="step === 'loading'">
      <p class="modal-text">{{ t("renderdoc.generating") }}</p>
      <div class="modal-actions">
        <BaseButton @click="step = 'choice'">{{ t("renderdoc.back") }}</BaseButton>
      </div>
    </template>

    <template v-else>
      <p class="modal-text">{{ t("renderdoc.tokenHint") }}</p>
      <div v-for="item in fields" :key="item.key" class="rd-field">
        <div class="rd-field-head">
          <span class="field-label">{{ item.label }}</span>
          <BaseButton size="sm" variant="ghost" @click="copy(item.value, item.key)">
            {{ copied === item.key ? t("renderdoc.copied") : t("renderdoc.copy") }}
          </BaseButton>
        </div>
        <pre class="rd-value">{{ item.value }}</pre>
      </div>
      <div class="modal-actions">
        <BaseButton @click="step = 'choice'">{{ t("renderdoc.back") }}</BaseButton>
        <BaseButton variant="primary" @click="copy(allText, 'all')">
          {{ copied === "all" ? t("renderdoc.copied") : t("renderdoc.copyAll") }}
        </BaseButton>
      </div>
    </template>
  </BaseModal>
</template>

<style scoped>
.rd-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 12px 0 4px;
}

.rd-group-title {
  color: var(--text-dim);
  font-size: 11.5px;
}

.rd-choices {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 12px 0 4px;
}

.rd-generate {
  margin-top: 8px;
}

/* 选项是两行字的卡片按钮：默认那套单行排版在这里不够用 */
.rd-choice {
  height: auto;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 10px 12px;
  text-align: left;
}

.rd-choice-title {
  font-size: 13px;
  font-weight: 600;
}

.rd-choice-hint {
  font-size: 11.5px;
  font-weight: 400;
  opacity: 0.75;
}

.rd-error {
  margin-top: 8px;
  color: var(--red);
  font-size: 11.5px;
}

.rd-field {
  margin-top: 12px;
}

.rd-field-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

/* 值按原样展示：等宽字、保留换行，长了在框内滚动，不把弹窗撑宽 */
.rd-value {
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
