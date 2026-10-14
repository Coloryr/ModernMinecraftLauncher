<script setup lang="ts">
// 添加实例 · 文件列表面板：标题 + 已选计数 + 全选 / 全不选 + 文件树
// 「导入压缩包」与「添加文件夹」两个模式共用（两者只差树怎么读进来）
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import FileTree from "../../../components/FileTree.vue";
import { collectFileKeys, type FileNode } from "../../../lib/fileTree";

const props = defineProps<{
  /** 内容树根节点 */
  tree: FileNode[];
  /** 已勾选的文件 key */
  checked: Set<string>;
  /** 已展开的目录 key */
  expanded: Set<string>;
}>();

const emit = defineEmits<{
  /** 勾选 / 取消某个文件 */
  (e: "toggle-file", key: string): void;
  /** 勾选 / 取消整个目录，`on` 为 true 表示全选 */
  (e: "toggle-dir", node: FileNode, on: boolean): void;
  /** 展开 / 收起目录 */
  (e: "toggle-expand", key: string): void;
  /** 展开目录时按需读取它的子节点 */
  (e: "lazy-load", node: FileNode): void;
  /** 全选 / 全不选，`on` 为 true 表示全选 */
  (e: "set-all", on: boolean): void;
}>();

/** 已加载的叶子文件总数（懒加载目录展开后再计入） */
const total = computed(() => collectFileKeys(props.tree).length);
</script>

<template>
  <div class="files-head">
    <span class="files-label">{{ t("add.files") }}</span>
    <span class="files-note">{{ t("add.filesSelected", { n: checked.size, total }) }}</span>
    <span class="files-actions">
      <button type="button" class="files-btn" @click="emit('set-all', true)">{{ t("add.filesAll") }}</button>
      <button type="button" class="files-btn" @click="emit('set-all', false)">{{ t("add.filesNone") }}</button>
    </span>
  </div>
  <div class="file-list">
    <FileTree :nodes="tree" :checked="checked" :expanded="expanded" @toggle-file="emit('toggle-file', $event)"
      @toggle-dir="(n: FileNode, on: boolean) => emit('toggle-dir', n, on)"
      @toggle-expand="emit('toggle-expand', $event)" @lazy-load="emit('lazy-load', $event)" />
  </div>
</template>

<style scoped>
.files-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 10px 0 6px;
}

.files-label {
  font-weight: 600;
  font-size: 12.5px;
  color: var(--text);
  flex-shrink: 0;
}

/* 已选计数：贴着标题，右对齐的两个按钮留在最右 */
.files-note {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--text-dim);
}

.files-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.files-btn {
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  height: var(--ctl-h);
  padding: 0 10px;
  transition: border-color 0.15s, background 0.15s;
}

.files-btn:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.files-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.file-list {
  max-height: 220px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 6px;
  background: var(--bg);
  margin-bottom: 8px;
}
</style>
