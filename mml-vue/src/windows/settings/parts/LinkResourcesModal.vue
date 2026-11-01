<script setup lang="ts">
// 已下载资源列表：公共目录里有哪些模组 / 资源包，以及它们还占多少空间
//
// 数据来自启动器的资源索引，文件被删掉的也列出来并标灰，方便发现残留记录。
import { computed, onMounted, ref } from "vue";
import { t } from "../../../lib/i18n";
import { commands, type LinkResourceDto } from "../../../lib/bindings";
import { formatSize } from "../../../lib/format";
import { showToast } from "../../../lib/toast";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";

const emit = defineEmits<{
  /** 关闭弹窗 */
  (e: "close"): void;
}>();

/** 全部记录 */
const items = ref<LinkResourceDto[]>([]);
/** 正在读取 */
const loading = ref(true);
/** 搜索词，匹配名字与文件名 */
const keyword = ref("");

/** 过滤后的列表 */
const visible = computed(() => {
  const word = keyword.value.trim().toLowerCase();
  if (!word) return items.value;
  return items.value.filter(
    (item) =>
      item.name.toLowerCase().includes(word) || item.file.toLowerCase().includes(word),
  );
});

/** 合计大小：只算还在磁盘上的 */
const totalSize = computed(() =>
  items.value.filter((item) => item.exists).reduce((sum, item) => sum + item.size, 0),
);

/** 列表顶部那行统计 */
const headText = computed(() =>
  t("winSettings.resourceLinkListCount", {
    count: items.value.length,
    size: formatSize(totalSize.value),
  }),
);

onMounted(async () => {
  try {
    items.value = await commands.settings.listLinkResources();
  } catch (e) {
    showToast(String(e));
  } finally {
    loading.value = false;
  }
});

/**
 * 在资源管理器里定位一份资源
 *
 * - `item`: 列表里的一行
 */
async function openItem(item: LinkResourceDto) {
  try {
    await commands.settings.openLinkResource(item.kind, item.file);
  } catch (e) {
    showToast(String(e));
  }
}

/**
 * 类型标签文案
 *
 * - `kind`: `mod` / `resourcepack`
 *
 * # 返回值
 *
 * 本地化后的类型名
 */
function kindLabel(kind: string): string {
  return kind === "mod" ? t("winSettings.resourceLinkKindMod") : t("winSettings.resourceLinkKindPack");
}

/**
 * 哈希缩略：前 8 位就够区分，完整值放 tooltip
 *
 * - `hash`: 文件 SHA1
 *
 * # 返回值
 *
 * 前 8 位
 */
function shortHash(hash: string): string {
  return hash.slice(0, 8);
}
</script>

<template>
  <BaseModal :title="t('winSettings.resourceLinkListTitle')" :width="640" @close="emit('close')">
    <div class="link-res-head">
      <span class="link-res-count">{{ headText }}</span>
      <input v-model="keyword" class="field-input link-res-search" :placeholder="t('winSettings.resourceLinkListSearch')"
        spellcheck="false" />
    </div>
    <div v-if="loading" class="empty-tip">{{ t("winSettings.resourceLinkListLoading") }}</div>
    <div v-else-if="visible.length === 0" class="empty-tip">{{ t("winSettings.resourceLinkListEmpty") }}</div>
    <div v-else class="link-res-list">
      <div v-for="item in visible" :key="item.kind + item.hash" class="link-res-row"
        :class="{ missing: !item.exists }">
        <span class="link-res-kind">{{ kindLabel(item.kind) }}</span>
        <span class="link-res-name" v-tip="item.file">{{ item.name || item.file }}</span>
        <span class="link-res-hash" v-tip="item.hash">{{ shortHash(item.hash) }}</span>
        <span class="link-res-size">{{ item.exists ? formatSize(item.size) : t("winSettings.resourceLinkListMissing") }}</span>
        <BaseButton size="sm" @click="openItem(item)">{{ t("winSettings.resourceLinkListOpen") }}</BaseButton>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
.link-res-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}

.link-res-count {
  flex: 1;
  font-size: 11.5px;
  color: var(--text-dim);
}

.link-res-search {
  width: 220px;
}

.link-res-list {
  display: flex;
  flex-direction: column;
  max-height: 52vh;
  overflow-y: auto;
}

.link-res-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 4px;
  border-bottom: 1px solid var(--border);
}

.link-res-row.missing {
  opacity: 0.55;
}

.link-res-kind {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 10.5px;
}

.link-res-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
}

.link-res-hash {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-dim);
}

.link-res-size {
  flex-shrink: 0;
  width: 88px;
  text-align: right;
  font-size: 11.5px;
  color: var(--text-dim);
}
</style>
