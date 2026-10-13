<script setup lang="ts">
// 配置文件窗口：左侧列可编辑的文件，右侧按类型给四种视图
//
// - 文本（json / json5 / toml / yaml / properties / xml / js / snbt / 纯文本）→ 文本框编辑 + 保存
// - NBT（.dat / .nbt / .rio）→ NBT 树
// - 区域文件（.mca）→ 区块表
// - 图片 / 压缩包 → 二进制视图（一页 4 KiB 的十六进制）
//
// 来源两条：实例自己的 options.txt 与 `config` / `kubejs` / `local`，或某个存档目录下的文件。
// 实例取自 gui_config 的 `mainWindow.selectedInstance`（与资源管理窗口同一口径，窗口自己读）；
// 后端只认"相对游戏目录的路径"，列表里给什么就原样回传。
import { computed, onMounted, ref } from "vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import {
  api,
  listConfigFiles,
  listConfigSaveFiles,
  listSaves,
  openInstanceDir,
  readConfigFile,
  writeConfigFile,
} from "../../lib/api";
import { loadGuiConfig } from "../../lib/guiConfig";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import HexView from "./parts/HexView.vue";
import McaTable from "./parts/McaTable.vue";
import NbtTree from "./parts/NbtTree.vue";
import type { FileItemDto, SaveItemDto } from "../../lib/bindings";

/** 请求关闭本窗口 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 文本类文件：能编能存；其余走只读的结构 / 二进制视图 */
const TEXT_TYPES = ["json", "json5", "toml", "yaml", "properties", "xml", "javascript", "snbt", "text"];

/** 当前实例 */
const instanceUuid = ref("");
const instanceName = ref("");
/** 存档列表，"看某个存档里的文件"用 */
const worlds = ref<SaveItemDto[]>([]);
/** 当前来源：空串 = 实例本身，否则是存档目录名 */
const source = ref("");
const files = ref<FileItemDto[]>([]);
const selected = ref<FileItemDto | null>(null);
const loading = ref(false);

/** 文本内容与读进来的原文：两者不同就是"未保存" */
const text = ref("");
const origin = ref("");
const saving = ref(false);

const dirty = computed(() => text.value !== origin.value);
const isText = computed(() => !!selected.value && TEXT_TYPES.includes(selected.value.fileType));

/** 读当前实例与它的存档列表，再列文件 */
async function load() {
  loading.value = true;
  try {
    const [list, cfg] = await Promise.all([api.getInstances(), loadGuiConfig()]);
    const uuid = cfg?.mainWindow.selectedInstance ?? "";
    const found = list.find((i) => i.uuid === uuid) ?? null;
    // 换了实例就把来源退回"实例本身"：原来那个存档名在新实例里没有
    if (found?.uuid !== instanceUuid.value) source.value = "";
    instanceUuid.value = found?.uuid ?? "";
    instanceName.value = found?.name ?? "";
    worlds.value = found ? await listSaves(found.uuid) : [];
  } catch (e) {
    showToast(tErr(e));
    instanceUuid.value = "";
    instanceName.value = "";
    worlds.value = [];
  } finally {
    loading.value = false;
  }
  await loadFiles();
}

/** 按当前来源列文件 */
async function loadFiles() {
  selected.value = null;
  text.value = "";
  origin.value = "";
  if (!instanceUuid.value) {
    files.value = [];
    return;
  }

  loading.value = true;
  try {
    const list = source.value
      ? await listConfigSaveFiles(instanceUuid.value, source.value)
      : await listConfigFiles(instanceUuid.value);
    // 后端按扫描顺序给，这里按路径排序：同一批文件每次打开顺序都一样
    files.value = [...list].sort((a, b) => a.name.localeCompare(b.name));
  } catch (e) {
    files.value = [];
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 选中一个文件：文本类顺手把内容拉下来，其余交给各自的视图组件 */
async function open(file: FileItemDto) {
  selected.value = file;
  text.value = "";
  origin.value = "";
  if (!TEXT_TYPES.includes(file.fileType)) return;

  loading.value = true;
  try {
    const data = await readConfigFile(instanceUuid.value, file.name);
    text.value = data;
    origin.value = data;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 保存当前文本文件，整份覆盖 */
async function save() {
  const file = selected.value;
  if (!file || !isText.value || saving.value || !dirty.value) return;

  saving.value = true;
  try {
    await writeConfigFile(instanceUuid.value, file.name, text.value);
    origin.value = text.value;
    showToast(t("config.saved"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    saving.value = false;
  }
}

/** 在文件管理器里打开这个实例的目录 */
function openDir() {
  if (!instanceUuid.value) return;
  void openInstanceDir(instanceUuid.value);
}

/** 换来源：存档下拉改了就重列 */
async function pickSource(value: string) {
  if (value === source.value) return;
  source.value = value;
  await loadFiles();
}

onMounted(() => void load());
// 单窗口模式切回本窗口时重读；有没保存的改动就不动，免得把编辑内容冲掉
useWindowRefresh(() => {
  if (!dirty.value) void load();
});
</script>

<template>
  <WindowFrame :title="t('config.title')" body-fill @close="emit('close')">
    <div class="cfg-bar">
      <span class="cfg-inst">{{ instanceName || t("config.noInstance") }}</span>
      <select class="field-select cfg-source" :value="source" :disabled="loading"
        @change="pickSource(($event.target as HTMLSelectElement).value)">
        <option value="">{{ t("config.instance") }}</option>
        <option v-for="world in worlds" :key="world.dir" :value="world.dir">
          {{ world.levelName || world.dir }}
        </option>
      </select>
      <BaseButton size="sm" :disabled="loading" @click="loadFiles">{{ t("config.refresh") }}</BaseButton>
      <BaseButton size="sm" :disabled="!instanceUuid" @click="openDir">{{ t("config.openDir") }}</BaseButton>

      <template v-if="isText">
        <span v-if="dirty" class="cfg-dirty">{{ t("config.unsaved") }}</span>
        <BaseButton size="sm" variant="primary" :disabled="saving || !dirty" @click="save">
          {{ t("config.save") }}
        </BaseButton>
      </template>
      <span v-else-if="selected" class="cfg-readonly">{{ t("config.readonly") }}</span>
    </div>

    <div class="cfg-body">
      <!-- 左：文件列表。名字是相对游戏目录的路径，长的自己省略 -->
      <aside class="cfg-list">
        <button v-for="file in files" :key="file.name" class="cfg-file"
          :class="{ on: selected?.name === file.name }" :title="file.name" @click="open(file)">
          <span class="cfg-file-name">{{ file.name }}</span>
          <span class="cfg-file-type">{{ file.fileType }}</span>
        </button>
        <p v-if="!files.length" class="empty-tip">
          {{ loading ? t("config.loading") : t("config.noFiles") }}
        </p>
      </aside>

      <!-- 右：按类型选视图 -->
      <section class="cfg-view">
        <p v-if="!selected" class="empty-tip">{{ t("config.pickFile") }}</p>
        <textarea v-else-if="isText" v-model="text" class="cfg-text" spellcheck="false" />
        <NbtTree v-else-if="selected.fileType === 'nbt'" :uuid="instanceUuid" :file="selected.name" />
        <McaTable v-else-if="selected.fileType === 'mca'" :uuid="instanceUuid" :file="selected.name" />
        <HexView v-else :uuid="instanceUuid" :file="selected.name" />
      </section>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* 工具条：一行放不下时也不换行，交给各控件自己缩 */
.cfg-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding-bottom: 10px;
}

.cfg-inst {
  flex-shrink: 0;
  font-size: 13px;
  color: var(--text);
}

/* 存档下拉：跟工具条一样高（.field-select 的表单规格是 42px，这里压矮一档） */
.cfg-source {
  width: auto;
  min-width: 160px;
  max-width: 260px;
  min-height: 0;
  padding: 6px 26px 6px 10px;
  font-size: 12.5px;
  background-position: right 6px center;
}

.cfg-dirty {
  flex-shrink: 0;
  color: var(--yellow);
  font-size: 12px;
}

.cfg-readonly {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 12px;
}

/* 两栏：左固定宽、右吃掉剩余；两层都要 min-height: 0，不然子元素滚不动 */
.cfg-body {
  display: flex;
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.cfg-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 250px;
  flex-shrink: 0;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
}

.cfg-file {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.cfg-file:hover {
  background: var(--bg-hover);
}

.cfg-file.on {
  background: var(--accent-soft);
  color: var(--accent);
}

.cfg-file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cfg-file-type {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 10.5px;
}

.cfg-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

/* 文本框：等宽、铺满、可纵向拉 */
.cfg-text {
  flex: 1;
  min-height: 0;
  resize: none;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-solid, var(--bg-card));
  color: var(--text);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 12.5px;
  line-height: 1.55;
  outline: none;
}

.cfg-text:focus {
  border-color: var(--accent);
}
</style>
