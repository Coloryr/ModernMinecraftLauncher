<script setup lang="ts">
// 替换版本弹窗：列出这个模组在平台上的版本，点一条就把当前 jar 换成那一份
//
// 只负责拉列表与选择：过滤口径（当前实例的游戏版本 + 加载器）与真正换文件都在 ModPane ——
// 取数走它传进来的 `load`，选中后抛回 `pick`，换文件复用「更新下载」那一条链路。
import { onMounted, ref } from "vue";
import { t, tErr } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { FileListDto, FileListItemDto, ModItemDto } from "../../../../lib/bindings";
import { formatSize } from "../../../../lib/format";
import { isoDate } from "../../types";

const props = defineProps<{
  /** 要换版本的那个模组 */
  target: ModItemDto;
  /** 取某一页版本；入参是目标模组与页码，从 0 起，失败抛错 */
  load: (item: ModItemDto, page: number) => Promise<FileListDto>;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 选中某一版：把那条文件抛回 ModPane */
  (e: "pick", chosen: FileListItemDto): void;
}>();

/** 已取到的版本，按平台返回顺序，一般是新的在前 */
const list = ref<FileListItemDto[]>([]);
/** 已经取到第几页，从 0 起 */
const page = ref(0);
/** 还有下一页 */
const maxPage = ref(0);
/** 正在取：首屏与「加载更多」共用 */
const loading = ref(false);
/** 出错文案；非空时列表位置只显示它 */
const error = ref("");

/**
 * 取一页
 *
 * - `more`: 点「加载更多」为真，追加到现有列表；首屏为假，整份换掉
 */
async function fetchPage(more: boolean) {
  if (loading.value) return;
  loading.value = true;
  error.value = "";
  const next = more ? page.value + 1 : 0;
  try {
    const res = await props.load(props.target, next);
    list.value = more ? [...list.value, ...res.list] : res.list;
    maxPage.value = res.maxPage;
    page.value = next;
  } catch (e) {
    error.value = tErr(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => fetchPage(false));

/** 这一条就是当前装着的那份，换上没意义 */
function isCurrent(item: FileListItemDto): boolean {
  return item.isDownload;
}

/** 点一行：当前那一份不给换 */
function pick(item: FileListItemDto) {
  if (isCurrent(item)) return;
  emit("pick", item);
}

</script>

<template>
  <BaseModal :title="t('resource.replaceVersion')" :width="560" fixed-height="520px" :closable="false"
    @close="emit('close')">
    <p class="rep-title m-0 break-all text-title font-semibold text-fg">{{ target.name || target.file }}</p>
    <p v-if="target.version" class="rep-sub mb-0 mt-[3px] text-body text-dim">{{ t("resource.updateCurrent", { version: target.version }) }}</p>

    <p v-if="error" class="error-text">{{ error }}</p>
    <p v-else-if="!list.length" class="empty-tip">
      {{ loading ? t("resource.replaceWait") : t("resource.replaceEmpty") }}
    </p>

    <div v-else class="rep-list mr-[-8px] mt-3 flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto pr-2">
      <button v-for="item in list" :key="item.source.fid" class="rep-row flex w-full cursor-pointer items-start gap-2.5 rounded-ctl border border-border bg-card px-2.5 py-2 text-left text-fg transition-all duration-100 [font-family:inherit] enabled:hover:border-accent enabled:hover:bg-accent-soft"
        :class="{ 'rep-row-current cursor-default opacity-60': isCurrent(item) }" :disabled="isCurrent(item)" @click="pick(item)">
        <span class="rep-body flex min-w-0 flex-col">
          <span class="rep-head flex flex-wrap items-baseline gap-x-2.5 gap-y-[3px]">
            <span class="rep-name break-all text-body font-semibold text-fg">{{ item.name }}</span>
            <span v-if="isCurrent(item)" class="rep-tag rounded-[6px] border border-accent-border px-1.5 text-meta text-accent">{{ t("resource.replaceCurrent") }}</span>
          </span>
          <span class="rep-meta mt-[3px] flex flex-wrap gap-x-2.5 gap-y-[3px] text-meta text-dim">
            <span v-if="item.time">{{ t("resource.updateDate", { date: isoDate(item.time) }) }}</span>
            <span v-if="item.size > 0">{{ formatSize(item.size) }}</span>
          </span>
        </span>
      </button>
    </div>

    <div v-if="!error && page < maxPage" class="rep-more mt-2.5 text-center">
      <BaseButton size="sm" :disabled="loading" @click="fetchPage(true)">
        {{ t("resource.replaceMore") }}
      </BaseButton>
    </div>

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 弹窗在 body 下，样式走 scoped（与 ModUpdateModal / ModDepsPage 同一处理）。
   排版全在模板上的工具类里；这里只留这条对 BaseModal 内 .modal-body 子元素的规则，
   让加载 / 出错提示吃掉剩余高度，动作按钮才贴底。 */

.modal-body>.empty-tip,
.modal-body>.error-text {
  flex: 1;
}
</style>
