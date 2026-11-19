<script setup lang="ts">
// 实例的自定义启动配置：json/ 目录下每个文件一行的摘要表
//
// 与 ColorMC 游戏编辑窗口 tab1 的那张表同一组列（排序 / 名称 / UID / 版本 / 路径）。
// 数据是读目录现算的，不进 gui_config —— 用户可以手改 json 目录，拉一次就是最新的。
// 每行可以就地改原文（弹窗里编辑）或交给系统默认程序打开；最后一行用来新建一份。
import { ref, watch } from "vue";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import { api } from "../lib/api";
import BaseButton from "./ui/BaseButton.vue";
import BaseModal from "./ui/BaseModal.vue";
import GlyphIcon from "./ui/GlyphIcon.vue";
import type { CustomJsonItemDto } from "../lib/bindings";

const props = defineProps<{
  /** 实例 uuid；换实例时重新拉一次 */
  uuid: string;
}>();

const list = ref<CustomJsonItemDto[]>([]);
/** 拉取代次：切实例时丢弃旧一发的返回 */
let loadSeq = 0;

/** 拉一次列表；失败按空表处理，不打断详情页 */
async function load() {
  const mine = ++loadSeq;
  if (!props.uuid) {
    list.value = [];
    return;
  }
  try {
    const items = await api.getCustomJson(props.uuid);
    if (mine === loadSeq) list.value = items;
  } catch {
    if (mine === loadSeq) list.value = [];
  }
}

watch(() => props.uuid, load, { immediate: true });

// ---- 编辑 ----

/** 正在编辑的文件名；空串表示弹窗没开 */
const editing = ref("");
/** 编辑器里的原文 */
const draft = ref("");
/** 新建弹窗是否打开 */
const creating = ref(false);
/** 新建时填的文件名 */
const newName = ref("");
/** 保存 / 新建期间不许重复点 */
const busy = ref(false);
/** 上一步失败的原因，显示在弹窗里 */
const error = ref("");

/**
 * 打开一份自定义 JSON 的编辑弹窗
 *
 * - \`file\`: json/ 里的文件名
 */
async function openEditor(file: string) {
  try {
    draft.value = await api.readCustomJson(props.uuid, file);
    error.value = "";
    editing.value = file;
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 保存正在编辑的那份：后端会先按 JSON 解析一遍 */
async function saveEditor() {
  if (!editing.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await api.saveCustomJson(props.uuid, editing.value, draft.value);
    editing.value = "";
    await load();
    showToast(t("customJson.saved"));
  } catch (e) {
    error.value = tErr(e);
  } finally {
    busy.value = false;
  }
}

/**
 * 用系统默认程序打开一份自定义 JSON
 *
 * - \`file\`: json/ 里的文件名
 */
async function openFile(file: string) {
  try {
    await api.openCustomJson(props.uuid, file, false);
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 打开新建弹窗，名字清空 */
function openCreate() {
  newName.value = "";
  error.value = "";
  creating.value = true;
}

/** 新建一份：建完直接打开它的编辑弹窗，省得再点一次 */
async function create() {
  const name = newName.value.trim();
  if (!name || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    const file = await api.createCustomJson(props.uuid, name);
    creating.value = false;
    await load();
    await openEditor(file);
  } catch (e) {
    error.value = tErr(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="json-panel">
    <p v-if="list.length === 0" class="json-empty">{{ t("customJson.empty") }}</p>
    <table v-else class="json-table">
      <thead>
        <tr>
          <th class="col-order">{{ t("customJson.order") }}</th>
          <th>{{ t("customJson.name") }}</th>
          <th>{{ t("customJson.uid") }}</th>
          <th>{{ t("customJson.version") }}</th>
          <th>{{ t("customJson.file") }}</th>
          <!-- 操作列表头留空：两个文字按钮自己说得清 -->
          <th class="col-ops"></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in list" :key="item.file">
          <td class="col-order">{{ item.order }}</td>
          <td>{{ item.name }}</td>
          <td class="mono">{{ item.uid }}</td>
          <td>{{ item.version }}</td>
          <td class="mono">{{ item.file }}</td>
          <td class="col-ops">
            <div class="json-ops">
              <BaseButton size="sm" variant="ghost" @click="openEditor(item.file)">{{ t("customJson.edit") }}</BaseButton>
              <BaseButton size="sm" variant="ghost" @click="openFile(item.file)">{{ t("customJson.openFile") }}</BaseButton>
            </div>
          </td>
        </tr>
      </tbody>
    </table>

    <!-- 最后一行：新建一份自定义 JSON -->
    <BaseButton class="json-add" variant="ghost" @click="openCreate">
      <GlyphIcon name="plus" :size="13" :weight="2.2" /> {{ t("customJson.add") }}
    </BaseButton>

    <!-- 编辑弹窗：原文是 JSON，等宽字、竖排换行都保留 -->
    <BaseModal v-if="editing" :title="t('customJson.editTitle', { file: editing })" :width="640"
      @close="editing = ''">
      <textarea v-model="draft" class="json-editor" spellcheck="false"></textarea>
      <p v-if="error" class="json-error">{{ error }}</p>
      <div class="modal-actions">
        <BaseButton @click="editing = ''">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="busy" @click="saveEditor">{{ t("customJson.save") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 新建弹窗：只问文件名，内容用后端给的模板 -->
    <BaseModal v-if="creating" :title="t('customJson.addTitle')" @close="creating = false">
      <label class="field-label">{{ t("customJson.fileName") }}</label>
      <input v-model="newName" class="field-input" :placeholder="t('customJson.namePlaceholder')" spellcheck="false"
        @keyup.enter="create" />
      <p v-if="error" class="json-error">{{ error }}</p>
      <div class="modal-actions">
        <BaseButton @click="creating = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" :disabled="busy || !newName.trim()" @click="create">{{ t("customJson.create") }}</BaseButton>
      </div>
    </BaseModal>
  </div>
</template>

<style scoped>
.json-panel {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px 16px;
  overflow-x: auto;
}

.json-empty {
  margin: 0 0 10px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.json-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
}

.json-table th {
  padding: 4px 8px 6px;
  text-align: left;
  font-weight: 600;
  color: var(--text-dim);
  white-space: nowrap;
}

.json-table td {
  padding: 5px 8px;
  border-top: 1px solid var(--border);
}

.col-order {
  width: 48px;
}

/* 操作列：两个文字按钮靠右，不换行 */
.col-ops {
  width: 1%;
  white-space: nowrap;
}

.json-ops {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
}

/* uid 与文件名是标识，等宽字更好认 */
.mono {
  font-family: "Cascadia Code", Consolas, monospace;
  color: var(--text-dim);
}

/* 新建按钮占满整行：和表同宽，读作"在这张表末尾加一条" */
.json-add {
  width: 100%;
  margin-top: 10px;
  justify-content: center;
  border-style: dashed;
}

.json-editor {
  width: 100%;
  min-height: 320px;
  max-height: 60vh;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
  resize: vertical;
  outline: none;
}

.json-editor:focus {
  border-color: var(--accent);
}

.json-error {
  margin: 8px 0 0;
  color: var(--red);
  font-size: 11.5px;
}
</style>
