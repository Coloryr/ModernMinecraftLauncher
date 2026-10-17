<script setup lang="ts">
// 更新下载弹窗：一条一个可更新的模组，显示"当前版本 → 最新版本"，左边勾选要下哪几个。
//
// 只负责画与勾选：检查由 ModPane 触发、整份结果传进来，点「下载」把勾中的条目抛回去。
// 查询失败的个数在底部如实说明 —— 没查出来的不等于"已是最新"。
import { computed, ref } from "vue";
import { t } from "../../../../lib/i18n";
import VirtualList from "../VirtualList.vue";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { ModUpdateCheckDto, ModUpdateDto } from "../../../../lib/bindings";

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

/**
 * 发布时间只取日期那一段
 *
 * - `iso`: ISO 8601 字符串，形如 `2024-03-01T12:00:00Z`
 *
 * # 返回值
 *
 * `2024-03-01`；不是这个形态时原样返回
 *
 * 不引入日期解析：接口给的一定是 ISO，切前 10 位就是日期，还省掉时区换算。
 */
function dateOf(iso: string): string {
  return /^\d{4}-\d{2}-\d{2}/.test(iso) ? iso.slice(0, 10) : iso;
}
</script>

<template>
  <BaseModal :title="t('resource.updateTitle')" :width="640" :closable="false" @close="emit('close')">
    <!-- 全选 + 已选计数：条目可能几十个，逐个点太慢 -->
    <div class="upd-toolbar">
      <label class="upd-all">
        <input type="checkbox" class="upd-check" :checked="allChecked" :indeterminate="someChecked" @change="toggleAll" />
        <span>{{ t("resource.selectAll") }}</span>
      </label>
      <span class="upd-count">{{ t("resource.selectedCount", { n: checked.size }) }}</span>
    </div>

    <!-- 更新条数可能上百：接虚拟列表，只渲染视口附近那几行；行高不齐（名字会换行），量第一行当估值 -->
    <VirtualList class="upd-list" :count="updates.length" :row-gap="8" measure-row>
      <template #default="{ start, end }">
        <label v-for="u in updates.slice(start, end)" :key="u.uuid" class="upd-row" :class="{ 'upd-row-on': isChecked(u) }">
          <input type="checkbox" class="upd-check" :checked="isChecked(u)" @change="toggle(u)" />
          <span class="upd-body">
            <span class="upd-head">
              <span class="upd-name">{{ u.name || u.file }}</span>
              <span v-if="u.disable" class="upd-tag">{{ t("resource.modDisabled") }}</span>
            </span>
            <span class="upd-meta">
              <span>{{ t("resource.updateCurrent", { version: u.version || u.file }) }}</span>
              <span class="upd-arrow">→</span>
              <span class="upd-new">{{ t("resource.updateNew", { version: u.newVersion || u.newFile }) }}</span>
            </span>
            <span class="upd-note">
              <span class="upd-file">{{ u.file }} → {{ u.newFile }}</span>
              <span v-if="u.date">{{ t("resource.updateDate", { date: dateOf(u.date) }) }}</span>
            </span>
          </span>
        </label>
      </template>
    </VirtualList>

    <!-- 只在有查询失败时留一行：没查出来的不等于"已是最新"，这行不能省 -->
    <p v-if="props.result.failed" class="upd-foot">
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

<style scoped>
/* 弹窗在 body 下，样式走 scoped（与 ModDepsModal 同一处理） */
.upd-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.upd-all {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}

/* 勾选框：原生控件 + accent-color 跟主题走（弹窗在 body 下，拿不到窗口级那套样式） */
.upd-check {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  margin: 0;
  accent-color: var(--accent);
  cursor: pointer;
}

.upd-count {
  color: var(--text-dim);
  font-size: 11.5px;
}

.upd-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 52vh;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行的右边缘仍落在弹窗内容区里，滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

/* 一行一条：整行是 label，点哪儿都能勾 */
.upd-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-card);
  cursor: pointer;
}

/* 勾中的那条给主色边框：一屏十几条时一眼看得出选了哪些 */
.upd-row-on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.upd-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.upd-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 3px 10px;
}

.upd-name {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

.upd-tag {
  padding: 0 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-dim);
  font-size: 11px;
}

.upd-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px;
  margin-top: 3px;
  color: var(--text-dim);
  font-size: 12px;
}

/* 新版本是这一行的重点，给主色；旧版本与箭头保持次要 */
.upd-new {
  color: var(--accent);
}

.upd-note {
  display: flex;
  flex-wrap: wrap;
  gap: 3px 10px;
  margin-top: 3px;
  color: var(--text-dim);
  font-size: 11.5px;
  word-break: break-all;
}

.upd-foot {
  margin: 10px 0 0;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
