<script setup lang="ts">
// 存档列表：备份、还原、打开目录、删除；数据包是同一分类下的子页，见 DatapackPane
import { ref } from "vue";
import { t } from "../../../lib/i18n";
import { stripFormatting } from "../../../lib/formatting";
import { showToast } from "../../../lib/toast";
import {
  backupSave,
  deleteSave,
  listSaveBackups,
  restoreSaveBackup,
} from "../../../lib/api";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import SaveTabs from "./SaveTabs.vue";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { SaveBackupDto, SaveItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，存档列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化、确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { saves, instanceUuid, loading } = props.data;
const { busy, act, actLocal, askConfirm, askDelete, openFolder } = props.ops;

function remove(item: SaveItemDto) {
  // 确认框里用**去掉格式码**的名字：那里是纯文本，`§a` 只会显示成乱码
  askDelete(stripFormatting(item.levelName || item.dir), () =>
    deleteSave(instanceUuid.value, item.dir),
  );
}

/** 备份存档：后端返回备份出来的文件名 */
function backup(item: SaveItemDto) {
  act(async () => {
    const name = await backupSave(instanceUuid.value, item.dir);
    showToast(t("resource.backupOk", { name }));
  });
}

/**
 * 要还原的那个存档 + 它的备份列表，`null` 表示弹窗没开
 *
 * 列表在**点开弹窗时**才去后端取：备份数已经随存档列表给过来了，即 `item.backups`，
 * 所以"有没有备份"不用额外请求；具体文件列表只在真要还原时才需要。
 */
const restorePick = ref<{ item: SaveItemDto; list: SaveBackupDto[] } | null>(null);

function openRestore(item: SaveItemDto) {
  actLocal(async () => {
    const list = await listSaveBackups(instanceUuid.value, item.dir);
    if (!list.length) {
      showToast(t("resource.backupNone"));
      return;
    }
    restorePick.value = { item, list };
  });
}

/** 选中某个备份：先确认（**破坏性**），再还原并重拉列表 */
function pickRestore(backupFile: string) {
  const pick = restorePick.value;
  if (!pick) return;
  restorePick.value = null;
  const name = stripFormatting(pick.item.levelName || pick.item.dir);
  askConfirm(
    t("resource.restore"),
    t("resource.restoreConfirm", { name }),
    async () => {
      await act(async () => {
        await restoreSaveBackup(instanceUuid.value, pick.item.dir, backupFile);
      });
    },
    // act 里已经重拉过列表了
    { reload: false },
  );
}

/** 上次游玩时间；入参是 Unix 毫秒，未知时显示 -- */
function formatTime(ms: number): string {
  if (!ms) return "--";
  return new Date(ms).toLocaleString();
}

/** 备份体积，输入字节，输出人类可读文本 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <ContentHead :data="data">
    <SaveTabs :data="data" />
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <div v-else class="item-list">
    <ResourceRow v-for="item in saves" :key="item.dir" :icon="item.icon" :name="item.levelName || item.dir">
      <!-- 存档名可以带 `§` 格式码（地图作者常用来上色），过一遍显示期解析 -->
      <template #name>
        <FormattedText :text="item.levelName || item.dir" />
      </template>
      <template #badges>
        <span v-if="item.broken" class="badge badge-red">{{ t("resource.broken") }}</span>
        <span v-if="item.hardCore" class="badge badge-red">Hardcore</span>
        <!-- 有备份才给还原入口：没有备份的存档点还原只会弹一句"还没有备份" -->
        <span v-if="item.backups" class="badge">
          {{ t("resource.backupCount", { n: item.backups }) }}
        </span>
      </template>
      <template #sub>
        {{ item.dir }} · {{ t("resource.lastPlayed", { time: formatTime(item.lastPlayed) }) }}
      </template>
      <template #actions>
        <button class="mini-btn" :disabled="busy" @click="backup(item)">
          {{ t("resource.backup") }}
        </button>
        <button v-if="item.backups" class="mini-btn" :disabled="busy" @click="openRestore(item)">
          {{ t("resource.restore") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="openFolder('saves', item.dir)">
          {{ t("resource.openFolder") }}
        </button>
        <button class="mini-btn danger" :disabled="busy" @click="remove(item)">
          {{ t("resource.delete") }}
        </button>
      </template>
    </ResourceRow>
    <div v-if="!saves.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>

  <!-- 选一个备份来还原（破坏性，选中后还会再确认一次） -->
  <BaseModal v-if="restorePick" :title="t('resource.restorePickTitle')" :closable="false" @close="restorePick = null">
    <div class="backup-pick-list">
      <button v-for="b in restorePick.list" :key="b.file" class="backup-pick-row" :disabled="busy"
        @click="pickRestore(b.file)">
        <span class="backup-pick-name" :title="b.file">{{ b.file }}</span>
        <span class="backup-pick-meta">{{ formatTime(b.time) }} · {{ formatSize(b.size) }}</span>
      </button>
    </div>
    <div class="modal-actions">
      <BaseButton @click="restorePick = null">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<!-- 备份弹窗的样式走 scoped：BaseModal 是 Teleport 到 body 的，
     窗口级 resource.css 那套 .resource-layout 前缀够不着它（备注弹窗同理） -->
<style scoped>
.backup-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 备份可能很多：给个上限自己滚，别把弹窗撑出屏幕 */
  max-height: 320px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.backup-pick-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 11px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s;
}

.backup-pick-row:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.backup-pick-row:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 文件名可被挤压，时间 / 体积不缩 */
.backup-pick-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.backup-pick-meta {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
