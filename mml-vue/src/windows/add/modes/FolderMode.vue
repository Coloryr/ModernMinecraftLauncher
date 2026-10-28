<script setup lang="ts">
// 添加实例 · 模式三：添加文件夹，路径框 + 实例列表 / 内容树
//
// 两种结果二选一：
// - 目录里**装着实例**，如 `.minecraft` 及其下的 `versions/*` → 列出来让用户勾选要导入哪些；
// - 目录本身就是**一个实例目录**（没有可扫描的实例）→ 退回原来的内容树，整目录作为一个实例导入。
import { nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import FileTreePanel from "../parts/FileTreePanel.vue";
import LoaderQueryProgress from "../../../components/LoaderQueryProgress.vue";
import type { FileNode } from "../../../lib/fileTree";
import type { FolderInstanceDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 文件夹路径 */
  path: string;
  /** 目录内容树，懒加载时子节点随展开补齐 */
  tree: FileNode[];
  /** 已勾选的文件 key */
  checked: Set<string>;
  /** 已展开的目录 key */
  expanded: Set<string>;
  /** 扫描出的可导入实例；空表示目录本身就是实例目录，走内容树那条路 */
  found: FolderInstanceDto[];
  /** 已勾选要导入的实例路径 */
  picked: Set<string>;
  /** 正在扫描 */
  scanning: boolean;
  /** 扫描失败的原因（空 = 没出错） */
  scanError: string;
  /** 校验失败：文件夹必填（高亮 + 自动聚焦） */
  invalid?: boolean;
  /** 校验失败的文案，贴在路径框下面 */
  error?: string;
}>();

const emit = defineEmits<{
  /** 文件夹路径变化 */
  (e: "update:path", v: string): void;
  /** 点了「浏览」选文件夹 */
  (e: "pick"): void;
  /** 点了「扫描目录」，扫描手输 / 粘贴的路径 */
  (e: "rescan"): void;
  /** 勾选 / 取消某个扫到的实例，`on` 为 true 表示勾选 */
  (e: "toggle-instance", path: string, on: boolean): void;
  /** 全选 / 全不选扫到的实例，`on` 为 true 表示全选 */
  (e: "set-all-instances", on: boolean): void;
  /** 勾选 / 取消某个文件 */
  (e: "toggle-file", key: string): void;
  /** 勾选 / 取消整个目录，`on` 为 true 表示全选 */
  (e: "toggle-dir", node: FileNode, on: boolean): void;
  /** 展开 / 收起目录 */
  (e: "toggle-expand", key: string): void;
  /** 展开目录时按需读取它的子节点 */
  (e: "lazy-load", node: FileNode): void;
  /** 全选 / 全不选内容树，`on` 为 true 表示全选 */
  (e: "set-all", on: boolean): void;
}>();

const pathInput = ref<HTMLInputElement | null>(null);

// 必填校验失败：聚焦路径框，高亮见模板的 is-invalid
watch(
  () => props.invalid,
  async (invalid) => {
    if (!invalid) return;
    await nextTick();
    pathInput.value?.focus();
  },
);
</script>

<template>
  <label class="field-label">{{ t("add.folder") }} <span class="req">*</span></label>
  <div class="path-row">
    <input ref="pathInput" :value="path" class="field-input" :class="{ 'is-invalid': invalid }"
      :placeholder="t('add.folderPlaceholder')" spellcheck="false" autocomplete="off"
      @input="emit('update:path', ($event.target as HTMLInputElement).value)" />
    <BaseButton class="pick-btn" size="sm" variant="accent" @click="emit('pick')">{{ t("add.browse") }}</BaseButton>
    <!-- 手动输入 / 粘贴路径后用它触发扫描（选目录走确认框，不用点这里） -->
    <BaseButton v-if="path.trim()" size="sm" variant="ghost" :disabled="scanning" @click="emit('rescan')">
      {{ t("add.folderScan") }}
    </BaseButton>
  </div>

  <!-- 校验失败的原因贴在框下面，别让它落到底部按钮上方去找 -->
  <p v-if="error" class="error-text" role="alert">{{ error }}</p>

  <!-- 扫描失败：把原因显示出来，以前静默回落，看起来像功能没生效 -->
  <p v-if="scanError" class="field-hint scan-err">
    {{ t("add.folderScanFail", { msg: scanError }) }}
  </p>

  <!-- 扫描中：不确定滚动进度条（与加载器查询同一套观感），不是一行干文字 -->
  <LoaderQueryProgress v-else-if="scanning" kind="versions" :visible="true" :step="0" :total="0"
    :label="t('add.folderScanning')" />

  <!-- 扫到了实例：列出来勾选（默认全选） -->
  <template v-else-if="found.length">
    <div class="found-head">
      <span class="found-title">{{ t("add.folderFound", { n: found.length }) }}</span>
      <span class="found-actions">
        <button type="button" class="found-link" @click="emit('set-all-instances', true)">
          {{ t("add.filesAll") }}
        </button>
        <button type="button" class="found-link" @click="emit('set-all-instances', false)">
          {{ t("add.filesNone") }}
        </button>
      </span>
    </div>
    <div class="found-list">
      <label v-for="item in found" :key="item.path" class="found-item">
        <input type="checkbox" :checked="picked.has(item.path)"
          @change="emit('toggle-instance', item.path, ($event.target as HTMLInputElement).checked)" />
        <span class="found-name">{{ item.name }}</span>
        <span class="found-path" v-tip="item.path">{{ item.path }}</span>
      </label>
    </div>
  </template>

  <!-- 没扫到实例：目录本身就是实例目录，退回内容树（整目录作为一个实例导入） -->
  <template v-else-if="tree.length">
    <FileTreePanel :tree="tree" :checked="checked" :expanded="expanded" @toggle-file="emit('toggle-file', $event)"
      @toggle-dir="(n: FileNode, on: boolean) => emit('toggle-dir', n, on)"
      @toggle-expand="emit('toggle-expand', $event)" @lazy-load="emit('lazy-load', $event)"
      @set-all="emit('set-all', $event)" />
  </template>

  <!-- 还没选文件夹 / 选了个没扫到实例又读不出内容的目录 -->
  <p v-else class="field-hint">{{ path ? t("add.folderNone") : t("add.folderHint") }}</p>
</template>

<style scoped>
/* 卡片里第一条标签顶格，不留多余的 12px */
.field-label:first-child {
  margin-top: 0;
}

/* 扫描失败提示：用告警色，别和普通灰色提示混在一起 */
.scan-err {
  color: var(--red);
}

.found-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 12px;
}

.found-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-dim);
}

.found-actions {
  display: flex;
  gap: 10px;
}

.found-link {
  border: none;
  background: transparent;
  padding: 0;
  color: var(--accent);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
}

.found-link:hover {
  text-decoration: underline;
}

.found-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 6px;
  max-height: 200px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.found-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: 7px;
  cursor: pointer;
}

.found-item:hover {
  background: var(--bg-hover);
}

.found-name {
  font-size: 13px;
  color: var(--text);
  flex-shrink: 0;
}

/* 路径只作辅助信息：窄了先省略它 */
.found-path {
  flex: 1;
  min-width: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: right;
}
</style>
