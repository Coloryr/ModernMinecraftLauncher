<script setup lang="ts">
// 导出文件树的一行：目录行带展开箭头，文件行在箭头位置留同样宽的占位
//
// 行高固定 27px（吸顶行按滚动位置换算行号时要靠它），名字单行截断。
import { t } from "../../../lib/i18n";
import type { ExportEntryDto } from "../../../lib/bindings";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";

const props = defineProps<{
  /** 这一行的条目 */
  entry: ExportEntryDto;
  /** 缩进层级，根目录的子项为 0 */
  depth: number;
  /** 目录是否展开；文件行忽略 */
  open: boolean;
  /** 是否勾选 */
  checked: boolean;
  /** 必选项：勾着但点不动 */
  locked: boolean;
  /** 是不是吸顶的那一份：要自己垫一层不透明底，压住下面滑上来的行 */
  pinned?: boolean;
}>();

const emit = defineEmits<{
  /** 勾选 / 取消勾选一个条目 */
  (e: "toggle", entry: ExportEntryDto): void;
  /** 展开 / 收起一个目录，父组件负责拉数据 */
  (e: "open", path: string): void;
}>();

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <div class="tree-row" :class="{ 'is-dir': entry.dir, 'is-pin': props.pinned }"
    :style="{ paddingLeft: `${8 + depth * 16}px` }">
    <span class="tree-caret" :class="{ open }" @click="emit('open', entry.path)">
      <GlyphIcon v-if="entry.dir" name="chevron-down" :size="12" :weight="2.4" />
    </span>
    <input type="checkbox" class="tree-box" :checked="locked || checked" :disabled="locked"
      :title="locked ? t('winExport.mandatory') : undefined" @change="emit('toggle', entry)" />
    <span class="tree-name" :class="{ dir: entry.dir, locked }" :title="entry.path"
      @click="entry.dir ? emit('open', entry.path) : emit('toggle', entry)">{{
        entry.path.split("/").pop()
      }}</span>
    <span class="tree-size">{{ entry.dir ? "—" : sizeText(entry.size) }}</span>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  gap: 8px;
  /* 定高：吸顶行按「滚动位置 / 行高」直接换算出当前行号 */
  height: 27px;
  padding: 0 8px;
  border-radius: 8px;
  font-size: 12.5px;
  /* 点一下就是一整行的操作，别让它顺手选中文字 */
  user-select: none;
}

.tree-row:hover {
  background: var(--bg-hover);
}

/* 吸顶的那一份压在滚动内容上，必须是实底；悬停色叠在这层实底上，
   直接换成半透明的 --bg-hover 会把下面的行透出来 */
.tree-row.is-pin {
  background-color: var(--bg-solid, var(--bg-card));
}

.tree-row.is-pin:hover {
  background-color: var(--bg-solid, var(--bg-card));
  background-image: linear-gradient(var(--bg-hover), var(--bg-hover));
}

/* 箭头占位：文件行也要占同样的宽度，名字才对得齐。收起朝右、展开朝下 */
.tree-caret {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  flex-shrink: 0;
  color: var(--text-dim);
  cursor: pointer;
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.tree-caret.open {
  transform: rotate(0);
}

/* 勾选框放大一档，原生控件只调尺寸，配色仍跟主题走 */
.tree-box {
  flex-shrink: 0;
  width: 15px;
  height: 15px;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

/* 必选项：勾着但点不动 */
.tree-box:disabled {
  cursor: default;
}

.tree-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.tree-name.dir {
  font-weight: 600;
}

.tree-name.locked {
  cursor: default;
}

.tree-size {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
