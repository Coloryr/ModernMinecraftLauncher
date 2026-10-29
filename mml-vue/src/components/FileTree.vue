<script setup lang="ts">
// 文件树（递归组件）：目录可展开 / 收起，节点带勾选框
// checked 保存叶子文件的 key 集合；目录勾选态由后代文件推导：全选 / 半选 / 未选
import { collectFileKeys, type FileNode } from "../lib/fileTree";
import FileTree from "./FileTree.vue";

defineProps<{
  /** 本层的节点；目录节点带 children */
  nodes: FileNode[];
  /** 已勾选的叶子文件 key 集合；目录的勾选态由后代推导 */
  checked: Set<string>;
  /** 已展开的目录 key 集合 */
  expanded: Set<string>;
}>();

const emit = defineEmits<{
  /** 切换单个叶子文件的勾选 */
  (e: "toggle-file", key: string): void;
  /** 切换目录勾选；`on` 为 true 表示整目录选中 */
  (e: "toggle-dir", node: FileNode, on: boolean): void;
  /** 展开 / 收起目录 */
  (e: "toggle-expand", key: string): void;
  /** 首次展开懒加载目录时请求子节点 */
  (e: "lazy-load", node: FileNode): void;
}>();

/** 目录的勾选态：全选 on、部分选 ind、未选 off */
function dirState(node: FileNode, checked: Set<string>): "on" | "off" | "ind" {
  const files = collectFileKeys(node.children);
  if (!files.length) return "off";
  const on = files.filter((f) => checked.has(f)).length;
  if (on === files.length) return "on";
  if (on > 0) return "ind";
  return "off";
}

function onChevronClick(node: FileNode) {
  emit("toggle-expand", node.key);
  // 懒加载目录：首次展开时请求子节点
  if (node.lazy) emit("lazy-load", node);
}
</script>

<template>
  <div class="ftree">
    <div v-for="node in nodes" :key="node.key" class="ftree-node">
      <div class="ftree-row">
        <!-- 展开 / 收起 -->
        <span v-if="node.isDir" class="ftree-chevron" :class="{ open: expanded.has(node.key) }"
          @click="onChevronClick(node)">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
        <span v-else class="ftree-chevron"></span>

        <!-- 勾选 -->
        <input type="checkbox" :checked="node.isDir ? dirState(node, checked) === 'on' : checked.has(node.key)"
          :indeterminate="node.isDir && dirState(node, checked) === 'ind'" @change="
            node.isDir
              ? emit('toggle-dir', node, ($event.target as HTMLInputElement).checked)
              : emit('toggle-file', node.key)
            " />

        <!-- 图标 -->
        <svg v-if="node.isDir" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="#f5b944" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round" class="ftree-ico">
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
        </svg>
        <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round" class="ftree-ico">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z" />
          <path d="M14 2v6h6" />
        </svg>

        <span class="ftree-name">{{ node.name }}</span>
      </div>

      <!-- 子节点（有内容才渲染，避免空白） -->
      <div v-if="node.isDir && expanded.has(node.key) && node.children.length > 0" class="ftree-children">
        <FileTree :nodes="node.children" :checked="checked" :expanded="expanded"
          @toggle-file="emit('toggle-file', $event)"
          @toggle-dir="(n: FileNode, on: boolean) => emit('toggle-dir', n, on)"
          @toggle-expand="emit('toggle-expand', $event)" @lazy-load="emit('lazy-load', $event)" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.ftree-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 4px 6px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12.5px;
  color: var(--text);
}

.ftree-row:hover {
  background: var(--bg-hover);
}

.ftree-chevron {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  border-radius: 4px;
  transition: transform 0.12s;
}

.ftree-chevron.open {
  transform: rotate(180deg);
}

.ftree-chevron:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.ftree-row input[type="checkbox"] {
  accent-color: var(--accent);
  flex-shrink: 0;
  margin: 0;
}

.ftree-ico {
  flex-shrink: 0;
  color: var(--text-dim);
}

.ftree-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ftree-children {
  margin-left: 16px;
  border-left: 1px solid var(--border);
  padding-left: 4px;
}
</style>
