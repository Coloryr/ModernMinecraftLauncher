<script setup lang="ts">
// 更新下载弹窗：一条一个可更新的模组，显示"当前版本 → 最新版本"，左边勾选要下哪几个。
//
// 只负责画与勾选：检查由 ModPane 触发、整份结果传进来，点「下载」把勾中的条目抛回去。
// 查询失败的个数在底部如实说明 —— 没查出来的不等于"已是最新"。
import { computed, ref } from "vue";
import { t } from "../../../../lib/i18n";
import VirtualList from "../../../../components/ui/VirtualList.vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { ModUpdateCheckDto, ModUpdateDto } from "../../../../lib/bindings";
import { isoDate } from "../../types";

const props = defineProps<{
  /** 检查结果，updates 非空时才开这个弹窗 */
  result: ModUpdateCheckDto;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 点「下载」：把勾选的条目抛给 ModPane，它负责换算成下载参数 */
  (e: "download", items: ModUpdateDto[]): void;
}>();

/** 有新版本的条目 */
const updates = computed(() => props.result.updates);

/**
 * 勾选中的条目 uuid
 *
 * 默认**全勾**：点「更新检查」的人本来就是想更新，全不勾等于白点一次；
 * 要留哪一个再手动取消即可。用 uuid 作键，与列表行同一个身份。
 */
const checked = ref<Set<string>>(new Set(updates.value.map((item) => item.uuid)));

/** 这一条勾着没 */
function isChecked(item: ModUpdateDto): boolean {
  return checked.value.has(item.uuid);
}

/** 勾选 / 取消这一条 */
function toggle(item: ModUpdateDto) {
  const next = new Set(checked.value);
  if (next.has(item.uuid)) {
    next.delete(item.uuid);
  } else {
    next.add(item.uuid);
  }
  checked.value = next;
}

/** 全勾了 */
const allChecked = computed(
  () => updates.value.length > 0 && checked.value.size === updates.value.length,
);

/** 勾了一部分：全选框显示成半选 */
const someChecked = computed(() => checked.value.size > 0 && !allChecked.value);

/** 全选 / 全不选 */
function toggleAll() {
  checked.value = allChecked.value ? new Set() : new Set(updates.value.map((item) => item.uuid));
}

/** 点「下载」：把勾中的条目抛回去，换算与下载都在 ModPane */
function apply() {
  const picked = updates.value.filter(isChecked);
  if (picked.length) emit("download", picked);
}

</script>

<template>
  <BaseModal :title="t('resource.updateTitle')" :width="640" :closable="false" @close="emit('close')">
    <!-- 全选 + 已选计数：条目可能几十个，逐个点太慢 -->
    <div class="upd-toolbar mb-2 flex items-center justify-between gap-2.5">
      <label class="upd-all flex cursor-pointer items-center gap-[7px] text-body text-fg">
        <input type="checkbox" class="upd-check m-0 h-[15px] w-[15px] shrink-0 cursor-pointer accent-accent" :checked="allChecked" :indeterminate="someChecked"
          @change="toggleAll" />
        <span>{{ t("resource.selectAll") }}</span>
      </label>
      <span class="upd-count text-meta text-dim">{{ t("resource.selectedCount", { n: checked.size }) }}</span>
    </div>

    <!-- 更新条数可能上百：接虚拟列表，只渲染视口附近那几行；行高不齐（名字会换行），量第一行当估值 -->
    <VirtualList class="upd-list mr-[-8px] flex max-h-[52vh] flex-col gap-2 overflow-y-auto pr-2" :count="updates.length" :row-gap="8" measure-row>
      <template #default="{ start, end }">
        <label v-for="u in updates.slice(start, end)" :key="u.uuid" class="upd-row flex cursor-pointer items-start gap-2.5 rounded-ctl border border-border bg-card px-2.5 py-2"
          :class="{ 'upd-row-on border-accent-border bg-accent-soft': isChecked(u) }">
          <input type="checkbox" class="upd-check m-0 h-[15px] w-[15px] shrink-0 cursor-pointer accent-accent" :checked="isChecked(u)" @change="toggle(u)" />
          <span class="upd-body flex min-w-0 flex-col">
            <span class="upd-head flex flex-wrap items-baseline gap-x-2.5 gap-y-[3px]">
              <span class="upd-name break-all text-body font-semibold text-fg">{{ u.name || u.file }}</span>
              <span v-if="u.disable" class="upd-tag rounded-[6px] border border-border px-1.5 text-meta text-dim">{{ t("resource.modDisabled") }}</span>
            </span>
            <span class="upd-meta mt-[3px] flex flex-wrap items-baseline gap-1.5 text-body text-dim">
              <span>{{ t("resource.updateCurrent", { version: u.version || u.file }) }}</span>
              <span class="upd-arrow">→</span>
              <span class="upd-new text-accent">{{ t("resource.updateNew", { version: u.newVersion || u.newFile }) }}</span>
            </span>
            <span class="upd-note mt-[3px] flex flex-wrap gap-x-2.5 gap-y-[3px] break-all text-meta text-dim">
              <span class="upd-file">{{ u.file }} → {{ u.newFile }}</span>
              <span v-if="u.date">{{ t("resource.updateDate", { date: isoDate(u.date) }) }}</span>
            </span>
          </span>
        </label>
      </template>
    </VirtualList>

    <!-- 只在有查询失败时留一行：没查出来的不等于"已是最新"，这行不能省 -->
    <p v-if="props.result.failed" class="upd-foot m-0 mt-2.5 text-meta text-dim">
      {{ t("resource.updateFailed", { n: props.result.failed }) }}
    </p>

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="accent" :disabled="!checked.size" @click="apply">
        {{ t("resource.updateStart") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

