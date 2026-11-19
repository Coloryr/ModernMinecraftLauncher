<script setup lang="ts">
// 游戏内代理：地址 / 端口 / 用户名 / 密码，最后一行可以测一次连通性
import { ref } from "vue";
import { t, tErr } from "../lib/i18n";
import { api } from "../lib/api";
import BaseButton from "./ui/BaseButton.vue";
import NumberStepper from "./ui/NumberStepper.vue";
import type { InstanceArgsDto } from "../lib/bindings";

const props = defineProps<{
  /** 当前实例的启动参数 */
  args: InstanceArgsDto;
}>();

const emit = defineEmits<{
  /** 回传改动后的整份启动参数 */
  (e: "update:args", v: InstanceArgsDto): void;
}>();

/** 合并 patch 后回传整份参数 */
function update(patch: Partial<InstanceArgsDto>) {
  emit("update:args", { ...props.args, ...patch });
}

/** 正在检测：按钮转圈 + 禁用，别连点 */
const checking = ref(false);
/** 上一次检测的结论；空串表示还没测过 */
const result = ref("");
/** 上一次检测是不是失败：失败用红色 */
const failed = ref(false);

/**
 * 测一次代理能不能连上
 *
 * 只测地址与端口的连通性 —— 账号密码要启动时才带上，这里验不了。
 */
async function check() {
  if (checking.value) return;
  checking.value = true;
  result.value = "";
  failed.value = false;
  try {
    const ms = await api.testProxy(props.args.proxyIp.trim(), props.args.proxyPort);
    result.value = t("proxy.checkOk", { ms });
  } catch (e) {
    failed.value = true;
    result.value = tErr(e);
  } finally {
    checking.value = false;
  }
}
</script>

<template>
  <div class="proxy-panel">
    <div class="proxy-row">
      <span class="proxy-label">{{ t("proxy.ip") }}</span>
      <input class="field-input field-grow" :value="args.proxyIp" placeholder="127.0.0.1" spellcheck="false"
        @input="update({ proxyIp: ($event.target as HTMLInputElement).value })" />
      <span class="proxy-label small">{{ t("proxy.port") }}</span>
      <NumberStepper :model-value="args.proxyPort" :min="1" :max="65535" :step="1"
        @update:model-value="(v: number) => update({ proxyPort: v })" />
    </div>

    <div class="proxy-row">
      <span class="proxy-label">{{ t("proxy.user") }}</span>
      <input class="field-input field-grow" :value="args.proxyUser" placeholder="user" spellcheck="false"
        @input="update({ proxyUser: ($event.target as HTMLInputElement).value })" />
      <span class="proxy-label small">{{ t("proxy.password") }}</span>
      <input class="field-input field-grow" type="password" :value="args.proxyPass" placeholder="••••••" spellcheck="false"
        @input="update({ proxyPass: ($event.target as HTMLInputElement).value })" />
    </div>

    <!-- 最后一行：检测 + 结论。结论跟在这一行上，测完不用再去别处找 -->
    <div class="proxy-row proxy-check">
      <span class="proxy-hint">{{ t("proxy.checkHint") }}</span>
      <BaseButton size="sm" :disabled="checking || !args.proxyIp.trim()" @click="check">
        {{ t("proxy.check") }}
      </BaseButton>
      <span v-if="result" class="proxy-result" :class="{ bad: failed }">{{ result }}</span>
    </div>
  </div>
</template>

<style scoped>
.proxy-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px;
}

.proxy-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.proxy-label {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-dim);
  min-width: 66px;
}

.proxy-label.small {
  min-width: 0;
  margin-left: 4px;
}

/* 占满剩余宽度的表单项。不能叫 .grow：Tailwind 有 flex-grow 工具类同名 */
.field-grow {
  flex: 1;
  min-width: 110px;
}

/* 检测行：与上面两行同一套标签列宽，按钮紧跟在提示文字后面 */
.proxy-check {
  padding-top: 2px;
  border-top: 1px solid var(--border);
}

.proxy-hint {
  min-width: 66px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.proxy-result {
  font-size: 11.5px;
  color: var(--green);
}

.proxy-result.bad {
  color: var(--red);
}
</style>
