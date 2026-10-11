<script setup lang="ts">
// 添加实例 · 模式二：导入压缩包（路径框 + 文件树 + 压缩包类型）
import { nextTick, ref, watch } from "vue";
import { t } from "../../../lib/i18n";
import BaseButton from "../../../components/ui/BaseButton.vue";
import FileTreePanel from "../parts/FileTreePanel.vue";
import LoaderQueryProgress from "../../../components/LoaderQueryProgress.vue";
import type { FileNode } from "../../../lib/fileTree";

const props = defineProps<{
  /** 压缩包路径 */
  path: string;
  /** 压缩包内容树，懒加载时子节点随展开补齐 */
  tree: FileNode[];
  /** 已勾选的文件 key */
  checked: Set<string>;
  /** 已展开的目录 key */
  expanded: Set<string>;
  /** 可选的压缩包类型 ID 列表 */
  packTypes: string[];
  /** 已选压缩包类型 ID，决定按普通压缩包还是整合包安装 */
  packType: string;
  /** 正在读压缩包条目 / 识别整合包类型 */
  scanning: boolean;
  /** 校验失败：压缩包必填（高亮 + 自动聚焦） */
  invalid?: boolean;
}>();

const emit = defineEmits<{
  /** 压缩包路径变化 */
  (e: "update:path", v: string): void;
  /** 点了「浏览」选压缩包 */
  (e: "pick"): void;
  /** 勾选 / 取消某个文件 */
  (e: "toggle-file", key: string): void;
  /** 勾选 / 取消整个目录，`on` 为 true 表示全选 */
  (e: "toggle-dir", node: FileNode, on: boolean): void;
  /** 展开 / 收起目录 */
  (e: "toggle-expand", key: string): void;
  /** 全选 / 全不选，`on` 为 true 表示全选 */
  (e: "set-all", on: boolean): void;
  /** 已选压缩包类型变化 */
  (e: "update:packType", v: string): void;
}>();

const pathInput = ref<HTMLInputElement | null>(null);

// 必填校验失败：聚焦路径框（高亮见模板的 is-invalid）
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
  <label class="field-label">{{ t("add.archive") }} <span class="req">*</span></label>
  <div class="path-row">
    <input ref="pathInput" :value="path" class="field-input" :class="{ 'is-invalid': invalid }"
      :placeholder="t('add.archivePlaceholder')" spellcheck="false" autocomplete="off"
      @input="emit('update:path', ($event.target as HTMLInputElement).value)" />
    <BaseButton class="pick-btn" size="sm" variant="accent" @click="emit('pick')">{{ t("add.browse") }}</BaseButton>
  </div>

  <!-- 读取条目 / 识别类型中：不确定滚动进度条，与加载器查询、目录扫描同一套观感 -->
  <LoaderQueryProgress v-if="scanning" kind="versions" :visible="true" :step="0" :total="0"
    :label="t('add.archiveReading')" />

  <template v-if="tree.length">
    <FileTreePanel :tree="tree" :checked="checked" :expanded="expanded" @toggle-file="emit('toggle-file', $event)"
      @toggle-dir="(n: FileNode, on: boolean) => emit('toggle-dir', n, on)"
      @toggle-expand="emit('toggle-expand', $event)" @set-all="emit('set-all', $event)" />

    <label class="field-label">{{ t("add.packType") }}</label>
    <select :value="packType" class="field-select"
      @change="emit('update:packType', ($event.target as HTMLSelectElement).value)">
      <option v-for="p in packTypes" :key="p" :value="p">{{ t(`add.pack.${p}`) }}</option>
    </select>
  </template>
  <!-- 还没选包时的空状态：说明这一步之后会发生什么 -->
  <p v-else class="field-hint">{{ t("add.archiveHint") }}</p>
</template>

<style scoped>
/* 卡片里第一条标签顶格，不留多余的 12px */
.field-label:first-child {
  margin-top: 0;
}

/* 这个「整合包类型」下拉在卡片最底部，下方常常没空间：默认的"优先向下、
   放不下才翻转"在这里没兜住（底下就是窗口边），弹层会被切掉。
   改成按可用空间挑方向（most-block-size），空间在下面就往下、在上面就往上。
   只作用于本页这一个下拉，forms.css 里那条全局的偏好不动 */
select.field-select::picker(select) {
  position-try-order: most-block-size;
}
</style>
