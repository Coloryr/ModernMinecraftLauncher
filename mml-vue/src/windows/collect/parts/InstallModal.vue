<script setup lang="ts">
// 收藏项安装：选目标实例的覆盖弹窗
//
// 卡片上原来的小下拉换成了这个：多选可以一次装好几个，所以先对**支持加载器 / 支持游戏版本**
// 取交集 —— 交集之外的实例装进去也是坏的。没记支持信息的项（老收藏）当"不限"，不参与收窄。
import { computed, ref, watch } from "vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import { t } from "../../../lib/i18n";
import type { CollectItemDto, InstanceInfoDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要装的收藏项，至少一个 */
  items: CollectItemDto[];
  /** 全部实例，弹窗自己按交集筛 */
  instances: InstanceInfoDto[];
}>();

const emit = defineEmits<{
  /** 关掉，不装 */
  (e: "close"): void;
  /** 确认安装到该实例 */
  (e: "install", uuid: string): void;
}>();

/**
 * 若干份支持列表的交集
 *
 * 空列表表示"这项不限"，不参与收窄；全都不限时交集为空，界面按"不限"显示。
 *
 * - `lists`: 每项的支持列表
 *
 * # 返回值
 *
 * 交集；没有任何一项记了支持信息时返回空数组
 */
function intersect(lists: string[][]): string[] {
  const filled = lists.filter((list) => list.length > 0);
  if (!filled.length) return [];
  return filled.reduce((acc, cur) => acc.filter((value) => cur.includes(value)));
}

/** 选中项共同支持的加载器 */
const loaders = computed(() => intersect(props.items.map((item) => item.loaders)));
/** 选中项共同支持的游戏版本 */
const versions = computed(() => intersect(props.items.map((item) => item.gameVersions)));

/** 同时满足两个交集的实例 */
const candidates = computed(() =>
  props.instances.filter(
    (inst) =>
      (!loaders.value.length || loaders.value.includes(inst.loader)) &&
      (!versions.value.length || versions.value.includes(inst.version)),
  ),
);

/** 当前选中的实例 uuid */
const pick = ref("");

// 候选一变（刚打开、交集变了）就落到第一个能用的实例上
watch(
  candidates,
  (list) => {
    if (!list.some((inst) => inst.uuid === pick.value)) {
      pick.value = list[0]?.uuid ?? "";
    }
  },
  { immediate: true },
);

/** 某项自己的支持信息，一行显示；没记就写"不限" */
function requirement(item: CollectItemDto): string {
  const parts: string[] = [];
  if (item.loaders.length) parts.push(item.loaders.join(" / "));
  if (item.gameVersions.length) {
    const head = item.gameVersions.slice(0, 3).join(" / ");
    parts.push(item.gameVersions.length > 3 ? `${head} …` : head);
  }
  return parts.length ? parts.join(" · ") : t("collect.installAny");
}

/** 交集的显示文本：最多列三个版本，多的省略 */
function summary(list: string[]): string {
  if (!list.length) return t("collect.installAny");
  const head = list.slice(0, 3).join(" / ");
  return list.length > 3 ? `${head} …` : head;
}
</script>

<template>
  <BaseModal :title="t('collect.installTitle')" :closable="false" @close="emit('close')">
    <p class="inst-count">{{ t("collect.multiSelected", { count: items.length }) }}</p>
    <ul class="inst-items">
      <li v-for="item in items" :key="item.uuid" class="inst-item">
        <span class="inst-name" v-tip="item.name">{{ item.name }}</span>
        <span class="inst-need">{{ requirement(item) }}</span>
      </li>
    </ul>
    <p class="inst-line">{{ t("collect.installLoaders", { value: summary(loaders) }) }}</p>
    <p class="inst-line">{{ t("collect.installVersions", { value: summary(versions) }) }}</p>
    <template v-if="candidates.length">
      <label class="field-label inst-label">{{ t("collect.installTo") }}</label>
      <select v-model="pick" class="field-select">
        <option v-for="inst in candidates" :key="inst.uuid" :value="inst.uuid">
          {{ inst.name }} · {{ inst.version }}
        </option>
      </select>
    </template>
    <p v-else class="empty-tip">{{ t("collect.installNoFit") }}</p>
    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="accent" :disabled="!pick" @click="emit('install', pick)">
        {{ t("collect.installDo") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 选中项列表：最多六行高，多了自己滚，别把弹窗顶高 */
.inst-items {
  list-style: none;
  margin: 8px 0;
  padding: 0;
  max-height: 132px;
  overflow-y: auto;
}

.inst-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 2px 0;
  font-size: 12.5px;
}

.inst-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 支持信息是次要信息，比名字低一档 */
.inst-need {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

.inst-count,
.inst-line {
  margin: 4px 0;
  font-size: 12.5px;
  color: var(--text-dim);
}

.inst-label {
  margin-top: 10px;
}
</style>
