<script setup lang="ts">
// 添加实例 · 模式四：在线实例（下载地址）
import { nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import { isArchiveUrl, serverPackUrl } from "../../../lib/onlineInstance";
import type { ServerPackProbeDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 在线实例的下载地址 */
  url: string;
  /** 校验失败：下载地址必填（高亮 + 自动聚焦） */
  invalid?: boolean;
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

// 必填校验失败：聚焦地址框，高亮见模板的 is-invalid
watch(
  () => props.invalid,
  async (invalid) => {
    if (!invalid) return;
    await nextTick();
    urlInput.value?.focus();
  },
);

/** 失焦时补齐清单文件名：在线实例认 server.json；压缩包地址原样留着 */
function fillManifest() {
  const next = isArchiveUrl(props.url) ? props.url.trim() : serverPackUrl(props.url);
  if (next && next !== props.url) emit("update:url", next);
}
</script>

<template>
  <label class="field-label">{{ t("add.url") }} <span class="req">*</span></label>
  <input ref="urlInput" :value="url" class="field-input" :class="{ 'is-invalid': invalid }"
    :placeholder="t('add.urlPlaceholder')" spellcheck="false" autocomplete="off"
    @input="emit('update:url', ($event.target as HTMLInputElement).value)" @blur="fillManifest" />
  <p class="field-hint">{{ t("add.urlHint") }}</p>

  <!-- 填完地址自动读一次清单：先让人看清会装成什么，再决定要不要下载 -->
  <p v-if="probing" class="probe-tip">{{ t("add.probeReading") }}</p>
  <p v-else-if="probeError" class="probe-tip is-error">{{ probeError }}</p>
  <div v-else-if="probe" class="probe-card">
    <div class="probe-name">{{ probe.name }}</div>
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
    <p v-if="probe.text" class="probe-text">{{ probe.text }}</p>
  </div>
</template>

<style scoped>
/* 卡片里第一条标签顶格，不留多余的 12px */
.field-label:first-child {
  margin-top: 0;
}

/* 清单摘要：读的时候一句提示，失败时同一位置换成红字 */
.probe-tip {
  margin: 10px 0 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

.probe-tip.is-error {
  color: var(--red);
}

/* 摘要卡：与窗口里其它卡片同一副外观 */
.probe-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 10px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
}

.probe-name {
  margin-bottom: 2px;
  font-size: 13px;
  font-weight: 600;
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

.probe-text {
  margin: 6px 0 0;
  font-size: 11.5px;
  color: var(--text-dim);
  line-height: 1.6;
}
</style>
