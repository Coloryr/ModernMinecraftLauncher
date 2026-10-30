<script setup lang="ts">
// .minecraft 导入：「发现旧数据」弹窗里的一节
//
// 用户选一个版本、可改实例名；只搬游戏数据，见后端 windows/minecraft。
// 版本列表里 M²L 没收录的（LabyMod 这类）按自定义版本导入，界面上说明一句。
import { computed, ref, watch } from "vue";
import type { MinecraftImportDto, MinecraftInfoDto, MinecraftProgressDto } from "../../lib/bindings";
import { api, onMinecraftProgress } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import BaseButton from "../../components/ui/BaseButton.vue";

/** .minecraft 探测结果：目录、版本列表与会搬的顶层条目 */
const props = defineProps<{ info: MinecraftInfoDto }>();
/** `done` 表示实例已建好，父组件据此重拉实例列表；`busy` 表示正在搬，弹窗不许关 */
const emit = defineEmits<{
  (e: "done"): void;
  (e: "busy", busy: boolean): void;
}>();

/** 版本列表，按 id 排序 */
const versions = computed(() => props.info.versions);
/** 选中的版本 id，默认最后一个（通常最新） */
const version = ref(versions.value[versions.value.length - 1]?.id ?? "");
/** 实例名，默认跟版本 id；用户改过之后就不再跟着变 */
const name = ref(version.value);
/** 用户是否改过实例名 */
const nameEdited = ref(false);
/** 导入进行中 */
const running = ref(false);
/** 已经回答过（导入完成或选择不导入） */
const settled = ref(false);
/** 最近一次进度 */
const progress = ref<MinecraftProgressDto | null>(null);
/** 导入结果 */
const result = ref<MinecraftImportDto | null>(null);
/** 失败信息（后端给的是 i18n key） */
const error = ref("");

/** 进度事件取消订阅 */
let unlisten: (() => void) | null = null;

/** 选中版本的元数据 */
const current = computed(() => versions.value.find((item) => item.id === version.value));

/** 加载器文案 */
const loaderText = computed(() => t(`add.loader.${current.value?.loader ?? "normal"}`));

/** 百分比；total 为 0（未知）返回 -1 = 不确定进度 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return -1;
  return Math.min(100, Math.max(0, Math.round((p.done / p.total) * 100)));
});

watch(version, (now) => {
  if (!nameEdited.value) name.value = now;
});

/** 开始导入：订阅进度事件，跑完把结果放进 `result` */
async function importVersion() {
  if (running.value || !version.value) return;
  running.value = true;
  emit("busy", true);
  error.value = "";
  result.value = null;
  progress.value = null;
  try {
    unlisten = await onMinecraftProgress((p) => {
      progress.value = p;
    });
    result.value = await api.importMinecraft(
      props.info.path,
      version.value,
      name.value.trim() || null,
    );
    emit("done");
  } catch (e) {
    error.value = tErr(e);
  } finally {
    unlisten?.();
    unlisten = null;
    running.value = false;
    // 只有导入成功才算答完；失败时收起表单会看不到错误，也重试不了
    if (result.value) settled.value = true;
    emit("busy", false);
  }
}

/** 不导入：记下标记后收起 */
async function skip() {
  try {
    await api.skipMinecraft();
  } catch {
    /* 标记没写上顶多下次再问一次 */
  }
  settled.value = true;
}
</script>

<template>
  <section class="old-section">
    <div class="old-head">
      <span class="old-name">{{ t("minecraft.section") }}</span>
      <span class="old-meta">{{ t("minecraft.versions", { n: versions.length }) }}</span>
    </div>
    <div class="src">{{ info.path }}</div>

    <!-- ---------- 1. 还没开始 ---------- -->
    <template v-if="!running && !result && !settled">
      <div class="old-form">
        <label class="field-label">{{ t("minecraft.version") }}</label>
        <select v-model="version" class="field-select">
          <option v-for="item in versions" :key="item.id" :value="item.id">{{ item.id }}</option>
        </select>
        <span class="old-loader">{{ loaderText }}</span>
      </div>
      <div class="old-form">
        <label class="field-label">{{ t("minecraft.name") }}</label>
        <input v-model="name" class="field-input" @input="nameEdited = true" />
      </div>
      <p class="warn">{{ t("minecraft.hint") }}</p>
      <p v-if="current && !current.known" class="warn">{{ t("minecraft.unknown") }}</p>
      <p v-if="error" class="error">{{ error }}</p>
      <div class="old-actions">
        <BaseButton @click="skip">{{ t("minecraft.skip") }}</BaseButton>
        <BaseButton variant="accent" @click="importVersion">{{ t("minecraft.import") }}</BaseButton>
      </div>
    </template>

    <!-- ---------- 2. 导入中 ---------- -->
    <template v-else-if="running">
      <p class="modal-text">{{ t("minecraft.importing") }}</p>
      <div class="bar" :class="{ indet: percent < 0 }">
        <div class="bar-fill" :style="percent < 0 ? undefined : { width: percent + '%' }" />
      </div>
      <div class="run-info">
        <span v-if="progress && progress.total > 0">{{ progress.done }} / {{ progress.total }}</span>
        <span class="cur">{{ progress?.text }}</span>
      </div>
    </template>

    <!-- ---------- 3. 结果 ---------- -->
    <template v-else-if="result">
      <p class="modal-text">{{ t("minecraft.done", { n: result.files }) }}</p>
      <div class="line ok">
        {{ result.name }}{{ t("common.detailSep") }}{{ result.version }}
      </div>
      <div v-if="result.failed.length" class="line bad">
        {{ t("minecraft.failed", { n: result.failed.length }) }}
      </div>
    </template>

    <!-- ---------- 4. 跳过了 ---------- -->
    <p v-else-if="settled && !result" class="settled">{{ t("minecraft.skipped") }}</p>
  </section>
</template>
