<script setup lang="ts">
// 添加实例 · 模式四：在线实例（下载地址）
import { computed, nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import { isArchiveUrl, serverPackUrl } from "../../../lib/onlineInstance";
import type { ServerPackProbeDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 在线实例的下载地址 */
  url: string;
  /** 校验失败：下载地址必填（高亮 + 自动聚焦） */
  invalid?: boolean;
  /** 校验失败的文案，贴在地址框下面 */
  error?: string;
  /** 清单摘要；null 表示还没探到 */
  probe?: ServerPackProbeDto | null;
  /** 正在读清单 */
  probing?: boolean;
  /** 读清单失败的原因 */
  probeError?: string;
}>();

const emit = defineEmits<{
  /** 下载地址变化 */
  (e: "update:url", v: string): void;
}>();

const urlInput = ref<HTMLInputElement | null>(null);

/** 填的是整合包压缩包地址：在线实例不收，提示去「导入压缩包」 */
const archiveUrl = computed(() => isArchiveUrl(props.url));

// 必填校验失败：聚焦地址框，高亮见模板的 is-invalid
watch(
  () => props.invalid,
  async (invalid) => {
    if (!invalid) return;
    await nextTick();
    urlInput.value?.focus();
  },
);

/** 失焦时补齐清单文件名：在线实例认 server.json；压缩包地址原样留着交给上面的校验拦 */
function fillManifest() {
  if (archiveUrl.value) return;
  const next = serverPackUrl(props.url);
  if (next && next !== props.url) emit("update:url", next);
}
</script>

<template>
  <label class="field-label">{{ t("add.url") }} <span class="req">*</span></label>
  <input ref="urlInput" :value="url" class="field-input" :class="{ 'is-invalid': invalid }"
    :placeholder="t('add.urlPlaceholder')" spellcheck="false" autocomplete="off"
    @input="emit('update:url', ($event.target as HTMLInputElement).value)" @blur="fillManifest" />

  <!-- 校验失败的原因贴在框下面，别让它落到底部按钮上方去找 -->
  <p v-if="error" class="error-text" role="alert">{{ error }}</p>

  <p v-else-if="archiveUrl" class="error-text" role="alert">{{ t("add.urlArchiveNotAllowed") }}</p>

  <!-- 读清单期间与失败时都只留状态行，不再在它上面叠那行说明 -->
  <p v-else-if="!probing && !probeError" class="field-hint">{{ t("add.urlHint") }}</p>

  <!-- 填完地址自动读一次清单：先让人看清会装成什么，再决定要不要下载 -->
  <p v-if="probing" class="probe-tip mt-2.5 text-body text-dim">{{ t("add.probeReading") }}</p>
  <p v-else-if="probeError" class="probe-tip is-error mt-2.5 text-body">{{ probeError }}</p>
  <div v-else-if="probe" class="probe-card mt-2.5 flex flex-col gap-1 rounded-panel border border-border bg-card px-3.5 py-3">
    <div class="probe-name mb-0.5 text-title font-semibold">{{ probe.name }}</div>
    <div class="probe-row">
      <span>{{ t("meta.version") }}</span><b>{{ probe.version }}</b>
    </div>
    <div class="probe-row">
      <span>{{ t("meta.loader") }}</span>
      <b>{{ t(`add.loader.${probe.loader}`) }}<template v-if="probe.loaderVersion">
          {{ " " + probe.loaderVersion }}</template></b>
    </div>
    <div class="probe-row">
      <span>{{ t("add.probePackVersion") }}</span><b>{{ probe.packVersion || "—" }}</b>
    </div>
    <div class="probe-row">
      <span>{{ t("add.probeContents") }}</span>
      <b>{{ t("add.probeCounts", { files: probe.fileCount, archives: probe.archiveCount }) }}</b>
    </div>
    <p v-if="probe.text" class="probe-text mt-1.5 text-meta leading-[1.6] text-dim">{{ probe.text }}</p>
  </div>
</template>

<style scoped>
/* 卡片里第一条标签顶格，不留多余的 12px */
.field-label:first-child {
  margin-top: 0;
}

.probe-tip.is-error {
  color: var(--red);
}

/* 标签列定宽：几行的值才能对齐 */
.probe-row {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 8px;
  font-size: 12.5px;
}

.probe-row span {
  color: var(--text-dim);
  font-size: 11.5px;
  line-height: 20px;
}

.probe-row b {
  min-width: 0;
  font-weight: 500;
  line-height: 20px;
  word-break: break-all;
}
</style>
