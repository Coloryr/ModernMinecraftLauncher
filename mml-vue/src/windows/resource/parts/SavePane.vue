<script setup lang="ts">
// 存档列表：编辑模式 / 难度 / 极限 / 名字 / 种子、备份、还原、打开目录、删除；
// 数据包是同一分类下的子页，见 DatapackPane
//
// 支持多选（与模组列表同一套口径）：行左端勾选框加选、Shift + 点整段选中，
// 有选中项时顶栏换成批量操作条，右键则换成批量菜单 —— 两处**都只有「删除」**（用户口径）。
// **行尾不排按钮**，单行动作全在右键菜单里，单行与批量共用一个菜单组件，见 RowMenu。
// 批量备份与「还原」都不进批量：备份要一个一个来、还原得逐个挑备份，见 openRestore。
import { computed, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { stripFormatting } from "../../../lib/formatting";
import { showToast } from "../../../lib/toast";
import { openWindow } from "../../windowManager";
import { launchGame } from "../../../lib/launch";
import {
  api,
  backupSave,
  deleteSave,
  editSave,
  listSaveBackups,
  restoreSaveBackup,
} from "../../../lib/api";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import BaseSwitch from "../../../components/ui/BaseSwitch.vue";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import SaveTabs from "./SaveTabs.vue";
import VirtualList from "./VirtualList.vue";
import { RESOURCE_ROW_GAP } from "../composables/useVirtualRows";
import { useRowSelect } from "../composables/useRowSelect";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { SaveBackupDto, SaveItemDto } from "../../../lib/bindings";
import { formatSize, formatTime } from "../../../lib/format";

const props = defineProps<{
  /** 数据层，存档列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化、确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { saves, instanceUuid, loading, reloadCurrent } = props.data;
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

// ---------- 多选（顶栏批量操作） ----------
//
// 与模组列表同一套口径，选中逻辑见 useRowSelect。存档只有列表视图，没有分组也没有排序，
// 所以整段选择的顺序就是后端给的顺序。

/** 选中键用**目录名**：它就是后端认的存档身份，备份 / 删除前后都不变 */
const { selected, anchorKey, count, isSelected, toggle, onRowClick, clear } = useRowSelect<SaveItemDto>({
  keyOf: (item) => item.dir,
  order: () => saves.value,
});

/** 选中的那些存档 */
const selectedItems = computed(() => saves.value.filter(isSelected));

/** 批量删除（进回收站）：删完选中项已经不在列表里了，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteSaveConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) await deleteSave(instanceUuid.value, item.dir);
      });
      clear();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍
    { reload: false },
  );
}

/**
 * 要还原的那个存档 + 它的备份列表，`null` 表示弹窗没开
 *
 * 列表在**点开弹窗时**才去后端取：备份数已经随存档列表给过来了，即 `item.backups`，
 * 所以"有没有备份"不用额外请求；具体文件列表只在真要还原时才需要。
 */
const restorePick = ref<{ item: SaveItemDto; list: SaveBackupDto[] } | null>(null);

// ---------- 右键菜单 ----------
//
// 单行与批量共用 RowMenu：没有选中项时是单行菜单，动作只作用在右键那一行；
// 有选中项时是批量菜单，**只有「删除」**（用户口径）。

/** 打开着的菜单；`item` 为 `null` 表示批量菜单 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: SaveItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: SaveItemDto): RowMenuItem[] {
  // 「编辑存档」放最前：改模式与难度是这个列表里唯一的"改内容"动作。
  // 损坏的存档置灰：它没有 `Data` 复合标签，后端 set_mode 必然返回 DataNotFound
  const items: RowMenuItem[] = [
    // 「进入世界」= 1.20+ 的 quick play，直接进这个世界；低版本没有等价参数，不给入口
    ...(props.data.instance.value?.quickPlay
      ? [{ id: "enter-world", label: t("resource.enterWorld") }]
      : []),
    { id: "edit", label: t("resource.saveEdit"), disabled: item.broken },
    // 「修改配置文件」开配置窗口，直接落到这个存档的文件树：
    // 编辑弹窗只管 level.dat 那几项，这个存档下其它文件在那边看与改
    { id: "config", label: t("actions.editConfig") },
    { id: "backup", label: t("resource.backup") },
  ];
  // 有备份才给还原入口：没有备份的存档点还原只会弹一句"还没有备份"
  if (item.backups) items.push({ id: "restore", label: t("resource.restore") });
  items.push({ id: "rename", label: t("resource.rename"), separated: true });
  items.push({ id: "open-folder", label: t("resource.openFolder"), separated: true });
  items.push({ id: "remove", label: t("resource.delete"), danger: true, separated: true });
  return items;
}

/** 打开右键菜单 */
function openMenu(payload: { event: MouseEvent; item: SaveItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**：只给「删除」，「备份」留在顶栏那条批量条上
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

  rowMenu.value = { x, y, items: itemMenuItems(payload.item), item: payload.item };
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
    case "enter-world":
      void enterWorld(item);
      break;
    case "edit":
      openEdit(item);
      break;
    case "config":
      // 配置文件窗口直接落到这个存档；实例取本窗口锁定的那个，两边才看的是同一个实例
      openWindow("config", { uuid: instanceUuid.value, saveDir: item.dir });
      break;
    case "backup":
      backup(item);
      break;
    case "restore":
      openRestore(item);
      break;
    case "rename":
      props.ops.askRename("saves", item.dir);
      break;
    case "open-folder":
      openFolder("saves", item.dir);
      break;
    case "remove":
      remove(item);
      break;
  }
}

/** 直接进这个世界：后端会把它拼成 `--quickPlaySingleplayer`（1.20+ 才有） */
function enterWorld(item: SaveItemDto) {
  actLocal(async () => {
    if (!(await launchGame(instanceUuid.value, item.dir))) return;
    // 启动交给后台任务，这里立刻返回：说一句，别让人以为没反应
    showToast(t("resource.enterWorldStarted"));
  });
}

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

// ---------- 编辑存档 ----------
//
// 写的是存档里的 level.dat，后端 `resource_save_edit`：模式 / 难度 / 极限模式 / 名字 / 种子
// 一次全部下发。实例正在运行时后端拒绝，前端在开弹窗前也拦一道，见 openEdit。

/**
 * 编辑弹窗的草稿；`null` 表示没开
 *
 * 初值取**列表里那一条**，不重新读盘：列表本来就来自 level.dat，两边一致。
 * 种子在列表里就是字符串，一路带着走，别在这里转成数字。
 */
const editForm = ref<{
  item: SaveItemDto;
  levelName: string;
  gameType: number;
  difficulty: number;
  hardCore: boolean;
  seed: string;
} | null>(null);
const editBusy = ref(false);
/** 碰过输入框或点过保存才报红，弹窗刚开时不报警，与分组名弹窗同一口径 */
const editTouched = ref(false);

/**
 * 打开编辑弹窗
 *
 * 实例跑着的时候**不开**：游戏退出时会用它内存里的值覆盖 level.dat，改了也白改，
 * 所以先查一次运行状态并给提示。
 *
 * 运行标记取自 `main_get_instances`：`main_get_running` 读的是**调用方窗口**的模型，
 * 资源窗口调它只会拿到空表。
 *
 * - `item`: 要编辑的那条存档
 */
async function openEdit(item: SaveItemDto) {
  try {
    const list = await api.getInstances();
    if (list.some((one) => one.uuid === instanceUuid.value && one.running)) {
      showToast(t("err.instanceRunning"));
      return;
    }
  } catch (e) {
    showToast(tErr(e));
    return;
  }

  editForm.value = {
    item,
    levelName: item.levelName,
    gameType: item.gameType,
    difficulty: item.difficulty,
    hardCore: item.hardCore,
    seed: item.seed,
  };
  editTouched.value = false;
}

/**
 * 取值能不能在下拉里摆出来，即 0–3
 *
 * 旧存档的 GameType 常见 -1，那时下拉里只画一条禁用的「未知」占位，
 * 必须重新选一个才能存 —— 直接存会把"认不出来"悄悄写成"生存"。
 */
function isKnownValue(value: number): boolean {
  return value >= 0 && value <= 3;
}

/** 种子是否合法：十进制整数、可带负号，且落在 i64 范围内；口径与后端 `resource_save_edit` 一致 */
function seedValid(text: string): boolean {
  const trimmed = text.trim();
  if (!/^-?\d+$/.test(trimmed)) return false;
  // 用 BigInt 比范围：种子常常超出 JS 的安全整数范围，按 Number 判定会漏
  const value = BigInt(trimmed);
  return value >= -(2n ** 63n) && value <= 2n ** 63n - 1n;
}

/** 名字的校验结果，空串表示没问题 */
const editNameError = computed(() => {
  const form = editForm.value;
  if (!form) return "";
  return form.levelName.trim() ? "" : t("err.saveNameEmpty");
});

/** 种子的校验结果，空串表示没问题 */
const editSeedError = computed(() => {
  const form = editForm.value;
  if (!form) return "";
  return seedValid(form.seed) ? "" : t("err.saveSeed");
});

/**
 * 随机一个种子填进输入框
 *
 * 用 `crypto` 取 8 个随机字节按 i64 解释：level.dat 里的种子就是这个范围，
 * 而 `Math.random` 只有 53 位有效位，凑不出完整的 64 位。
 */
function randomSeed() {
  const form = editForm.value;
  if (!form) return;

  const bytes = crypto.getRandomValues(new Uint8Array(8));
  let value = 0n;
  for (const byte of bytes) value = (value << 8n) | BigInt(byte);
  form.seed = BigInt.asIntN(64, value).toString();
  editTouched.value = true;
}

/** 模式或难度还是"认不出来的原值"：给一条重新选的提示 */
const editModeUnknown = computed(() => {
  const form = editForm.value;
  if (!form) return false;
  return !isKnownValue(form.gameType) || !isKnownValue(form.difficulty);
});

/** 点过保存之后才报的那几处错 */
const editInvalid = computed(
  () => editTouched.value && (editModeUnknown.value || !!editNameError.value || !!editSeedError.value),
);

/**
 * 保存：写回 level.dat，然后重拉列表
 *
 * 模式与难度只在行的悬停提示里显示，重拉一次让它与新写进去的值对齐。
 * 实例正在运行时后端回 `err.instanceRunning`，这时弹窗**留着不关**，用户可以先去退游戏。
 */
async function saveEdit() {
  const form = editForm.value;
  if (!form || editBusy.value) return;
  editTouched.value = true;
  if (editInvalid.value) return;
  editBusy.value = true;
  try {
    await editSave(instanceUuid.value, form.item.dir, {
      gameType: form.gameType,
      difficulty: form.difficulty,
      hardCore: form.hardCore,
      levelName: form.levelName.trim(),
      seed: form.seed.trim(),
    });
    editForm.value = null;
    await reloadCurrent();
    showToast(t("resource.saveEditOk"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    editBusy.value = false;
  }
}

/** 游戏模式的取值名，下标就是 level.dat 的 GameType */
const GAME_TYPES = ["survival", "creative", "adventure", "spectator"];

/** 难度的取值名，下标就是 level.dat 的 Difficulty */
const DIFFICULTIES = ["peaceful", "easy", "normal", "hard"];

/** 游戏模式：level.dat 的 GameType，取值 0–3；认不出的（旧存档常见 -1）按未知显示 */
function modeLabel(value: number): string {
  return GAME_TYPES[value]
    ? t(`resource.gameType.${GAME_TYPES[value]}`)
    : t("resource.saveValueUnknown");
}

/** 难度：level.dat 的 Difficulty，取值 0–3 */
function difficultyLabel(value: number): string {
  return DIFFICULTIES[value]
    ? t(`resource.difficulty.${DIFFICULTIES[value]}`)
    : t("resource.saveValueUnknown");
}

/**
 * 悬停提示：行里放不下的那几条信息
 *
 * 行上只有名字、目录、上次游玩与几个徽标，游戏模式与难度没有展示位，这里补上；
 * 一行一条，靠 `\n` 断行（`.app-tip` 是 `pre-line`）。
 *
 * - `item`: 存档条目
 *
 * # 返回值
 *
 * 多行提示文本
 */
function saveTip(item: SaveItemDto): string {
  return [
    t("resource.saveModeLine", { value: modeLabel(item.gameType) }),
    t("resource.saveDifficultyLine", { value: difficultyLabel(item.difficulty) }),
    t("resource.lastPlayed", { time: formatTime(item.lastPlayed) }),
    t("resource.backupCount", { n: item.backups }),
    t("resource.saveDirLine", { dir: item.dir }),
  ].join("\n");
}
</script>

<template>
  <ContentHead :data="data">
    <SaveTabs :data="data" />
    <!-- 有选中项时顶栏换成批量操作条，与模组列表同一口径。
         这一条与右键的批量菜单**动作一致**，都只有「删除」：批量备份用户不要 -->
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
    </template>
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <VirtualList v-else class="item-list" :count="saves.length" :row-gap="RESOURCE_ROW_GAP">
    <template #default="{ start, end }">
      <ResourceRow v-for="item in saves.slice(start, end)" :key="item.dir" :icon="item.icon" :name="item.levelName || item.dir"
        v-tip.mouse="saveTip(item)" :class="{
          'row-selected': isSelected(item),
          'row-anchor': anchorKey === item.dir,
          'row-selecting': selected.size > 0,
        }" @click="onRowClick({ event: $event, item })" @contextmenu.prevent="openMenu({ event: $event, item })">
        <!-- 多选勾选框，与模组列表同一套，见 useRowSelect -->
        <template #select>
          <input type="checkbox" class="row-select" :checked="isSelected(item)" @pointerdown.stop
            @change.stop="toggle(item)" @click.stop />
        </template>
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
        <!-- 目录名也可能带 `§` 格式码（存档文件夹是玩家自己起的名），与名字同一套解析 -->
        <template #sub>
          <FormattedText :text="item.dir" /> · {{ t("resource.lastPlayed", { time: formatTime(item.lastPlayed) }) }}
        </template>
      </ResourceRow>
      <div v-if="!saves.length" class="empty-tip">{{ t("resource.empty") }}</div>
    </template>
  </VirtualList>

  <!-- 行右键菜单：单行与批量共用，菜单项见 itemMenuItems / batchMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />

  <!--
    编辑存档：名字 / 游戏模式 / 难度 / 极限模式 / 地图种子，写回该存档的 level.dat

    模式与难度**只能选 0–3**：原值认不出来时（旧存档的 GameType 常见 -1）多画一条禁用的
    「未知」占位，把现状摆在眼前，逼用户重新选一个 —— 直接存会把"认不出来"悄悄写成"生存"。

    种子是**字符串**：它在 level.dat 里是 i64，超过 JS 的安全整数范围，当数字过一遍就变样了。
    名字与种子的红框与提示在碰过之后才出现，与分组名弹窗同一口径。
  -->
  <BaseModal v-if="editForm" :title="t('resource.saveEdit')" :closable="false" @close="editForm = null">
    <p class="save-edit-name">
      <FormattedText :text="editForm.item.levelName || editForm.item.dir" />
    </p>
    <p class="save-edit-dir" :title="editForm.item.dir">{{ editForm.item.dir }}</p>

    <label class="field-label">{{ t("resource.saveNameField") }}</label>
    <input v-model="editForm.levelName" class="field-input" :class="{ 'is-invalid': editTouched && !!editNameError }"
      spellcheck="false" @input="editTouched = true" @keydown.enter="saveEdit" />
    <p v-if="editTouched && editNameError" class="error-text">{{ editNameError }}</p>

    <div class="field-row-2">
      <div>
        <label class="field-label">{{ t("resource.saveModeField") }}</label>
        <select v-model.number="editForm.gameType" class="field-select"
          :class="{ 'is-invalid': editTouched && !isKnownValue(editForm.gameType) }" @change="editTouched = true">
          <option v-if="!isKnownValue(editForm.gameType)" :value="-1" disabled>
            {{ t("resource.saveValueUnknown") }}
          </option>
          <option v-for="(name, index) in GAME_TYPES" :key="name" :value="index">
            {{ t(`resource.gameType.${name}`) }}
          </option>
        </select>
      </div>
      <div>
        <label class="field-label">{{ t("resource.saveDifficultyField") }}</label>
        <select v-model.number="editForm.difficulty" class="field-select"
          :class="{ 'is-invalid': editTouched && !isKnownValue(editForm.difficulty) }" @change="editTouched = true">
          <option v-if="!isKnownValue(editForm.difficulty)" :value="-1" disabled>
            {{ t("resource.saveValueUnknown") }}
          </option>
          <option v-for="(name, index) in DIFFICULTIES" :key="name" :value="index">
            {{ t(`resource.difficulty.${name}`) }}
          </option>
        </select>
      </div>
    </div>
    <p v-if="editTouched && editModeUnknown" class="error-text">{{ t("resource.saveEditUnknown") }}</p>

    <label class="field-label">{{ t("resource.saveSeedField") }}</label>
    <!-- 输入框 + 随机：`.path-row` 是"输入框 + 按钮一行"的通用写法，按钮靠它跟输入框等高 -->
    <div class="path-row">
      <input v-model="editForm.seed" class="field-input" :class="{ 'is-invalid': editTouched && !!editSeedError }"
        spellcheck="false" @input="editTouched = true" @keydown.enter="saveEdit" />
      <BaseButton size="sm" class="pick-btn" @click="randomSeed">
        {{ t("resource.saveSeedRandom") }}
      </BaseButton>
    </div>
    <p v-if="editTouched && editSeedError" class="error-text">{{ editSeedError }}</p>

    <div class="save-edit-switch">
      <span class="field-label">{{ t("resource.saveHardcoreField") }}</span>
      <BaseSwitch v-model="editForm.hardCore" />
    </div>

    <div class="modal-actions">
      <BaseButton :disabled="editBusy" @click="editForm = null">{{ t("resource.cancel") }}</BaseButton>
      <BaseButton variant="primary" :disabled="editBusy" @click="saveEdit">
        {{ t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>

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

<!-- 编辑弹窗与备份弹窗的样式都走 scoped：BaseModal 是 Teleport 到 body 的，
     窗口级 resource.css 那套 .resource-layout 前缀够不着它（备注弹窗同理） -->
<style scoped>
/* 存档名（作者可能带 `§` 格式码，过一遍显示期解析）与它的目录名，一行一条 */
.save-edit-name {
  margin: 0 0 2px;
  font-size: 13px;
  color: var(--text);
}

.save-edit-dir {
  margin: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 极限模式：标签 + 开关一行，标签不要 .field-label 的上下外边距 */
.save-edit-switch {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
}

.save-edit-switch .field-label {
  margin: 0;
}

.backup-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 备份可能很多：给个上限自己滚，别把弹窗撑出屏幕 */
  max-height: 320px;
  overflow-y: auto;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行的右边缘仍落在弹窗内容区里，滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

/* 一行一个备份：整行可点，点下去弹确认框。
   底色取 `--bg-hover` 而不是 `--bg-raised` —— 亮色主题下后者与弹窗底 `--bg-card`
   都是纯白，行就只剩一圈浅边框，"看不出是个能点的项目" */
.backup-pick-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 11px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-hover);
  color: var(--text);
  font-size: 12.5px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: all 0.12s;
}

.backup-pick-row:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--accent-soft);
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
