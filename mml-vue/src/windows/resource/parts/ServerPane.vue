<script setup lang="ts">
// 服务器列表（servers.dat）：添加 / 编辑 / 删除、打开目录 + **每行的连接状态**
//
// 添加/编辑的表单状态属于本分类，就放在这一层（草稿 + 保存中标记），
// 弹窗排版在 ServerFormModal。
//
// MOTD 分成两处，与主窗口的 MOTD 卡同一套观感：
// - **列表上方一张卡**，显示当前**选中**那一台的完整 MOTD：彩色分段、保留换行、人数/版本/延迟；
//   点列表里的行切换。卡片是 `components/MotdCard.vue`，与主窗口那两张是同一个组件。
// - **行里只说"能不能连上"** + 地址 —— 一行一句话，扫一眼就知道哪台连得上。
//
// MOTD 查询也是这一层：进分类 / 刷新列表时**并发查全部服务器**，结果填进对应那一行。
// 并发是必须的 —— 连不上的服要等到超时，串行的话后面的行全被它堵住。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { addServer, deleteServer, getMotd, updateServer } from "../../../lib/api";
import { faviconOf } from "../../../lib/motd";
import MotdCard from "../../../components/MotdCard.vue";
import ContentHead from "./ContentHead.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import ServerFormModal from "./ServerFormModal.vue";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { ServerFormDraft } from "../types";
import type { MotdDto, ServerItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，服务器列表、加载态与重拉从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，忙碌标记与确认弹窗从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { servers, instanceUuid, loading, reloadCurrent } = props.data;
// 行的操作只剩「编辑 / 删除」：服务器行没有"打开文件夹"这一项（用户点名去掉），
// 且都搬进了右键菜单
const { busy, act, askConfirm, askDelete } = props.ops;

/** 表单草稿，`null` 表示没开弹窗 */
const form = ref<ServerFormDraft | null>(null);
const saving = ref(false);

/** 一行的身份键：名字 + 地址；servers.dat 允许重名，地址才是真的定位 */
function keyOf(item: ServerItemDto): string {
  return `${item.name}|${item.ip}`;
}

/**
 * MOTD 查询结果：`keyOf(item)` → 结果
 *
 * 值可能是 `null`（查过但失败），而"键不存在"表示**还没回来** ——
 * 两者在界面上不一样（"查询中…" vs "离线"），所以不能都塞成 null。
 */
const motds = ref(new Map<string, MotdDto | null>());

/** 正在查的键集合（用于"查询中…"） */
const pending = ref(new Set<string>());

/** 并发查全部服务器，进分类 / 刷新列表时跑一遍 */
async function queryAll() {
  const list = servers.value;
  motds.value = new Map();
  if (!list.length) {
    pending.value = new Set();
    return;
  }
  pending.value = new Set(list.map(keyOf));

  await Promise.all(
    list.map(async (item) => {
      const key = keyOf(item);
      try {
        motds.value.set(key, await getMotd(item.ip));
      } catch {
        // 失败也要落一条 null：否则这一行会永远停在"查询中…"
        motds.value.set(key, null);
      } finally {
        const next = new Set(pending.value);
        next.delete(key);
        pending.value = next;
      }
    }),
  );
}

/**
 * 列表上方那张 MOTD 卡显示哪一台：存 `keyOf`，`null` = 还没选
 *
 * 默认选中第一台，列表一加载上方就有内容，不用先点一下；选中的那台被删掉或改名
 * 之后键会落空，也在下面退回第一台。
 */
const selectedKey = ref<string | null>(null);

/** 选中的条目；列表为空时是 `null`，上方卡片随之整块不渲染 */
const selected = computed<ServerItemDto | null>(() => {
  const list = servers.value;
  if (!list.length) {
    return null;
  }
  // 选中的那台还在就用它，找不到（删了 / 改名了）先退回第一台
  return list.find((item) => keyOf(item) === selectedKey.value) ?? list[0];
});

/**
 * 卡片上"刷新中…"至少露这么久
 *
 * 查本机 / 局域网的服务器常常几十毫秒就回来了，不压住时间的话"刷新中…"一闪而过、
 * 数据又没变，点下去看着就跟没点一样 —— 用户反馈过"点了刷新卡片没有反应"。
 * 500ms 是"能看清有这么一下"又不拖沓的量
 */
const REFRESH_MIN_MS = 500;

/**
 * 只重查一台（上方卡片上的刷新按钮）
 *
 * 与 [`queryAll`] 的区别只在范围：别的行保持原样、也不进"刷新中"，
 * 所以翻到哪台点刷新，动的就只有那一台
 */
async function queryOne(item: ServerItemDto) {
  const key = keyOf(item);
  const next = new Set(pending.value);
  next.add(key);
  pending.value = next; // → 卡片立刻显示"刷新中…"（meta 那一行优先显示它）

  const started = Date.now();
  let ok = false;
  let result: MotdDto | null = null;

  try {
    result = await getMotd(item.ip);
    ok = true;
  } catch {
    // 与 queryAll 同一口径：失败也落一条 null，否则这一行会永远停在"刷新中…"
  }

  // 先补足最短展示时间，再落结果 —— 否则这一下眨眼就过去了
  const rest = REFRESH_MIN_MS - (Date.now() - started);
  if (rest > 0) {
    await new Promise((resolve) => setTimeout(resolve, rest));
  }

  motds.value.set(key, result);

  const after = new Set(pending.value);
  after.delete(key);
  pending.value = after;

  // 数据可能一点没变（服务器信息本来就那样），给一条"确实刷新过"的回执
  if (ok) {
    showToast(t("tip.refreshed"));
  }
}

// 列表一换就重查一遍，即首次加载、点刷新、增删改之后重拉；并把选中项落到仍然有效的那台
watch(
  servers,
  () => {
    void queryAll();
    if (!servers.value.some((item) => keyOf(item) === selectedKey.value)) {
      selectedKey.value = servers.value.length ? keyOf(servers.value[0]) : null;
    }
  },
  { immediate: true },
);

/** 这一行的 MOTD；没查到 / 还在查时返回 null */
function motdOf(item: ServerItemDto): MotdDto | null {
  return motds.value.get(keyOf(item)) ?? null;
}

/** 这一行是不是正在查 */
function isLoading(item: ServerItemDto): boolean {
  return pending.value.has(keyOf(item));
}

/** 查到且服务器在线 */
function isOnline(item: ServerItemDto): boolean {
  return motdOf(item)?.state === "ok";
}

/**
 * 行图标：**只用服务器查询带回来的 favicon**
 *
 * `servers.dat` 里存的那个直接跳过，即 `item.icon`，后端已转成 data URL —— 用户口径。
 * 查不到 / 还在查时为 `""`，`ResourceRow` 对空图标是"整块不渲染"，不留占位框
 */
function iconOf(item: ServerItemDto): string {
  return faviconOf(motdOf(item)) || "";
}

/** 打开表单；传 `null` 是新增，传条目是编辑 */
function openForm(item: ServerItemDto | null) {
  form.value = item
    ? {
      edit: true,
      name: item.name,
      ip: item.ip,
      acceptTextures: item.acceptTextures,
      origName: item.name,
      origIp: item.ip,
    }
    : { edit: false, name: "", ip: "", acceptTextures: false, origName: "", origIp: "" };
}

/** 提交表单：新增，或按原 name/ip 定位改那一条；成功后重拉列表 */
async function save() {
  const draft = form.value;
  if (!draft || saving.value) return;
  const name = draft.name.trim();
  const ip = draft.ip.trim();
  if (!name || !ip) {
    showToast(t("err.nameIp"));
    return;
  }
  saving.value = true;
  try {
    if (draft.edit) {
      await updateServer(
        instanceUuid.value,
        draft.origName,
        draft.origIp,
        name,
        ip,
        draft.acceptTextures,
      );
    } else {
      await addServer(instanceUuid.value, name, ip);
    }
    form.value = null;
    await reloadCurrent();
  } catch (e) {
    // 失败提示后输入内容留着，用户改完再存
    showToast(tErr(e));
  } finally {
    saving.value = false;
  }
}

function remove(item: ServerItemDto) {
  askDelete(item.name || item.ip, () => deleteServer(instanceUuid.value, item.name, item.ip));
}

// ---------- 多选 ----------
//
// 与其它分类同一套口径（见 useRowSelect / RowMenu），身份键就是 keyOf。
// 唯一的差别在行点击：这里点行原来是"切换上方 MOTD 卡显示哪一台"，
// 所以只有多选状态下点行才是加选，见 onServerClick。

// `selected` 这个名字已经被上方 MOTD 卡占用了，所以这里把多选的集合改名
const {
  selected: selection,
  anchorKey,
  count,
  isSelected,
  toggle,
  onRowClick,
  clear,
} = useRowSelect<ServerItemDto>({
  keyOf,
  order: () => servers.value,
});

/** 选中的服务器 */
const selectedItems = computed(() => servers.value.filter(isSelected));

/**
 * 行点击
 *
 * 多选状态下点行 = 加选，与其它分类一致；否则维持原行为：
 * 把上方那张 MOTD 卡切到这一台
 *
 * - `payload.event`: 原始点击事件，取 `shiftKey` 判整段选择
 * - `payload.item`: 点到的这一行
 */
function onServerClick(payload: { event: MouseEvent; item: ServerItemDto }) {
  if (payload.event.shiftKey || count.value) {
    onRowClick(payload);
    return;
  }
  selectedKey.value = keyOf(payload.item);
}

/** 批量删除；删完选中项已不在列表里，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteServer(instanceUuid.value, item.name, item.ip);
      });
      clear();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍
    { reload: false },
  );
}

// ---------- 右键菜单 ----------

/** 打开着的菜单；`item` 为 `null` 表示批量菜单 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: ServerItemDto | null;
} | null>(null);

/** 单行菜单项：服务器的动作与这一行的内容无关，所以不看 `item` */
function itemMenuItems(): RowMenuItem[] {
  return [
    { id: "edit", label: t("resource.serverEdit") },
    { id: "remove", label: t("resource.delete"), danger: true, separated: true },
  ];
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: ServerItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**
  const picked = selectedItems.value;
  if (picked.length) {
    rowMenu.value = {
      x,
      y,
      title: t("resource.selectedCount", { n: picked.length }),
      items: [{ id: "remove", label: t("resource.delete"), danger: true }],
      item: null,
    };
    return;
  }

  rowMenu.value = { x, y, items: itemMenuItems(), item: payload.item };
}

/** 菜单动作分发 */
function onMenuPick(id: string) {
  const menu = rowMenu.value;
  if (!menu) return;

  if (!menu.item) {
    if (id === "remove") batchRemove();
    return;
  }

  const item = menu.item;
  switch (id) {
    case "edit":
      openForm(item);
      break;
    case "remove":
      remove(item);
      break;
  }
}
</script>

<template>
  <ContentHead :data="data">
    <!-- 有选中项时顶栏换成批量操作条，与其它分类同一口径 -->
    <template #actions>
      <template v-if="count">
        <span class="row-selected-count">{{ t("resource.selectedCount", { n: count }) }}</span>
        <button class="mini-btn danger" :disabled="busy" @click="batchRemove">
          {{ t("resource.delete") }}
        </button>
        <button class="mini-btn" :disabled="busy" @click="clear">
          {{ t("resource.clearSelection") }}
        </button>
      </template>
      <button v-else class="mini-btn" :disabled="busy" @click="openForm(null)">
        {{ t("resource.serverAdd") }}
      </button>
    </template>
  </ContentHead>

  <!--
    上方 MOTD 卡：显示**当前选中**那一台（点列表里的行切换）。
    与主窗口底部那张、实例详情里那张是同一个组件，所以观感完全一致；
    右上角的刷新按钮**只重查这一台**（别的行不动），见 queryOne。
    外面这层只做横向对齐 —— 卡片不在滚动容器里，拿不到 `.item-list` 的内边距
  -->
  <div v-if="selected" class="sv-motd">
    <MotdCard refreshable :motd="motdOf(selected)" :loading="isLoading(selected)" :name="selected.name || selected.ip"
      @refresh="queryOne(selected)" />
  </div>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <div v-else class="item-list">
    <ResourceRow v-for="item in servers" :key="keyOf(item)" class="sv-row" :class="{
      'sv-selected': keyOf(item) === selectedKey,
      'row-selected': isSelected(item),
      'row-anchor': anchorKey === keyOf(item),
      'row-selecting': selection.size > 0,
    }" :icon="iconOf(item)" :name="item.name" @click="onServerClick({ event: $event, item })"
      @contextmenu.prevent="openMenu({ event: $event, item })">
      <template #select>
        <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
          @change.stop="toggle(item)" @click.stop />
      </template>
      <template #badges>
        <span v-if="item.acceptTextures" class="badge badge-dim">
          {{ t("resource.acceptTextures") }}
        </span>
        <!-- 在线人数：查到了才显示；查不到时那一行下面会写连不上的原因 -->
        <span v-if="isOnline(item)" class="badge">
          {{
            t("server.players", {
              now: motdOf(item)?.playersOnline ?? 0,
              max: motdOf(item)?.playersMax ?? 0,
            })
          }}
        </span>
      </template>

      <!--
        副标题：**这一行自己回答"能不能连上"** + 地址。
        MOTD 搬去上方那张卡了，这里不再铺彩色分段 —— 一行只有一句话，扫一眼就分得清。
        地址**始终留着**：它是这一行的定位信息，与连不连得上无关
      -->
      <template #sub>
        <span v-if="isLoading(item)" class="sv-state wait">{{ t("server.refreshing") }}</span>
        <span v-else-if="isOnline(item)" class="sv-state ok">{{ t("server.online") }}</span>
        <span v-else class="sv-state off">{{ motdOf(item)?.message || t("server.offline") }}</span>
        <span class="sep">·</span>
        <span>{{ item.ip }}</span>
      </template>
    </ResourceRow>
    <div v-if="!servers.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title"
    @pick="onMenuPick" @close="rowMenu = null" />

  <ServerFormModal v-if="form" :form="form" :busy="saving" @submit="save" @close="form = null" />
</template>

<!-- 副标题里的分隔号：与模组行的 `·` 同一套观感；行内间距靠 CSS，不靠空格 -->
<style scoped>
.sep {
  margin: 0 5px;
  color: var(--text-dim);
  opacity: 0.7;
}

/* 行里的连接状态：能连上给绿、连不上给红、查询中压暗。
   MOTD 搬去上方卡片之后，这一句就是整行的主要信息，得能一眼扫出来 */
.sv-state.ok {
  color: var(--green);
}

.sv-state.off {
  color: var(--red);
}

.sv-state.wait {
  color: var(--text-dim);
}
</style>
