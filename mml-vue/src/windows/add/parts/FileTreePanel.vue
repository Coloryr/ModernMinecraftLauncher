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
  <div class="files-panel flex min-h-0 flex-1 flex-col">
    <div class="files-head mx-0 mb-1.5 mt-2.5 flex items-center gap-2.5">
      <span class="files-label shrink-0 text-body font-semibold text-fg">{{ t("add.files") }}</span>
      <span class="files-note min-w-0 flex-1 truncate text-[12px] text-dim">{{ t("add.filesSelected", { n: checked.size, total }) }}</span>
      <span class="files-actions flex shrink-0 gap-2">
        <button type="button" class="files-btn h-ctl cursor-pointer rounded-ctl border border-border bg-raised px-3.5 text-ctl font-semibold text-accent transition-[border-color,background] duration-150" @click="emit('set-all', true)">{{ t("add.filesAll") }}</button>
        <button type="button" class="files-btn h-ctl cursor-pointer rounded-ctl border border-border bg-raised px-3.5 text-ctl font-semibold text-accent transition-[border-color,background] duration-150" @click="emit('set-all', false)">{{ t("add.filesNone") }}</button>
      </span>
    </div>
    <div class="file-list mb-2 min-h-0 flex-1 overflow-y-scroll rounded-card border border-border bg-bg p-1.5 [scrollbar-gutter:stable]">
      <FileTree :nodes="tree" :checked="checked" :expanded="expanded" @toggle-file="emit('toggle-file', $event)"
        @toggle-dir="(n: FileNode, on: boolean) => emit('toggle-dir', n, on)"
        @toggle-expand="emit('toggle-expand', $event)" @lazy-load="emit('lazy-load', $event)" />
    </div>
  </div>
</template>

<style scoped>

.files-btn:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.files-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
</style>
