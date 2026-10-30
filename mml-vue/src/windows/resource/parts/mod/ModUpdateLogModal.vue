<script setup lang="ts">
// 模组更新记录弹窗：按**项目**分组列出这个实例换过的模组，每组是同一个项目的更新历史，新的在前
//
// 分组与排序都在后端做（表就是按项目编号索引的），这里只画与筛：
// - 搜索按**模组名或项目号**筛分组；
// - 筛选只有**更新日期范围**一项，筛的是记录，落在范围外的记录不显示、整组没了就整组不显示；
// - 右键某个组头 = 只看这一个模组，靠上方那条提示或再右键一次退回来。
//
// 记录由后端在换文件成功时追加（见实例目录的 mod_update.json），这里只读与删。
import { computed, onMounted, ref, watch } from "vue";
import { t, tErr } from "../../../../lib/i18n";
import { showToast } from "../../../../lib/toast";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import HighlightText from "../../../../components/ui/HighlightText.vue";
import SearchBox from "../../../../components/ui/SearchBox.vue";
import BaseDatePicker from "../../../../components/ui/BaseDatePicker.vue";
import type { ModUpdateProjectDto } from "../../../../lib/bindings";
import { formatTime } from "../../../../lib/format";

const props = defineProps<{
  /** 取分组好的记录，组内与组间都是新的在前；失败抛错 */
  load: () => Promise<ModUpdateProjectDto[]>;
  /**
   * 删掉一条记录
   *
   * - `projectId`: 项目编号
   * - `index`: 该项目内的下标，按 `load` 返回的顺序，即 0 是最近那一条
   */
  remove: (projectId: string, index: number) => Promise<void>;
  /** 进来就只看这一个项目；空串或缺省表示看全部分组 */
  initialProject?: string;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
}>();

/** 分组好的记录，组内与组间都是新的在前 */
const groups = ref<ModUpdateProjectDto[]>([]);
/** 正在取 */
const loading = ref(false);
/** 正在删一条：期间把删除按钮都禁掉，免得连点 */
const busy = ref(false);
/** 出错文案；非空时列表位置只显示它 */
const error = ref("");
/** 搜索词，按模组名或项目号匹配，大小写不敏感 */
const keyword = ref("");
/** 只看这一个项目；空串表示不设限 */
const onlyProjectId = ref(props.initialProject ?? "");
/** 更新日期的起始日，`YYYY-MM-DD`；空串表示这一端不限 */
const from = ref("");
/** 更新日期的结束日，`YYYY-MM-DD`；空串表示这一端不限 */
const to = ref("");

watch(keyword, (value) => {
  // 搜索框一输入就把"只看某个项目"收掉：两个条件同时生效会互相打架
  if (value.trim()) onlyProjectId.value = "";
});

onMounted(async () => {
  loading.value = true;
  try {
    groups.value = await props.load();
  } catch (e) {
    error.value = tErr(e);
  } finally {
    loading.value = false;
  }
});

/**
 * 日期输入的 `YYYY-MM-DD` 转那一刻的毫秒
 *
 * - `day`: 日期文本；`end` 为真取当天最后一刻，否则取当天 0 点
 *
 * # 返回值
 *
 * 毫秒时间戳；`day` 为空串时返回 0，调用方按"这一端不限"处理
 */
function dayTime(day: string, end: boolean): number {
  if (!day) return 0;
  return new Date(`${day}T${end ? "23:59:59.999" : "00:00:00"}`).getTime();
}

/** 搜索、日期范围与"只看一个项目"都过一遍之后要显示的分组 */
const visibleGroups = computed<ModUpdateProjectDto[]>(() => {
  const key = keyword.value.trim().toLowerCase();
  const start = dayTime(from.value, false);
  const end = dayTime(to.value, true);

  return groups.value
    .filter((group) => {
      if (onlyProjectId.value) return group.projectId === onlyProjectId.value;
      if (!key) return true;
      return (
        group.name.toLowerCase().includes(key) ||
        group.projectId.toLowerCase().includes(key)
      );
    })
    .map((group) => ({
      ...group,
      update: group.update.filter((item) => {
        if (start && item.time < start) return false;
        if (end && item.time > end) return false;
        return true;
      }),
    }))
    .filter((group) => group.update.length > 0);
});

/** 筛完之后还剩多少条记录，表头那句"共 N 条"用 */
const totalCount = computed(() =>
  visibleGroups.value.reduce((sum, group) => sum + group.update.length, 0),
);

/** 当前"只看"的那个模组名，提示条上显示；查不到时退回项目号 */
const onlyName = computed(() => {
  const group = groups.value.find((item) => item.projectId === onlyProjectId.value);
  return group ? group.name || group.projectId : onlyProjectId.value;
});

/** 有没有任何筛选在生效：搜索、日期范围、只看某个项目，用来决定显不显示「清除筛选」 */
const hasFilter = computed(
  () => !!keyword.value || !!from.value || !!to.value || !!onlyProjectId.value,
);

/** 清掉全部筛选，回到完整列表 */
function clearFilters() {
  keyword.value = "";
  from.value = "";
  to.value = "";
  onlyProjectId.value = "";
}

/** 收起的组，键是项目编号；默认全展开 —— 记录本来就不多，摊开省一次点击 */
const collapsed = ref<Set<string>>(new Set());

/**
 * 这一组收着没
 *
 * - `projectId`: 项目编号
 *
 * # 返回值
 *
 * 收着返回 true
 */
function isCollapsed(projectId: string): boolean {
  return collapsed.value.has(projectId);
}

/**
 * 收起 / 展开一组
 *
 * - `projectId`: 项目编号
 */
function toggleGroup(projectId: string) {
  const next = new Set(collapsed.value);
  if (next.has(projectId)) {
    next.delete(projectId);
  } else {
    next.add(projectId);
  }
  collapsed.value = next;
}

/**
 * 右键组头：只看这一个模组
 *
 * 再右键同一个组是取消；换一个组就切过去。退出还可以点提示条上的 ✕。
 *
 * - `projectId`: 项目编号
 */
function onlyProject(projectId: string) {
  const next = onlyProjectId.value === projectId ? "" : projectId;
  onlyProjectId.value = next;
  // 这一档不显示搜索框，把搜索词一并清掉，免得退回来时列表还带着看不见的筛选
  if (next) keyword.value = "";
}

/**
 * 删掉一条记录，删完重读整份
 *
 * 重读而不是就地删：下标是"该项目里的位置"，就地删要自己维护偏移，重读最省心，
 * 也顺手把删空的项目收掉。
 *
 * - `projectId`: 项目编号
 * - `index`: 该项目内的下标，即 0 是最近那一条
 */
async function removeEntry(projectId: string, index: number) {
  if (busy.value) return;
  busy.value = true;
  try {
    await props.remove(projectId, index);
    groups.value = await props.load();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    busy.value = false;
  }
}

/**
 * 空值兜底
 *
 * - `value`: 记录里的字段；旧版本号与旧文件号取不到时是空串
 *
 * # 返回值
 *
 * 非空时原样返回，空串返回 `--`
 */
function dash(value: string): string {
  return value || "--";
}

/**
 * SHA1 只露前 12 位，完整值挂在悬停提示里
 *
 * - `value`: 记录里的 SHA1
 *
 * # 返回值
 *
 * 截断后的文本；短于 12 位时原样返回
 */
function shortSha1(value: string): string {
  return value.length > 12 ? `${value.slice(0, 12)}…` : value;
}
</script>

<template>
  <BaseModal :title="t('resource.updateLog')" :width="560" fixed-height="520px" :closable="false"
    @close="emit('close')">
    <p v-if="error" class="error-text">{{ error }}</p>
    <p v-else-if="!groups.length" class="empty-tip">
      {{ loading ? t("resource.updateLogLoading") : t("resource.updateLogEmpty") }}
    </p>

    <template v-else>
      <!--
        搜索按名字，筛选只有更新日期范围这一项。
        **只看一个模组时搜索框整个不出现**：搜的就是模组名，一组里只有一个模组，
        搜了只会让人以为还能再筛；日期范围照旧，那一栏筛的是记录
      -->
      <div class="log-filter">
        <div v-if="!onlyProjectId" class="log-search">
          <SearchBox v-model="keyword" :placeholder="t('resource.updateLogSearch')" width="100%" />
        </div>
        <BaseDatePicker v-model="from" class="log-date" :placeholder="t('resource.updateLogDateFrom')"
          :max="to" :aria-label="t('resource.updateLogDateFrom')" />
        <span class="log-sep">–</span>
        <BaseDatePicker v-model="to" class="log-date" :placeholder="t('resource.updateLogDateTo')"
          :min="from" :aria-label="t('resource.updateLogDateTo')" />
      </div>

      <!-- 只看一个模组时给一条可关掉的提示，否则用户不知道列表为什么只剩一组 -->
      <p v-if="onlyProjectId" class="log-only">
        <span>{{ t("resource.updateLogOnly", { name: onlyName }) }}</span>
        <button class="log-only-x" :aria-label="t('resource.cancel')" @click="onlyProjectId = ''">
          <GlyphIcon name="close" :size="12" :weight="2.4" />
        </button>
      </p>

      <div class="log-summary">
        <p class="log-count">{{ t("resource.updateLogCount", { n: totalCount }) }}</p>
        <button v-if="hasFilter" class="log-clear" @click="clearFilters">
          {{ t("resource.updateLogClear") }}
        </button>
      </div>
      <p v-if="!visibleGroups.length" class="empty-tip">{{ t("resource.updateLogNoMatch") }}</p>

      <div v-else class="log-list">
        <section v-for="group in visibleGroups" :key="group.projectId" class="log-group">
          <!-- 组头整行可点：箭头 + 模组名（没记过就退回项目编号）+ 该组的条数；右键只看这一个 -->
          <button class="log-group-head" :class="{ collapsed: isCollapsed(group.projectId) }"
            @click="toggleGroup(group.projectId)" @contextmenu.prevent="onlyProject(group.projectId)">
            <GlyphIcon name="chevron-down" :size="12" class="log-group-caret" />
            <span class="log-group-name">
              <HighlightText :text="group.name || group.projectId" :query="keyword" />
            </span>
            <span class="log-group-count">{{ t("resource.updateLogGroupCount", { n: group.update.length }) }}</span>
          </button>

          <!-- 每条记录两行：上行是「版本跃迁 + 文件名 + 时间」，下行是文件号与 SHA1 两枚小标签。
               左侧一条时间轴把同一模组的更新按时间串起来，比一叠卡片更容易看出前后顺序 -->
          <div v-if="!isCollapsed(group.projectId)" class="log-rows">
            <div v-for="(item, index) in group.update" :key="index" class="log-row">
              <span class="log-dot"></span>
              <div class="log-head">
                <span class="log-ver">{{ t("resource.updateLogVersion", { from: dash(item.oldVersion), to: dash(item.version) }) }}</span>
                <span class="log-file">{{ item.file }}</span>
                <span class="log-right">
                  <span class="log-time">{{ formatTime(item.time) }}</span>
                  <button class="log-del" :disabled="busy" v-tip="t('resource.updateLogDelete')"
                    :aria-label="t('resource.updateLogDelete')" @click="removeEntry(group.projectId, index)">
                    <GlyphIcon name="trash" :size="12" />
                  </button>
                </span>
              </div>
              <div class="log-meta">
                <span class="log-chip">
                  {{ t("resource.updateLogFileId", { from: dash(item.oldFileId), to: dash(item.fileId) }) }}
                </span>
                <span class="log-chip mono" v-tip="item.sha1">
                  {{ t("resource.updateLogSha1") }} {{ shortSha1(item.sha1) }}
                </span>
              </div>
            </div>
          </div>
        </section>
      </div>
    </template>

    <div class="modal-actions">
      <BaseButton @click="emit('close')">{{ t("resource.close") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 弹窗在 body 下，样式走 scoped（与 ModReplaceModal / ModUpdateModal 同一处理） */

/* 搜索 + 日期范围一行摆下：搜索吃掉剩余宽度，两个日期是固定宽 */
.log-filter {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}

.log-search {
  flex: 1;
  min-width: 0;
}

.log-date {
  width: 132px;
  flex-shrink: 0;
}

.log-sep {
  color: var(--text-dim);
  flex-shrink: 0;
}

/* 只看一个模组时的提示条 */
.log-only {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 8px;
  padding: 5px 8px;
  border: 1px solid var(--accent-border);
  border-radius: var(--ctl-radius);
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 12px;
}

.log-only-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.log-only-x:hover {
  background: var(--bg-hover);
}

/* 统计行：「共 N 条」在左，有筛选时右边补一个「清除筛选」 */
.log-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.log-count {
  margin: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

.log-clear {
  padding: 2px 6px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--accent);
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
  transition: background 0.12s;
}

.log-clear:hover {
  background: var(--bg-hover);
}

/* 加载中 / 出错 / 空 / 筛完没结果：这几档没有可滚的列表，让提示吃掉剩余高度并居中，
   动作按钮才会贴底，而不是浮在面板中间 */
.modal-body > .empty-tip,
.modal-body > .error-text {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 弹窗高度被 fixed-height 锁死，这里吃掉剩余高度自己滚 ——
   **不能再给 max-height**：那样面板与列表会各滚一层，右边出现两条滚动条 */
.log-list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 */
  margin-right: -8px;
  padding-right: 8px;
}

/* 组头：箭头 + 模组名 + 条数；点整行收起 / 展开，右键只看这一个 */
.log-group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  /* 左右各让 6px：悬停底色有边距；下方卡片按同样的起点缩进，与组名对齐 */
  padding: 4px 6px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  text-align: left;
  transition: background 0.12s;
}

.log-group-head:hover {
  background: var(--bg-hover);
}

.log-group-caret {
  transition: transform 0.12s;
}

.log-group-head.collapsed .log-group-caret {
  transform: rotate(-90deg);
}

.log-group-name {
  word-break: break-all;
}

.log-group-count {
  flex-shrink: 0;
  margin-left: auto;
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 400;
}

/* 时间轴：一条竖线把同一模组的更新串起来，每条记录左缘一枚圆点。
   26px 的左边距与组名对齐（组头 6px 内边距 + 12px 箭头 + 8px 间距） */
.log-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
  position: relative;
  padding: 8px 0 2px 26px;
}

.log-rows::before {
  content: "";
  position: absolute;
  /* 与下面圆点的圆心对齐：26px 左内边距 - 16px 偏移 + 4px 半径 */
  left: 13px;
  top: 14px;
  bottom: 14px;
  width: 2px;
  border-radius: 1px;
  background: var(--border);
}

.log-row {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-card);
}

/* 圆点压在时间轴上，用强调色描一圈底色把它与竖线分开 */
.log-dot {
  position: absolute;
  left: -16px;
  top: 14px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 3px var(--bg-card);
}

.log-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 3px 8px;
}

/* 版本跃迁是本条记录的主信息：底色淡强调、等宽数字，扫一眼就能看出换了哪一版 */
.log-ver {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11.5px;
  font-weight: 600;
  white-space: nowrap;
}

.log-file {
  flex: 1;
  min-width: 0;
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

/* 时间与删除按钮摆一起，一起贴右边 */
.log-right {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  flex-shrink: 0;
}

.log-time {
  color: var(--text-dim);
  font-size: 11.5px;
}

.log-del {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.log-del:hover:not(:disabled) {
  background: color-mix(in srgb, var(--red) 14%, transparent);
  color: var(--red);
}

.log-del:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 文件号与 SHA1：两枚小标签，长哈希截断，完整值在悬停提示里 */
.log-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  color: var(--text-dim);
  font-size: 11.5px;
}

.log-chip {
  padding: 1px 6px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-side);
  word-break: break-all;
}

.log-chip.mono {
  font-family: monospace;
}
</style>
