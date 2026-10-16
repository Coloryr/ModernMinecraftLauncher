<script setup lang="ts">
// 模组分类：列表 / 表格两种展示方式 + 自定义分组
//
// - 顶栏（ContentHead）：搜索 + 视图切换 + 新建分组；有选中项时整条换成批量操作
// - 每个自建分组一块，后面按顺序表跟着「已启用 / 已禁用 / 识别失败」三个状态分组
// - 归组走「移到分组」弹窗与单行右键菜单的「移出分组」，**不支持拖动模组改分组**
//
// 数据：模组列表在 useResourceData，分组在 useModGroups（自己的 JSON），
// 拖动分组头调整分组顺序在 useReorderDrag。
import { computed, onMounted, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import SegmentedTabs from "../../../components/ui/SegmentedTabs.vue";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";
import DragGhost from "../../../components/ui/DragGhost.vue";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import ContentHead from "./ContentHead.vue";
import LoaderQueryProgress from "../../../components/LoaderQueryProgress.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ModList from "./mod/ModList.vue";
import ModTable from "./mod/ModTable.vue";
import ModDepsModal from "./mod/ModDepsModal.vue";
import ModUpdateModal from "./mod/ModUpdateModal.vue";
import ModGroupMenu, { type GroupMenuAction } from "./mod/ModGroupMenu.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import {
  deleteMod,
  disableMod,
  enableMod,
  modsDepTest,
  modsUpdateApply,
  modsUpdateCheck,
  onModsUpdateDone,
  onModsUpdateProgress,
  setModNote,
} from "../../../lib/api";
import { copyText } from "../../../lib/clipboard";
import { useModDeps } from "../composables/useModDeps";
import { useReorderDrag } from "../composables/useReorderDrag";
import { useRowSelect } from "../composables/useRowSelect";
import {
  useModGroups,
  STATE_GROUP_ID_OFF,
  STATE_GROUP_ID_ON,
  STATE_GROUP_ID_FAIL,
} from "../composables/useModGroups";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { useResourceView } from "../composables/useResourceView";
import type {
  DepProblemDto,
  ModItemDto,
  ModUpdateApplyDto,
  ModUpdateCheckDto,
  ModUpdateDoneDto,
  ModUpdateDto,
} from "../../../lib/bindings";
import { modHit, modSelectKey, sortedMods, type ModStateGroup } from "../types";
import { orderedMods } from "../modColumns";
import type { ModView } from "../types";

const props = defineProps<{
  /** 数据层，模组列表、加载态与扫描进度从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，串行化、确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
  /** 视图偏好，模组展示方式、列宽与表头排序从这里取 */
  view: ReturnType<typeof useResourceView>;
}>();

const {
  mods,
  instance,
  instanceUuid,
  loading,
  modProgress,
  modDuplicates,
  modDepProblems,
  dismissModDuplicates,
} = props.data;
const { busy, act, actLocal, askDelete, askConfirm, openFolder } = props.ops;

/** 依赖检测结果弹窗是否打开 */
const depOpen = ref(false);
/** 依赖检测正在跑：要重扫一遍 mods 目录，几百个包要数秒 */
const depBusy = ref(false);
/** 依赖检测的结果，打开弹窗时才有意义 */
const depProblems = ref<DepProblemDto[]>([]);

/** 更新流程正在跑：检查或下载，同一时刻只有一段，按钮统一置灰 */
const updateBusy = ref(false);
/** 现在跑的是哪一段：check = 逐个问平台，download = 下新文件；null = 没跑 */
const updateStage = ref<"check" | "download" | null>(null);
/** 更新进度；检查阶段的总数要等后端扫完 mods 目录才知道，在那之前一直是 0 */
const updateProgress = ref({ done: 0, total: 0 });
/** 更新检查结果；有更新才开弹窗，null = 没有可更新的 */
const updateResult = ref<ModUpdateCheckDto | null>(null);
/** 更新下载弹窗是否打开 */
const updateOpen = ref(false);

/**
 * 「有更新」虚拟分组的固定 uuid
 *
 * 与三个状态分组同一套键空间，取尾号 3；但**不落盘也不进顺序表** ——
 * 它只在这一轮检查有结果时存在，永远画在最前，位置不给用户拖。
 */
const UPDATE_GROUP_ID = "00000000-0000-0000-0000-000000000003";

/**
 * 去掉禁用后缀的文件名，用来把更新结果与列表里的行对上
 *
 * 更新结果记的是**检查那一刻**的文件名与 uuid，而启用 / 禁用会改文件名、uuid 跟着变；
 * 文件名去掉后缀后不变，拿它当键才能让结果扛住一次切换。
 *
 * - `file`: mods 目录下的文件名
 *
 * # 返回值
 *
 * 去掉 `.disable` / `.disabled` 的文件名
 */
function baseModFile(file: string): string {
  return file.replace(/\.disabled?$/i, "");
}

/** 更新结果按"去掉禁用后缀的文件名"索引 */
const updatesByFile = computed(() => {
  const map = new Map<string, ModUpdateDto>();
  for (const item of updateResult.value?.updates ?? []) {
    map.set(baseModFile(item.file), item);
  }
  return map;
});

/** 这一行有没有可更新的新版本 */
function hasUpdate(item: ModItemDto): boolean {
  return updatesByFile.value.has(baseModFile(item.file));
}

/** 有更新的那些行的多选键，两个视图靠它打「有更新」标签 */
const updateKeys = computed(() => {
  const keys = new Set<string>();
  for (const item of mods.value) {
    if (hasUpdate(item)) keys.add(modSelectKey(item));
  }
  return keys;
});

/** 问题种类的 i18n 键，与后端 `DepProblemKindDto` 一一对应 */
const DEP_KIND_LABEL: Record<DepProblemDto["kind"], string> = {
  missing: "resource.depKindMissing",
  mismatch: "resource.depKindMismatch",
  disabled: "resource.depKindDisabled",
};

/** 弹窗里下拉组的顺序，与后端排序一致：缺失 → 前置被禁用 → 版本不符 */
const DEP_KIND_ORDER: DepProblemDto["kind"][] = ["missing", "disabled", "mismatch"];

/** 检测结果按种类分组，只留有问题的那几组 */
const depGroups = computed(() =>
  DEP_KIND_ORDER.map((kind) => ({
    kind,
    items: depProblems.value.filter((p) => p.kind === kind),
  })).filter((group) => group.items.length > 0),
);

/** 展开着的下拉组；每次检测重新全展开 */
const depOpenKinds = ref<Set<DepProblemDto["kind"]>>(new Set());

/** 某个下拉组是不是展开的 */
function isDepOpen(kind: DepProblemDto["kind"]): boolean {
  return depOpenKinds.value.has(kind);
}

/** 收起 / 展开某个下拉组 */
function toggleDepGroup(kind: DepProblemDto["kind"]) {
  const next = new Set(depOpenKinds.value);
  if (next.has(kind)) {
    next.delete(kind);
  } else {
    next.add(kind);
  }
  depOpenKinds.value = next;
}

/** 依赖检测：列出缺前置 / 版本不符 / 前置被禁用 */
async function runDepTest() {
  const uuid = instanceUuid.value;
  if (!uuid || depBusy.value) return;
  depBusy.value = true;
  try {
    const problems = await modsDepTest(uuid);
    // 没有问题就用一条轻提示，别弹一个空弹窗
    if (!problems.length) {
      depProblems.value = [];
      showToast(t("resource.depNone"));
      return;
    }
    depProblems.value = problems;
    // 新结果全部展开，用户不用一个个点开
    depOpenKinds.value = new Set(DEP_KIND_ORDER);
    depOpen.value = true;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    depBusy.value = false;
  }
}

/**
 * 更新检查：列出有在线信息、且平台上有新文件的模组
 *
 * 查到更新就打开下载弹窗（勾选默认全给上），没查到只给一条轻提示，不留空窗。
 */
async function runUpdateCheck() {
  const uuid = instanceUuid.value;
  if (!uuid || updateBusy.value) return;
  updateBusy.value = true;
  updateStage.value = "check";
  updateProgress.value = { done: 0, total: 0 };
  // 先订阅再调命令，否则可能漏掉最开始那几条进度
  const unlisten = await onModsUpdateProgress((e) => {
    updateProgress.value = e;
  });
  try {
    const result = await modsUpdateCheck(uuid);
    // 没有更新就不弹空窗：按"一个都没查"与"只是都最新"给不同提示
    if (!result.updates.length) {
      updateResult.value = null;
      if (!result.checked) {
        showToast(t("resource.updateNoOnline"));
      } else if (result.failed) {
        showToast(t("resource.updateFailed", { n: result.failed }));
      } else {
        showToast(t("resource.updateNone"));
      }
      return;
    }
    updateResult.value = result;
    updateOpen.value = true;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    unlisten();
    updateBusy.value = false;
    updateStage.value = null;
    updateProgress.value = { done: 0, total: 0 };
  }
}

/**
 * 弹窗里点「下载」：把勾中的模组逐个换成平台上的新文件
 *
 * 下载参数里的 uuid 按**当前列表里的行**重算：检查之后用户可能启用 / 禁用过，
 * 文件名一变 uuid 就变了，拿旧 uuid 后端定位不到文件。对不上时退回结果里那个。
 * 下载是后台任务：进度走工具条下面那条，整批处理完由完成事件收尾。
 *
 * - `picked`: 弹窗里勾中的更新条目
 */
async function applyUpdates(picked: ModUpdateDto[]) {
  const uuid = instanceUuid.value;
  if (!uuid || updateBusy.value || !picked.length) return;

  const items: ModUpdateApplyDto[] = picked
    .map((item) => {
      const key = baseModFile(item.file);
      const current = mods.value.find((mod) => baseModFile(mod.file) === key);
      return {
        uuid: current?.uuid ?? item.uuid,
        source: item.source,
        projectId: item.projectId,
        newFileId: item.newFileId,
      };
    })
    .filter((item) => !!item.uuid);
  if (!items.length) return;

  updateBusy.value = true;
  updateStage.value = "download";
  updateProgress.value = { done: 0, total: items.length };
  updateOpen.value = false;

  let settle: (done: ModUpdateDoneDto) => void = () => {};
  const finished = new Promise<ModUpdateDoneDto>((resolve) => {
    settle = resolve;
  });
  // 先订阅再调命令，否则可能漏掉最开始那几条进度与完成事件
  const unlistenProgress = await onModsUpdateProgress((e) => {
    updateProgress.value = e;
  });
  const unlistenDone = await onModsUpdateDone((e) => settle(e));
  try {
    await modsUpdateApply(uuid, items);
    const done = await finished;
    // 换过的文件已经不是原来那一份了：结果作废并重拉列表，「有更新」分组随之消失
    updateResult.value = null;
    if (done.failed) {
      showToast(t("resource.updateDone", { ok: done.ok, failed: done.failed }));
    } else {
      showToast(t("resource.updateOk", { n: done.ok }));
    }
    await props.data.reloadCurrent();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    unlistenProgress();
    unlistenDone();
    updateBusy.value = false;
    updateStage.value = null;
    updateProgress.value = { done: 0, total: 0 };
  }
}

/**
 * 导出文本的首行：实例名 + 游戏版本 + 加载器与加载器版本
 *
 * 贴给别人排查时这几项都要；缺的项自动跳过，还没有实例时返回 `null`。
 *
 * # 返回值
 *
 * 一行摘要；没有实例时为 `null`
 */
function instanceHeader(): string | null {
  const inst = instance.value;
  if (!inst) return null;
  // 加载器走显示名（与新建实例那边同一个 i18n 键），原版就是"原版"
  const loaderName = inst.loader ? t(`add.loader.${inst.loader}`) : "";
  const loader = [loaderName, inst.loaderVersion].filter(Boolean).join(" ");
  return [inst.name, inst.version, loader].filter(Boolean).join("  ");
}

/**
 * 依赖检测结果的文本，供导出到剪贴板
 *
 * 按种类分组，与弹窗里看到的一致；每条把"装了什么 / 要求什么 / 谁需要"都带上，
 * 贴给别人排查时不用再补上下文。首行见 `instanceHeader`。
 *
 * # 返回值
 *
 * 多行文本
 */
function depReportText(): string {
  const lines: string[] = [];
  const header = instanceHeader();
  if (header) {
    lines.push(header);
  }
  for (const group of depGroups.value) {
    lines.push(`[${t(DEP_KIND_LABEL[group.kind])}]`);
    for (const p of group.items) {
      const parts = [p.modid];
      if (p.installed) {
        parts.push(t("resource.depInstalled", { version: p.installed }));
      }
      if (p.required !== "*") {
        parts.push(t("resource.depRequired", { range: p.required }));
      }
      if (p.requiredBy.length) {
        parts.push(t("resource.depBy", { list: p.requiredBy.join(", ") }));
      }
      lines.push(parts.join("  "));
    }
  }
  return lines.join("\n");
}

/** 把依赖检测结果复制到剪贴板 */
async function exportDepReport() {
  const ok = await copyText(depReportText());
  showToast(ok ? t("resource.copied") : t("resource.copyFail"));
}

/**
 * 重复安装的文本，供导出到剪贴板
 *
 * 首行是实例名与版本，之后一行一个重复的 modid 及涉及的文件。
 *
 * # 返回值
 *
 * 多行文本
 */
function duplicateReportText(): string {
  const lines: string[] = [];
  const header = instanceHeader();
  if (header) {
    lines.push(header);
  }
  lines.push(`[${t("resource.modDupTitle")}]`);
  for (const item of modDuplicates.value) {
    lines.push(`${item.modid}  ${item.files.map((file) => file.file).join(", ")}`);
  }
  return lines.join("\n");
}

/** 把重复安装结果复制到剪贴板 */
async function exportDuplicates() {
  const ok = await copyText(duplicateReportText());
  showToast(ok ? t("resource.copied") : t("resource.copyFail"));
}

const { modView, setModView, colWidths, setColWidth, sort, setSort } = props.view;

const groups = useModGroups(props.data);

/** 依赖关系：依赖图与"禁用前置时顺带禁用依赖方"都用它，见 useModDeps */
const { graphOf, dependentsOf, itemOf } = useModDeps(mods, instance);

/** 打开着依赖图的那个模组；null = 没开 */
const depTreeItem = ref<ModItemDto | null>(null);

/** 依赖图（含布局）：只在开着的时候算，别在模板里每次渲染重算一遍 */
const depGraph = computed(() => (depTreeItem.value ? graphOf(depTreeItem.value) : null));

/**
 * 依赖图里双击某个节点：把这张图换成以它为目标
 *
 * 图上只有 modid，条目要回 useModDeps 查；缺失的前置与虚拟前置查不到，那就不动。
 *
 * - `key`: 被双击节点的 modid（小写）
 */
function rerootGraph(key: string) {
  const item = itemOf(key);
  if (item) depTreeItem.value = item;
}

// ---------- 归组 / 改状态 ----------

/**
 * 移到自建分组之后收尾：**把模组改成启用**
 *
 * 用户定的口径是"移进分组 = 顺带启用"，所以从「已禁用」移进某个分组就是启用它。
 * 已经启用的不用碰 —— 启用 / 禁用会改文件名、uuid 跟着变，
 * 能不动就不动，也让"只是想归个组"这种常见操作不产生额外写盘。
 */
async function enableAfterMove(keys: string[]) {
  await setState(pickByState(keys, false), false);
}

/**
 * 把若干模组的启用状态落盘
 *
 * `wantDisable = true` 禁用、`false` 启用；`picked` 里只放"当前状态与目标不同"的那些。
 *
 * **不重拉列表**：启用 / 禁用只是给文件名加减后缀，jar 里的元数据一个字都没变 ——
 * 重扫要把每个包重新解析一遍（几百个包好几秒），纯属白干。
 * 后端会回**改名后的新身份**，即 uuid / 文件名 / 路径，就地改那一行即可；
 * `disable` 一改，`sections` 会把它挪到「已禁用 / 已启用」那一块去。
 */
async function applyState(picked: ModItemDto[], wantDisable: boolean) {
  await actLocal(async () => {
    for (const item of picked) {
      const renamed = wantDisable
        ? await disableMod(instanceUuid.value, item.uuid)
        : await enableMod(instanceUuid.value, item.uuid);
      // uuid 必须换：它是**文件路径的 v5**，文件名一变就变；
      // 不换的话再点一次"启用"会拿旧 uuid 去找文件，后端直接报找不到
      item.uuid = renamed.uuid;
      item.file = renamed.file;
      item.path = renamed.path;
      item.disable = wantDisable;
    }
  });
}

/** 待确认的"要不要顺带把相关模组一起改"；null = 没在问 */
const stateAsk = ref<{
  /** 本次操作的模组 */
  picked: ModItemDto[];
  /** 要一起改的那些 */
  related: ModItemDto[];
  /** true = 这次是禁用（一起禁用依赖方）；false = 这次是启用（一起启用前置） */
  wantDisable: boolean;
} | null>(null);

/**
 * 这些模组的强制前置里，现在**处于禁用状态**的那些
 *
 * 只找"装了但被禁用"的：压根没装属于"缺前置"，不是这里能顺手解决的。
 *
 * - `picked`: 这次要启用的模组
 *
 * # 返回值
 *
 * 需要一起启用的条目，按列表顺序
 */
function disabledPrerequisites(picked: ModItemDto[]): ModItemDto[] {
  const wanted = new Set<string>();
  for (const item of picked) {
    for (const dep of item.dependencies) {
      if (!dep.optional) {
        wanted.add(dep.modId.toLowerCase());
      }
    }
  }
  if (!wanted.size) return [];
  const pickedKeys = new Set(picked.map((item) => modSelectKey(item)));
  return mods.value.filter(
    (item) =>
      item.disable &&
      !!item.modId &&
      wanted.has(item.modId.toLowerCase()) &&
      !pickedKeys.has(modSelectKey(item)),
  );
}

/**
 * 改启用状态；两个方向都先看有没有"相关模组"要一起改
 *
 * - **禁用**：还有别的启用模组依赖它 —— 少了前置轻则功能缺失、重则起不来，
 *   问一句要不要一起禁用；
 * - **启用**：它要的强制前置现在被禁用着 —— 问一句要不要一起启用，省得一个个找。
 */
async function setState(picked: ModItemDto[], wantDisable: boolean) {
  if (!picked.length) return;
  const related = wantDisable
    ? dependentsOf(new Set(picked.map((item) => item.modId.toLowerCase()).filter((id) => !!id)))
    : disabledPrerequisites(picked);
  if (related.length) {
    stateAsk.value = { picked, related, wantDisable };
    return;
  }
  await applyState(picked, wantDisable);
}

/** 回答"要不要一起改"：`together = false` 就是只动点下去的那些 */
function answerStateAsk(together: boolean) {
  const ask = stateAsk.value;
  stateAsk.value = null;
  if (!ask) return;
  void applyState(together ? [...ask.picked, ...ask.related] : ask.picked, ask.wantDisable);
}

/** 从分组表记的 SHA1 里挑出"当前状态与目标不同"的那些条目 */
function pickByState(keys: string[], wantDisable: boolean): ModItemDto[] {
  return keys
    .map((key) => bySha1.value.get(key))
    .filter((item): item is ModItemDto => !!item)
    .filter((item) => item.disable !== wantDisable);
}

// ---------- 多选（顶栏批量操作） ----------
//
// 选中逻辑在 useRowSelect，与存档列表共用一套；这里只接上模组自己的键与显示顺序。

/**
 * 选中键取 `modSelectKey`，即 **SHA1 优先、uuid 兜底**
 *
 * 用 SHA1 而不是 uuid：启用 / 禁用只是给文件名加减后缀，uuid（文件路径的 v5）会跟着变，
 * 而 SHA1 是内容哈希、不变 —— 与分组表同一套键。所以"批量启用"之后选中状态仍然有效，
 * 可以接着"移到分组"；坏 jar 读不出内容哈希，`modSelectKey` 用 uuid 兜底。
 *
 * 整段选择按**当前看得见**的顺序：表格视图套"按列排序"的 `orderedMods`，
 * 列表视图套 `sortedMods`，与两个视图各自渲染出来的顺序一致；收起的分组不算进整段。
 */
const {
  selected,
  anchorKey,
  count: selectedCount,
  isSelected,
  toggle: toggleSelect,
  toggleMany: toggleSelectMany,
  onRowClick,
  clear: clearSelection,
} = useRowSelect<ModItemDto>({
  keyOf: (item) => modSelectKey(item),
  order: () =>
    sections.value
      .filter((sec) => isOpen(sec.id))
      .flatMap((sec) =>
        modView.value === "table" ? orderedMods(sec.items, sort.value) : sortedMods(sec.items),
      ),
});

/** 选中的那些条目；批量操作要 uuid 去调命令，选中键只是身份 */
const selectedItems = computed<ModItemDto[]>(() => mods.value.filter(isSelected));

/** 批量改启用状态：`wantDisable = true` 禁用、`false` 启用 */
async function batchSetState(wantDisable: boolean) {
  const picked = selectedItems.value.filter((item) => !item.fail && item.disable !== wantDisable);
  await setState(picked, wantDisable);
}

/** 批量删除（进回收站）：删完选中项已经不在列表里了，直接清空选择 */
function batchRemove() {
  const picked = selectedItems.value;
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.batchDeleteConfirm", { n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) {
          await deleteMod(instanceUuid.value, item.uuid);
        }
      });
      clearSelection();
    },
    // act 内部已经重拉过列表了，这里别再拉一遍；重拉模组要重解析所有 jar，很慢
    { reload: false },
  );
}

/**
 * 批量移到某个分组
 *
 * - `group`: 目标分组 uuid
 * - `keys`: 要移动的选中键，缺省是当前选中的那些
 *
 * **顺带启用**，见 [`enableAfterMove`]。
 */
async function batchMove(group: string, keys: string[] = [...selected.value]) {
  const sha1s = sha1Of(keys);
  if (!sha1s.length) return;
  await groups.setGroup(group, sha1s);
  await enableAfterMove(sha1s);
}

/**
 * 把选中键换算成分组表认的 SHA1
 *
 * 分组的键**必须是 SHA1**，内核的 `Mod.Groups` 按内容哈希记；而选中键可能是 uuid
 * （没有 SHA1 的条目），这里换掉并把换不出来的过滤掉，别把 uuid 混进分组表里。
 *
 * - `keys`: 选中键，SHA1 或 uuid
 *
 * # 返回值
 *
 * 换算出来的 SHA1 列表；一个都换不出来时为空
 */
function sha1Of(keys: string[]): string[] {
  return keys
    .map((key) => mods.value.find((item) => modSelectKey(item) === key))
    .map((item) => item?.sha1)
    .filter((sha1): sha1 is string => !!sha1);
}

/**
 * 批量"移到分组"的弹窗是否打开
 *
 * 用弹窗而不是顶栏下拉：下拉一次只看得见一项，而且"移到分组"这个占位项会以
 * **选项**的形式混在分组名里，看着像能选的东西（用户点名不要这样）。
 * 弹窗里把分组一次列全，点哪个就移过去。
 */
const groupPickOpen = ref(false);

/** 弹窗里选中某个分组：点完即关，然后移过去 */
function pickGroup(group: string) {
  groupPickOpen.value = false;
  const source = moveGroupSource.value;
  moveGroupSource.value = null;
  if (!source) {
    // 顶栏"移到分组"：源是选中项
    void batchMove(group);
    return;
  }
  if (source.kind === "mod") {
    // 单行右键的"移到分组"：源就这一行
    void batchMove(group, [source.sha1]);
    return;
  }
  // 分组右键菜单的"转移内容"：源是那一整组
  const keys = groupItems(source.id)
    .map((item) => item.sha1)
    .filter((key): key is string => !!key);
  void batchMove(group, keys);
}

/**
 * 弹窗里点「新建分组」：新建分组表单**叠在**这个弹窗上（用户口径）
 *
 * 刻意**不关**"移到分组"弹窗：建完新分组直接出现在下面的候选里，点一下就能移过去，
 * 不用关掉再重新点一次「移到分组」。两个弹窗都是 Teleport 到 body 的，
 * 后挂载的那个自然画在上面，所以表单盖住它、关掉表单又回到它。
 */
function createGroupFromPick() {
  openGroupForm();
}

// ---------- 分组右键菜单 ----------
//
// 菜单项全部作用在**整组**上：启用所有 / 禁用所有 / 转移内容 / 删除所有模组，
// 自建分组再多三项：重命名分组 / 删除分组 / 移除分组，即把内容移出自建分组。
// 状态分组是内核按启用状态分的，改不了名也删不掉，也没有"移出"可言 ——
// 它里面的模组本来就不在任何自建分组里。
// 菜单本身是 `mod/ModGroupMenu.vue`（只负责画与定位），动作在这里执行。

/** 打开右键菜单的那个分组（null = 没开） */
const groupMenu = ref<{
  id: string;
  label: string;
  custom: boolean;
  count: number;
  /** 组里还有禁用的模组 → 菜单给"启用所有" */
  canEnable: boolean;
  /** 组里还有启用的模组 → 菜单给"禁用所有" */
  canDisable: boolean;
  /** 组里可多选的模组已全部选中 → 菜单给"取消全选" */
  allSelected: boolean;
  x: number;
  y: number;
} | null>(null);

/** 分组里的模组；按 id 从当前 sections 里取那一批 */
function groupItems(id: string): ModItemDto[] {
  return sections.value.find((sec) => sec.id === id)?.items ?? [];
}

/**
 * 整组全选 / 取消全选
 *
 * 右键菜单的「全选」与表格里分组行的勾选框都是它：组内全部已选就整组取消，
 * 否则整组选上，见 useRowSelect 的 `toggleMany`。
 */
function groupToggleSelect(id: string) {
  toggleSelectMany(groupItems(id));
}

/** 组内可多选的模组是否已全部选中 */
function groupAllSelected(id: string): boolean {
  const keys = groupItems(id)
    .map(modSelectKey)
    .filter((key) => !!key);
  return keys.length > 0 && keys.every((key) => selected.value.has(key));
}

/** 打开分组右键菜单；表格视图由 ModTable 抛上来，列表视图直接绑在分组头上 */
function openGroupMenu(payload: {
  event: MouseEvent;
  id: string;
  label: string;
  custom: boolean;
  count: number;
}) {
  groupMenu.value = {
    id: payload.id,
    label: payload.label,
    custom: payload.custom,
    count: payload.count,
    // 启用 / 禁用两项**按组内实际状态**给，用户口径是已启用分组只显示"禁用所有"：
    // 识别失败的模组改不了启用状态（勾选框也是禁用的），所以不计入判断
    canEnable: groupItems(payload.id).some((item) => !item.fail && item.disable),
    canDisable: groupItems(payload.id).some((item) => !item.fail && !item.disable),
    allSelected: groupAllSelected(payload.id),
    x: payload.event.clientX,
    y: payload.event.clientY,
  };
}

/** 右键菜单：整组启用 / 禁用；只挑状态不同的，和顶栏批量一个口径 */
async function groupSetState(id: string, wantDisable: boolean) {
  const picked = groupItems(id).filter((item) => item.disable !== wantDisable);
  if (picked.length) await setState(picked, wantDisable);
}

/** 右键菜单：整组删除（进回收站，先确认） */
function groupRemoveMods(id: string, label: string) {
  const picked = groupItems(id);
  if (!picked.length) return;
  askConfirm(
    t("resource.delete"),
    t("resource.deleteGroupModsConfirm", { name: label, n: picked.length }),
    async () => {
      await act(async () => {
        for (const item of picked) {
          await deleteMod(instanceUuid.value, item.uuid);
        }
      });
    },
    // act 内部已经重拉过列表，这里别再拉一遍；重拉要重解析所有 jar，很慢
    { reload: false },
  );
}

/**
 * "移到分组"弹窗的源
 *
 * - `group`: 分组右键的「转移内容」，源是那一整组，候选里要排掉它自己
 * - `mod`: 单行右键的「移到分组」，源就是这一行
 * - `null`: 顶栏那条，源是当前选中项
 *
 * 三种源共用同一个弹窗，标题里报出源是谁，见 groupPickTitle。
 */
const moveGroupSource = ref<
  | { kind: "group"; id: string; label: string }
  | { kind: "mod"; sha1: string; label: string }
  | null
>(null);

/** 分组右键菜单：整组转移 */
function groupMoveAll(id: string, label: string) {
  moveGroupSource.value = { kind: "group", id, label };
  groupPickOpen.value = true;
}

/** 单行菜单：把这一行移到别的分组 */
function modMoveToGroup(item: ModItemDto) {
  // 没算出 sha1 的模组没有分组身份，菜单里那一项是灰的
  if (!item.sha1) return;
  moveGroupSource.value = { kind: "mod", sha1: item.sha1, label: item.name || item.file };
  groupPickOpen.value = true;
}

/**
 * 右键菜单：把整组内容移出自建分组
 *
 * 与单行菜单的「移出分组」同一条路：只改归属，**不动启用状态**，
 * 移完它们回到「已启用 / 已禁用」那一块；分组本身留着，成了个空分组。
 */
async function groupMoveOut(id: string) {
  const sha1s = groupItems(id)
    .map((item) => item.sha1)
    .filter((sha1): sha1 is string => !!sha1);
  if (!sha1s.length) return;
  await groups.setGroup(null, sha1s);
}

/** 这次要移的那些模组的 SHA1：单行就是那一行，顶栏那条与批量就是选中项 */
function sourceSha1s(): string[] {
  const source = moveGroupSource.value;
  if (source?.kind === "mod") return [source.sha1];
  return selectedItems.value.map((item) => item.sha1).filter((sha1) => !!sha1);
}

/** 这些模组**共同**归属的自建分组 uuid；不都在同一个分组里时是空串 */
function groupOfAll(sha1s: string[]): string {
  const first = sha1s.length ? (groups.groupOfKey.value.get(sha1s[0]) ?? "") : "";
  if (!first) return "";
  return sha1s.every((sha1) => groups.groupOfKey.value.get(sha1) === first) ? first : "";
}

/**
 * 这些模组还有没有别的分组可去
 *
 * 只有一个分组、而它们就在这个分组里时无处可去，单行 / 批量那几处「移到分组」都置灰。
 * **一个分组都没有时算有**：弹窗里有「新建分组」，建完就能移过去。
 *
 * - `sha1s`: 要移的那些模组的 SHA1；没有 SHA1 的条目会被滤掉
 *
 * # 返回值
 *
 * 还能移时为 `true`
 */
function canMoveKeys(sha1s: string[]): boolean {
  const keys = sha1s.filter((sha1) => !!sha1);
  if (!keys.length) return false;
  if (!groups.groups.value.length) return true;
  const own = groupOfAll(keys);
  return groups.groups.value.some((g) => g.uuid !== own);
}

/** 顶栏那条与批量菜单：这一批还有没有别的分组可去 */
const canMoveSelection = computed(() => canMoveKeys(selectedItems.value.map((item) => item.sha1)));

/**
 * 弹窗里的候选目标
 *
 * 排掉源**已经在**的那个分组：整组转移时就是源分组自己，单行 / 批量时是这一批
 * 共同的那个归属 —— 移过去是空操作。一批横跨两个分组时只排共同的那一个：
 * "把它们都挪到 A"对还在 B 里的那些仍然是有意义的动作。
 */
const groupPickTargets = computed(() => {
  const source = moveGroupSource.value;
  const own = source?.kind === "group" ? source.id : groupOfAll(sourceSha1s());
  return groups.groups.value.filter((g) => g.uuid !== own);
});

/** 弹窗标题：把这次是谁要移报出来，三种源共用这一个弹窗 */
const groupPickTitle = computed(() => {
  const source = moveGroupSource.value;
  if (!source) return t("resource.batchMoveTo");
  return source.kind === "group"
    ? t("resource.moveGroupTitle", { name: source.label })
    : t("resource.moveModTitle", { name: source.label });
});

/** 关掉"移到分组"弹窗：顺手清掉这次的源，免得下次打开时还按上一次的源算 */
function closeGroupPick() {
  groupPickOpen.value = false;
  moveGroupSource.value = null;
}

/** 菜单动作分发 */
function onGroupMenuAction(kind: GroupMenuAction) {
  const menu = groupMenu.value;
  if (!menu) return;
  switch (kind) {
    case "rename-group":
      openGroupForm(menu.id, menu.label);
      break;
    case "delete-group":
      removeGroup(menu.id, menu.label);
      break;
    case "select-all":
      groupToggleSelect(menu.id);
      break;
    case "enable-all":
      void groupSetState(menu.id, false);
      break;
    case "disable-all":
      void groupSetState(menu.id, true);
      break;
    case "move-all":
      groupMoveAll(menu.id, menu.label);
      break;
    case "move-out":
      void groupMoveOut(menu.id);
      break;
    case "delete-mods":
      groupRemoveMods(menu.id, menu.label);
      break;
  }
}

// ---------- 模组行右键菜单 ----------
//
// 单行与批量**共用一个菜单组件**（RowMenu）：没有选中项时是单行菜单，动作只作用在
// 右键那一行；有选中项时是批量菜单，动作作用在选中的那一批。菜单项在这里算好，
// 动作由 onMenuPick 分发到已有的单行 / 批量操作上。

/**
 * 打开着的菜单，`null` 表示没开
 *
 * `item` 为 `null` 表示这是**批量菜单**，动作作用在选中项上；否则是单行菜单。
 */
const rowMenu = ref<{
  x: number;
  y: number;
  title?: string;
  items: RowMenuItem[];
  item: ModItemDto | null;
} | null>(null);

/** 单行菜单项 */
function itemMenuItems(item: ModItemDto): RowMenuItem[] {
  const items: RowMenuItem[] = [
    // 启用 / 禁用放第一条：改状态是最常用的动作
    {
      id: item.disable ? "enable" : "disable",
      label: item.disable ? t("resource.enable") : t("resource.disable"),
      // 识别失败的模组改不了启用状态，勾选框也是禁用的
      disabled: item.fail,
    },
    { id: "note", label: t("resource.modNote") },
    { id: "open-folder", label: t("resource.openFolder") },
  ];
  // 依赖树：没声明任何前置时没什么可展开的，置灰
  items.push({
    id: "dep-tree",
    label: t("resource.depTree"),
    disabled: !item.dependencies.length,
  });
  // 分组那两条挨着：移到别的分组 / 移出分组，后者只在这一行确实在组里时才给
  items.push({
    id: "move",
    label: t("resource.batchMoveTo"),
    // 只有它自己那个分组时无处可去，置灰，见 canMoveKeys
    disabled: !canMoveKeys([item.sha1]),
    separated: true,
  });
  if (isGrouped(item)) items.push({ id: "ungroup", label: t("resource.ungroup") });
  items.push({ id: "remove", label: t("resource.delete"), danger: true, separated: true });
  return items;
}

/**
 * 批量菜单项：只放**对这一批每一项都成立**的动作
 *
 * 判据与分组菜单同一口径：识别失败的模组改不了启用状态，不计入。
 * 单行菜单里的「备注」「打开文件夹」对一批模组没有意义，所以不出现。
 */
function batchMenuItems(picked: ModItemDto[]): RowMenuItem[] {
  const items: RowMenuItem[] = [];
  if (picked.some((item) => !item.fail && item.disable)) {
    items.push({ id: "enable", label: t("resource.enable") });
  }
  if (picked.some((item) => !item.fail && !item.disable)) {
    items.push({ id: "disable", label: t("resource.disable") });
  }
  items.push({ id: "move", label: t("resource.batchMoveTo"), disabled: !canMoveSelection.value, separated: true });
  items.push({ id: "remove", label: t("resource.delete"), danger: true, separated: true });
  return items;
}

/** 打开右键菜单；列表视图与表格视图都抛这个事件上来 */
function openItemMenu(payload: { event: MouseEvent; item: ModItemDto }) {
  const x = payload.event.clientX;
  const y = payload.event.clientY;

  // 有选中项时，右键 = **批量动作**；用户口径是多选后菜单也要跟着变
  const picked = selectedItems.value;
  if (picked.length) {
    rowMenu.value = {
      x,
      y,
      title: t("resource.selectedCount", { n: picked.length }),
      items: batchMenuItems(picked),
      item: null,
    };
    return;
  }

  rowMenu.value = { x, y, items: itemMenuItems(payload.item), item: payload.item };
}

/** 这一行是否在某个自建分组里；不在的话「移出分组」是空操作，菜单里就不给 */
function isGrouped(item: ModItemDto): boolean {
  return !!item.sha1 && groups.groupOfKey.value.has(item.sha1);
}

/** 菜单动作分发 */
function onMenuPick(id: string) {
  const menu = rowMenu.value;
  if (!menu) return;

  // 批量：全部复用顶栏那几条批量操作
  if (!menu.item) {
    switch (id) {
      case "enable":
        void batchSetState(false);
        break;
      case "disable":
        void batchSetState(true);
        break;
      case "move":
        // 源就是当前选中项：`moveGroupSource` 清空即走"顶栏那条"分支
        moveGroupSource.value = null;
        groupPickOpen.value = true;
        break;
      case "remove":
        batchRemove();
        break;
    }
    return;
  }

  const item = menu.item;
  switch (id) {
    case "move":
      modMoveToGroup(item);
      break;
    case "ungroup":
      // 只改归属，**不动启用状态**：移完它回到「已启用 / 已禁用」那一块
      if (item.sha1) void groups.setGroup(null, [item.sha1]);
      break;
    case "enable":
      void setState([item], false);
      break;
    case "disable":
      void setState([item], true);
      break;
    case "note":
      openNote(item);
      break;
    case "open-folder":
      openFolder("mods", item.file);
      break;
    case "dep-tree":
      depTreeItem.value = item;
      break;
    case "remove":
      remove(item);
      break;
  }
}

// ---------- 搜索 ----------

/** 搜索词；**不持久化**，这是一次性的筛选而不是视图偏好，重开窗口该是干净的 */
const keyword = ref("");
const query = computed(() => keyword.value.trim().toLowerCase());

/**
 * 命中判定：按展示方式选字段（见 `modHit`），表格模式**再往里看一层**
 *
 * 表格里内置模组（jar-in-jar）是独立的行，所以内置命中时也要保留父行 —— 否则搜
 * 内置库的名字永远搜不出来；命中的内置行由 ModTable 自动展开显示。
 * 列表模式不递归：内置行跟着父行一起出现，父行命中就够了。
 *
 * - `item`: 一个模组条目
 * - `q`: 已转小写的搜索词
 * - `view`: 当前展示方式
 *
 * # 返回值
 *
 * 本条或它的任一内置模组命中即为 `true`
 */
function hit(item: ModItemDto, q: string, view: ModView): boolean {
  if (modHit(item, q, view)) return true;
  return view === "table" && item.jarInJar.some((child) => hit(child, q, view));
}

/** 当前要显示的模组（搜索词为空时就是全部）；两种模式搜的字段不同，见 `hit` */
const matched = computed(() => {
  const q = query.value;
  if (!q) return mods.value;
  const view = modView.value;
  return mods.value.filter((item) => hit(item, q, view));
});

/** 搜了但一个都没命中 */
const noMatch = computed(() => query.value !== "" && matched.value.length === 0);

// ---------- 分组块 ----------

/**
 * 状态分组 → 固定 uuid / 文案键
 *
 * 状态分组与自建分组**共用一套 uuid 键**，即 useModGroups 的 `STATE_GROUP_ID_*`：
 * 渲染身份、排序、折叠状态全用它，不必再维护"两套键互相映射"。
 *
 * 它们不是用户分组：不可改名 / 删除，归属由"启用 / 禁用"决定 ——
 * 只在**不属于任何自建分组**的模组之间分桶，见 [`sections`]。
 */
const STATE_KEYS: Record<ModStateGroup, { id: string; labelKey: string }> = {
  on: { id: STATE_GROUP_ID_ON, labelKey: "resource.groupOn" },
  off: { id: STATE_GROUP_ID_OFF, labelKey: "resource.groupOff" },
  fail: { id: STATE_GROUP_ID_FAIL, labelKey: "resource.groupFail" },
};

/** 分组视图里的一块：自建分组 或 状态分组 */
interface ModSection {
  /** 分组 uuid（自建与状态分组同一套键空间） */
  id: string;
  /** 分组名；自建分组是用户起的名字，状态分组是 `t()` 好的文案 */
  label: string;
  /** 这一块里的模组，顺序就是显示顺序 */
  items: ModItemDto[];
  /** 自建分组：可改名 / 删除，能接收「移到分组」 */
  custom: boolean;
  /** 状态分组的类型；自建分组没有 */
  state?: ModStateGroup;
}

/**
 * 分组视图的块：**按用户拖出来的顺序**，顺序表见 useModGroups 的 `order`
 *
 * - 自建分组：空的也留着，那是「移到分组」的去处，得看得见
 * - 「已启用 / 已禁用」：**空也显示**，这是用户点名的要求 —— 空着更是个提示
 * - 「识别失败」：空就收起来（没有坏包时它只是个噪音）
 *
 * 顺序表里可能有已删分组的旧 uuid，所以以实到的为准（找不到的跳过）。
 */
const sections = computed<ModSection[]>(() => {
  const of = groups.groupOfKey.value;
  // 有更新的先挑走：「有更新」块优先于自建分组与状态分组，一个模组只出现在一处
  const withUpdate = matched.value.filter(hasUpdate);
  // 剩下的、又不属于任何自建分组的，才按状态分桶
  const rest = matched.value.filter(
    (item) => !hasUpdate(item) && (!item.sha1 || !of.has(item.sha1)),
  );

  const byId = new Map<string, ModSection>();
  byId.set(UPDATE_GROUP_ID, {
    id: UPDATE_GROUP_ID,
    label: t("resource.updateGroup"),
    items: withUpdate,
    custom: false,
  });
  for (const group of groups.groups.value) {
    byId.set(group.uuid, {
      id: group.uuid,
      label: group.name,
      items: matched.value.filter(
        (item) => !hasUpdate(item) && !!item.sha1 && of.get(item.sha1) === group.uuid,
      ),
      custom: true,
    });
  }
  for (const state of ["on", "off", "fail"] as ModStateGroup[]) {
    const { id, labelKey } = STATE_KEYS[state];
    byId.set(id, {
      id,
      label: t(labelKey),
      items: rest.filter((item) => stateHit(item, state)),
      custom: false,
      state,
    });
  }

  const list: ModSection[] = [];
  // 「有更新」不在顺序表里，按口径固定画在最前
  const update = byId.get(UPDATE_GROUP_ID);
  if (update && visible(update)) list.push(update);
  for (const key of groups.order.value) {
    if (key === UPDATE_GROUP_ID) continue;
    const sec = byId.get(key);
    if (!sec || !visible(sec)) continue;
    list.push(sec);
  }
  // 兜底：分组是刚建的、顺序表还没回来时，别把它漏掉（后端也会在下次下发时补上）
  for (const [key, sec] of byId) {
    if (key !== UPDATE_GROUP_ID && !groups.order.value.includes(key) && visible(sec)) {
      list.push(sec);
    }
  }
  return list;
});

/** 一个模组是否属于某个状态分组 */
function stateHit(item: ModItemDto, state: ModStateGroup): boolean {
  if (state === "fail") return item.fail;
  if (item.fail) return false;
  return state === "off" ? item.disable : !item.disable;
}

/**
 * 这一块现在要不要显示
 *
 * 「识别失败」空着就收起来；自建分组与「已启用 / 已禁用」始终显示。
 * **搜索时例外**：搜索是对"看得见的东西"做过滤，此时空分组（含状态分组）一律收起 ——
 * 否则一搜索满屏都是空的「已启用 / 已禁用」，命中的那几条反而要找。
 */
function visible(sec: ModSection): boolean {
  if (query.value) return sec.items.length > 0;
  // 「有更新」只在这一轮检查有结果时存在，没有更新就整块收起来
  if (sec.id === UPDATE_GROUP_ID) return sec.items.length > 0;
  if (sec.custom) return true;
  return sec.state !== "fail" || sec.items.length > 0;
}

/** SHA1 → 列表里那一条；选中与归组都按内容哈希记，改启用状态要用 uuid 定位 */
const bySha1 = computed(() => {
  const map = new Map<string, ModItemDto>();
  for (const item of mods.value) {
    if (item.sha1) map.set(item.sha1, item);
  }
  return map;
});

/**
 * 分组头上的计数
 *
 * 自建分组两种状态都有，所以报"启用 / 禁用"；**状态分组只报项数** ——
 * 在「已启用」里再写一行"禁用 0"、在「已禁用」里写"启用 0"都是纯噪音（用户点名）。
 */
function counts(sec: ModSection): string {
  if (sec.items.length === 0) return "";
  if (!sec.custom) return t("resource.groupCountItems", { n: sec.items.length });
  const off = sec.items.filter((item) => item.disable).length;
  return t("resource.groupCounts", { n: sec.items.length, on: sec.items.length - off, off });
}

// ---------- 分组折叠 ----------

/**
 * 分组块的收起 / 展开
 *
 * 状态存在**实例的 `gui_setting.json`**，即 `Mod.GroupCollapsed`，见 useModGroups：
 * 用户明确要求记住（"已启用默认不展开"），而且换实例各记各的。
 * 键就是分组 uuid —— 与顺序表同一套，不必再转换。
 */
function isOpen(id: string): boolean {
  return !groups.isCollapsed(id);
}

/** 收起 / 展开这一块并落盘 */
function toggleCollapse(id: string) {
  groups.toggleCollapsed(id);
}

// ---------- 分组头拖拽排序 ----------

/**
 * 块的元素表，量位置算插入点用
 *
 * 用 `Map` 而不是 `querySelectorAll`：块的渲染顺序就是 `sections` 的顺序，
 * 按 uuid 直接结对更稳，uuid 也不适合写进选择器。
 */
const sectionEls = new Map<string, HTMLElement>();

function bindSection(id: string, el: Element | null) {
  if (el) sectionEls.set(id, el as HTMLElement);
  else sectionEls.delete(id);
}

/** 当前渲染的块 id；拖拽排序的坐标轴，与 DOM 里的顺序一致 */
const sectionIds = computed(() => sections.value.map((sec) => sec.id));

const {
  dragKey: draggingSection,
  insertAt: insertSectionAt,
  ghost: sectionGhost,
  onPointerDown: onSectionPointerDown,
  consumeSuppressClick: consumeSectionClick,
} = useReorderDrag({
  keys: () => sectionIds.value,
  rectOf: (key) => sectionEls.get(key)?.getBoundingClientRect() ?? null,
  onDrop: (from, insertAt) => void moveSection(from, insertAt),
});

/** 拖动副本与落点占位上显示的组名：被拖那一块的显示名 */
const sectionGhostLabel = computed(() => {
  const key = sectionGhost.value?.item;
  return key ? (sections.value.find((sec) => sec.id === key)?.label ?? "") : "";
});

/** 落点占位上的数量：被拖那一块的条目数 */
const sectionGhostCount = computed(() => {
  const key = sectionGhost.value?.item;
  return key ? (sections.value.find((sec) => sec.id === key)?.items.length ?? 0) : 0;
});

/**
 * 把某一块拖到第 `insertAt` 块之前
 *
 * 与左侧分类栏同一算法：摘掉被拖项后，目标下标在被拖项之后要减一。
 * 落盘的就是**分组 uuid**；状态分组与自建分组同一套键空间，不用转换。
 */
async function moveSection(from: string, insertAt: number) {
  // 「有更新」固定在最前，拖它没有意义；也不让它混进顺序表
  if (from === UPDATE_GROUP_ID) return;
  const ids = sectionIds.value;
  const index = ids.indexOf(from);
  if (index < 0) return;
  const next = [...ids];
  next.splice(index, 1);
  next.splice(Math.max(0, Math.min(next.length, insertAt > index ? insertAt - 1 : insertAt)), 0, from);

  // 只把"看得见的块"排了序，隐藏的块（空的识别失败等）不在里面 ——
  // 它们在顺序表里的位置由后端保留原样，这里按原顺序补回去
  const visible = new Set(ids);
  const hidden = groups.order.value.filter((id) => !visible.has(id));
  await groups.setOrder([...next.filter((id) => id !== UPDATE_GROUP_ID), ...hidden]);
}

/** 插入线画不画在第 index 块之前 */
function showSectionLine(index: number): boolean {
  return draggingSection.value !== null && insertSectionAt.value === index;
}

/**
 * 点分组头（箭头 / 名字那一带）→ 折叠或展开
 *
 * 名字那一带同时是**拖拽把手**，所以要先问一句"刚才那一下是不是拖完松手的"：
 * 拖过就把这次 click 吞掉，否则排完序会顺手把分组折叠了。
 * `consumeSuppressClick` 只在真的拖过时才返回 true，见 useDragGesture 的 `dragged`。
 */
function onSectionToggle(id: string) {
  if (consumeSectionClick()) return;
  toggleCollapse(id);
}

// ---------- 分组增删改 ----------

/**
 * 分组名弹窗
 *
 * `id` 为**空串**表示"新建"；否则是"重命名某个分组"（带 uuid）。
 * 名字只在这一个字段里，所以弹窗不再需要同时记"原名"。
 */
const groupForm = ref<{ id: string; name: string } | null>(null);
const groupBusy = ref(false);
/** 用户在名字框里打过字或点过保存：碰过之后才报红，弹窗刚开时不报警 */
const groupFormTouched = ref(false);

/**
 * 分组名不合法时的提示，空串表示合法
 *
 * 判定与后端 `resource_mod_group_add` / `_rename` 同一套：去掉首尾空白后为空、或与别的分组
 * 重名。重命名时要排除自己，否则一打开就报"已存在"。
 */
const groupFormError = computed(() => {
  const form = groupForm.value;
  if (!form) return "";
  const name = form.name.trim();
  if (!name) return t("err.groupEmpty");
  const dup = groups.groups.value.some((g) => g.uuid !== form.id && g.name === name);
  return dup ? t("err.groupExists") : "";
});

/** 现在要不要画红框与提示 */
const groupFormInvalid = computed(() => groupFormTouched.value && !!groupFormError.value);

/** 打开弹窗：不传 id 是新建，传分组 uuid 与当前名字是重命名 */
function openGroupForm(id = "", name = "") {
  groupForm.value = { id, name };
  groupFormTouched.value = false;
}

/** 提交分组名弹窗：有 id 就重命名，否则新建 */
async function saveGroupForm() {
  const form = groupForm.value;
  if (!form || groupBusy.value) return;
  // 名字不合法时给出红框与提示，不再静默什么都不做
  groupFormTouched.value = true;
  if (groupFormError.value) return;
  const name = form.name.trim();
  groupBusy.value = true;
  try {
    const ok = form.id ? await groups.rename(form.id, name) : await groups.add(name);
    if (ok) groupForm.value = null;
  } finally {
    groupBusy.value = false;
  }
}

/** 删除分组；传 uuid，确认框里显示的是分组名 */
function removeGroup(id: string, name: string) {
  askConfirm(
    t("resource.groupDelete"),
    t("resource.groupDeleteConfirm", { name }),
    () => groups.remove(id),
    // 分组数据不在资源列表里：删完不用重拉列表；重拉模组要重解析 jar，很慢
    { reload: false },
  );
}

// ---------- 模组备注 ----------

/**
 * 备注弹窗的草稿；`item` 是列表里那一条的引用，存完直接改它
 *
 * 备注存在 `gui_setting.json` 的 `Mod.ModName`（与 ColorMC 互通），
 * **不在模组列表里**，所以存完**不能重拉列表** —— 重拉一次要重新解析每个 jar
 * 的元数据（数秒），用户只是写一句备注而已。改本地那一份就够。
 */
const noteForm = ref<{ item: ModItemDto; text: string } | null>(null);
const noteBusy = ref(false);

/** 打开备注弹窗，草稿带上当前备注 */
function openNote(item: ModItemDto) {
  noteForm.value = { item, text: item.note };
}

/** 保存；`text` 传空串 = 清掉备注 */
async function saveNote(text: string) {
  const form = noteForm.value;
  if (!form || noteBusy.value) return;
  const value = text.trim();
  noteBusy.value = true;
  try {
    await setModNote(instanceUuid.value, form.item.file, value);
    form.item.note = value;
    noteForm.value = null;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    noteBusy.value = false;
  }
}

/**
 * 表格里双击备注**就地编辑**：与备注弹窗**同一个命令**（`setModNote`），
 * 同样只改本地那一份、不重拉列表（理由见上面那段注释）。
 */
async function saveNoteInline(item: ModItemDto, text: string) {
  if (noteBusy.value) return;
  const value = text.trim();
  noteBusy.value = true;
  try {
    await setModNote(instanceUuid.value, item.file, value);
    item.note = value;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    noteBusy.value = false;
  }
}

// ---------- 模组操作（两种视图共用） ----------

/** 单行启用 / 禁用 */
function toggle(item: ModItemDto) {
  // 单个就是"批量传一个"：走同一条路，同样不重扫、就地更新那一行
  void setState([item], !item.disable);
}

function remove(item: ModItemDto) {
  askDelete(item.name || item.file, () => deleteMod(instanceUuid.value, item.uuid));
}

onMounted(() => void groups.load());
</script>

<template>
  <ContentHead :data="data" :busy="depBusy || updateBusy">
    <template #actions>
      <!--
        有选中项时顶栏换成批量操作条，用户选的口径是"顶栏出现批量按钮"。
        整条替换而不是并列：选中之后的重点是"对这些项做什么"，
        搜索框 / 视图切换 / 新建分组这时都不该再占着位置。
      -->
      <template v-if="selectedCount">
        <span class="row-selected-count">{{ t("resource.selectedCount", { n: selectedCount }) }}</span>
        <!-- 按钮统一纯图标：名称走悬浮提示与 aria-label，占位由 .icon-only 收成方形 -->
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.enable')"
          :aria-label="t('resource.enable')" @click="batchSetState(false)">
          <GlyphIcon name="check" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.disable')"
          :aria-label="t('resource.disable')" @click="batchSetState(true)">
          <GlyphIcon name="ban" :size="14" />
        </button>
        <!-- 归组：开弹窗选目标，不用下拉 —— 下拉一次只看得见一项，
             而且"移到分组"会以占位项的形式混在选项里，看着像能选的东西。
             提示与弹窗标题共用同一条文案（不带省略号）。
             选中项都在同一个分组里、又没别的分组可去时置灰，见 canMoveSelection -->
        <button class="mini-btn icon-btn icon-only" :disabled="busy || !canMoveSelection"
          v-tip="t('resource.batchMoveTo')" :aria-label="t('resource.batchMoveTo')" @click="groupPickOpen = true">
          <GlyphIcon name="folder" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only danger" :disabled="busy" v-tip="t('resource.delete')"
          :aria-label="t('resource.delete')" @click="batchRemove">
          <GlyphIcon name="trash" :size="14" />
        </button>
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.clearSelection')"
          :aria-label="t('resource.clearSelection')" @click="clearSelection">
          <GlyphIcon name="close" :size="14" />
        </button>
      </template>

      <template v-else>
        <input v-model="keyword" class="mod-search" :placeholder="t('resource.searchPlaceholder')" spellcheck="false"
          autocomplete="off" />
        <SegmentedTabs :model-value="modView" :options="[
          { value: 'list', label: t('resource.viewList') },
          { value: 'table', label: t('resource.viewTable') },
        ]" @update:model-value="setModView($event as ModView)" />
        <button class="mini-btn icon-btn icon-only" :disabled="busy" v-tip="t('resource.groupAdd')"
          :aria-label="t('resource.groupAdd')" @click="openGroupForm()">
          <GlyphIcon name="folder-plus" :size="14" />
        </button>
        <!-- 依赖检测要重扫一遍模组，跑的时候按钮置灰并把图标转起来 -->
        <button class="mini-btn icon-btn icon-only" :disabled="depBusy || loading" v-tip="t('resource.depTest')"
          :aria-label="t('resource.depTest')" @click="runDepTest">
          <GlyphIcon :name="depBusy ? 'refresh' : 'puzzle'" :size="14" :class="{ 'dep-spin': depBusy }" />
        </button>
        <!-- 更新检查要重扫一遍模组、再逐个问平台；按钮只放图标，进度在工具条下方 -->
        <button class="mini-btn icon-btn icon-only" :disabled="updateBusy || loading"
          v-tip="t('resource.updateCheck')" :aria-label="t('resource.updateCheck')" @click="runUpdateCheck">
          <GlyphIcon :name="updateStage === 'check' ? 'refresh' : 'update'" :size="14"
            :class="{ 'dep-spin': updateStage === 'check' }" />
        </button>
        <!-- 下载更新：勾选要更新的模组再下；还没查出可更新的就无处可下，置灰 -->
        <button class="mini-btn icon-btn icon-only" :disabled="busy || updateBusy || loading || !updateResult"
          v-tip="t('resource.updateDownload')" :aria-label="t('resource.updateDownload')" @click="updateOpen = true">
          <GlyphIcon name="download" :size="14" />
        </button>
      </template>
    </template>
  </ContentHead>

  <!--
    更新进度：**整行显示在工具条下方**，不再挤在按钮右边；检查与下载共用这一条。
    检查阶段后端要先扫一遍 mods 目录才知道总数，那之前走不确定滚动条，拿到总数后转成确定进度条
  -->
  <LoaderQueryProgress :visible="updateBusy" kind="query" :step="updateProgress.done"
    :total="updateProgress.total"
    :label="t(updateStage === 'download' ? 'resource.updateDownloading' : 'resource.updateChecking')" />

  <!--
    加载中：与「下载整合包」窗口同一套骨架行，呼吸动画见 `styles/skeleton.css`

    模组这一档是真慢 —— 每个 jar 都要解析元数据，几百个包要好几秒，
    所以给骨架 + **扫描进度 `x/x`**，进度事件由后端逐文件发出，见 useResourceData
  -->
  <div v-if="loading" class="item-list">
    <ListSkeleton :done="modProgress.done" :total="modProgress.total" />
  </div>
  <div v-else-if="noMatch" class="empty-tip">{{ t("resource.searchEmpty") }}</div>

  <!--
    表格视图：**一张总表**（分组名就是第一列），不是每块一张小表
    —— 这样"哪个模组在哪个组"一眼看得到，也不必在每块上重复 15 列表头。
  -->
  <div v-else-if="modView === 'table'" class="item-list mod-groups mod-groups-table">
    <!--
      整张表共用一块白底：行不再各自一块白底。分两层是为了**滚到最右/最底不留空白**：
      - `.mod-table-panel`：白底卡片 + 内边距（白底与内容的呼吸位），**自己不滚**；
      - `.mod-table-scroll`：缩在卡片内边距里的滚动容器 —— 内边距不在它身上，
        所以它的可滚动区域正好等于内容，滚到头就是内容边缘，不会多出一块空白。
      滚动条因此落在白色底里面（用户口径）。
    -->
    <div class="mod-table-panel">
      <div class="mod-table-scroll">
        <template v-for="(sec, idx) in sections" :key="sec.id">
          <div v-if="showSectionLine(idx)" class="mod-group-insert">
          <span class="mod-group-name">{{ sectionGhostLabel }}</span>
          <span class="mod-group-count">{{ sectionGhostCount }}</span>
        </div>

          <section :ref="(el) => bindSection(sec.id, el as Element | null)" class="mod-group mod-group-table"
            :class="{ 'mod-group-dragging': draggingSection === sec.id }">
            <ModTable :items="sec.items" :busy="busy" :selected-keys="selected" :update-keys="updateKeys"
              :query="query" :group-label="sec.label"
              :group-key="sec.id" :group-open="isOpen(sec.id)" :custom="sec.custom" :dragging-group="draggingSection"
              :hide-header="idx > 0" :col-widths="colWidths" :sort="sort" @col-resize="setColWidth"
              @sort-change="setSort" @group-menu="openGroupMenu" @toggle="toggle" @remove="remove"
              @toggle-group="onSectionToggle(sec.id)" @drag-group="onSectionPointerDown($event, sec.id)"
              @note-save="saveNoteInline" @open-folder="openFolder('mods', $event.file)" @select="toggleSelect"
              @select-group="groupToggleSelect(sec.id)" @item-menu="openItemMenu" @row-click="onRowClick"
              :anchor-key="anchorKey" />
          </section>
        </template>
      </div>
    </div>
  </div>

  <!-- 列表视图：按块渲染，自建分组与三个状态分组各一块 -->
  <div v-else class="item-list mod-groups">
    <!-- 按块渲染，不用 section 上的 v-for：插入线要画在"两块之间"，得有个兄弟节点 -->
    <template v-for="(sec, idx) in sections" :key="sec.id">
      <!-- 拖分组头时的插入线：松手后这一块会落到这里 -->
      <div v-if="showSectionLine(idx)" class="mod-group-insert">
          <span class="mod-group-name">{{ sectionGhostLabel }}</span>
          <span class="mod-group-count">{{ sectionGhostCount }}</span>
        </div>

      <section :ref="(el) => bindSection(sec.id, el as Element | null)" class="mod-group" :class="{
        'mod-group-dragging': draggingSection === sec.id,
        'mod-group-empty': !sec.items.length,
      }">
        <div class="mod-group-head" @contextmenu.prevent="openGroupMenu({
          event: $event, id: sec.id, label: sec.label, custom: sec.custom, count: sec.items.length,
        })">
          <!--
            分组头分两块，**两块都能点开 / 收起**：
            - 箭头：只管折叠，不参与拖拽，`@pointerdown.stop` 挡住把手的按下事件；
            - 名字那一带：既是折叠的点击区，也是拖拽把手（按住拖动即调整分组顺序）。
              拖动结束时那一下 click 由 onSectionToggle 里的抑制标记吞掉。
          -->
          <button class="mod-group-toggle" :aria-label="t('resource.groupCollapse')" @pointerdown.stop
            @click.stop="toggleCollapse(sec.id)">
            <GlyphIcon class="mod-group-caret" :class="{ collapsed: !isOpen(sec.id) }" name="chevron-down" :size="13"
              :weight="2.4" />
          </button>
          <span class="mod-group-handle" v-tip="t('resource.groupDragTip')"
            @pointerdown="onSectionPointerDown($event, sec.id)" @click="onSectionToggle(sec.id)">
            <span class="mod-group-name">{{ sec.label }}</span>
          </span>
          <span class="mod-group-count">{{ counts(sec) }}</span>
          <!-- 分组的重命名 / 删除 / 整组批量操作都在**右键菜单**里，用户口径是撤掉原来那排
               悬停按钮，与表格视图同一套入口 -->
        </div>

        <div v-show="isOpen(sec.id)" class="mod-group-body">
          <p v-if="!sec.items.length" class="empty-tip">
            {{ t("resource.groupEmptyHint") }}
          </p>
          <ModList :items="sec.items" :busy="busy" :selected-keys="selected" :update-keys="updateKeys"
            @toggle="toggle" @remove="remove" @note="openNote" @open-folder="openFolder('mods', $event.file)"
            @select="toggleSelect" @item-menu="openItemMenu" @row-click="onRowClick" :anchor-key="anchorKey" />
        </div>
      </section>

      <!-- 尾部插入线：拖到最末一块之后 -->
      <div v-if="idx === sections.length - 1 && showSectionLine(sections.length)" class="mod-group-insert">
          <span class="mod-group-name">{{ sectionGhostLabel }}</span>
          <span class="mod-group-count">{{ sectionGhostCount }}</span>
        </div>
    </template>
  </div>

  <!-- 新建 / 重命名分组 -->
  <BaseModal v-if="groupForm" :title="groupForm.id ? t('resource.groupRenameTitle') : t('resource.groupAddTitle')"
    :closable="false" @close="groupForm = null">
    <label class="field-label">{{ t("resource.groupName") }}</label>
    <input v-model="groupForm.name" class="field-input" :class="{ 'is-invalid': groupFormInvalid }"
      :placeholder="t('resource.groupPlaceholder')" spellcheck="false" @input="groupFormTouched = true"
      @keydown.enter="saveGroupForm" />
    <p v-if="groupFormInvalid" class="error-text">{{ groupFormError }}</p>
    <div class="modal-actions">
      <BaseButton :disabled="groupBusy" @click="groupForm = null">
        {{ t("resource.cancel") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="groupBusy" @click="saveGroupForm">
        {{ t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>

  <!-- 模组备注：文件名只读展示；用户认的是名字，落盘的键是文件名 -->
  <BaseModal v-if="noteForm" :title="t('resource.modNoteTitle')" :closable="false" @close="noteForm = null">
    <label class="field-label">{{ noteForm.item.name || noteForm.item.file }}</label>
    <p class="mod-note-file" :title="noteForm.item.file">{{ noteForm.item.file }}</p>
    <textarea v-model="noteForm.text" class="field-input mod-note-input" :placeholder="t('resource.modNotePlaceholder')"
      spellcheck="false" @keydown.ctrl.enter="saveNote(noteForm.text)" />
    <div class="modal-actions">
      <BaseButton v-if="noteForm.item.note" :disabled="noteBusy" @click="saveNote('')">
        {{ t("resource.modNoteClear") }}
      </BaseButton>
      <BaseButton :disabled="noteBusy" @click="noteForm = null">
        {{ t("resource.cancel") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="noteBusy" @click="saveNote(noteForm.text)">
        {{ t("resource.save") }}
      </BaseButton>
    </div>
  </BaseModal>

  <!--
    批量「移到分组」：把自建分组一次列全，点哪一行就移到哪一组（点完即关）。
    比下拉快一步 —— 下拉要先展开、再选、再触发 change。

    **没有「移出分组」那一行**（用户要求）：那是"撤销归组"，混在这里会让"移到分组"
    多出一个反向选项；要移出就在模组行右键菜单里选「移出分组」。
    一个分组都没有时列表位置写「无分组」，下面用「新建分组」建一个
  -->
  <BaseModal v-if="groupPickOpen" :closable="false" @close="closeGroupPick" :title="groupPickTitle">
    <div class="group-pick-list">
      <button v-for="g in groupPickTargets" :key="g.uuid" class="group-pick-row" :disabled="busy"
        @click="pickGroup(g.uuid)">
        <GlyphIcon name="folder" :size="14" />
        <span class="group-pick-name">{{ g.name }}</span>
        <span class="group-pick-count">
          {{ t("resource.groupCountItems", { n: g.mods.length }) }}
        </span>
      </button>

      <!-- 一个可去的自建分组都没有：写明白"无分组"，别留一块空地看着像没加载出来。
           下面那个「新建分组」就是这时该做的事（用户要求） -->
      <p v-if="!groupPickTargets.length" class="empty-tip">
        {{ t("resource.groupNoGroups") }}
      </p>
    </div>

    <div class="modal-actions">
      <BaseButton :disabled="busy" @click="createGroupFromPick">
        {{ t("resource.groupAdd") }}
      </BaseButton>
      <BaseButton @click="closeGroupPick">{{ t("resource.cancel") }}</BaseButton>
    </div>
  </BaseModal>

  <!-- 拖动分组时的跟手副本：被拖那一块的组名 -->
  <DragGhost v-if="sectionGhost" :x="sectionGhost.x" :y="sectionGhost.y">
    <GlyphIcon name="folder" :size="14" />
    <span>{{ sectionGhostLabel }}</span>
  </DragGhost>

  <!-- 依赖图：选一个模组，展开它声明的强制 / 推荐前置；共用前置只画一个节点 -->
  <ModDepsModal v-if="depTreeItem && depGraph" :item="depTreeItem" :graph="depGraph"
    :problems="modDepProblems" @target="rerootGraph" @close="depTreeItem = null" />

  <!--
    改启用状态时问一句要不要顺带处理相关模组：
    禁用看"谁还需要它"，启用看"它要的前置是不是被禁用着"
  -->
  <BaseModal v-if="stateAsk" :width="460" :closable="false"
    :title="t(stateAsk.wantDisable ? 'resource.disableDepsTitle' : 'resource.enableDepsTitle')"
    @close="stateAsk = null">
    <p class="dep-note">
      {{ t(stateAsk.wantDisable ? "resource.disableDepsHint" : "resource.enableDepsHint", {
        name: stateAsk.picked.map((item) => item.name || item.file).join(", "),
        list: stateAsk.related.map((item) => item.name || item.file).join(", "),
      }) }}
    </p>
    <div class="modal-actions">
      <BaseButton @click="answerStateAsk(false)">
        {{ t(stateAsk.wantDisable ? "resource.disableDepsOnly" : "resource.enableDepsOnly") }}
      </BaseButton>
      <BaseButton variant="accent" @click="answerStateAsk(true)">
        {{ t(stateAsk.wantDisable ? "resource.disableDepsTogether" : "resource.enableDepsTogether") }}
      </BaseButton>
    </div>
  </BaseModal>

  <!-- 分组右键菜单：只负责画与定位，动作全部抛回这里执行，见 onGroupMenuAction -->
  <ModGroupMenu v-if="groupMenu" :x="groupMenu.x" :y="groupMenu.y" :label="groupMenu.label" :custom="groupMenu.custom"
    :count="groupMenu.count" :can-enable="groupMenu.canEnable" :can-disable="groupMenu.canDisable"
    :all-selected="groupMenu.allSelected" @action="onGroupMenuAction" @close="groupMenu = null" />

  <!-- 模组行右键菜单：单行与批量共用 RowMenu，菜单项见 itemMenuItems / batchMenuItems -->
  <RowMenu v-if="rowMenu" :x="rowMenu.x" :y="rowMenu.y" :items="rowMenu.items" :title="rowMenu.title" @pick="onMenuPick"
    @close="rowMenu = null" />

  <!--
    依赖检测结果：按种类分成几个可收起的下拉组（缺失 → 前置被禁用 → 版本不符），默认全展开。
    **有问题才开这个弹窗**，没问题在 runDepTest 里用一条轻提示，不留空弹窗
  -->
  <BaseModal v-if="depOpen" :title="t('resource.depTitle')" :closable="false" @close="depOpen = false">
    <div class="dep-list">
      <div v-for="g in depGroups" :key="g.kind" class="dep-group">
        <button class="dep-group-head" :class="{ collapsed: !isDepOpen(g.kind) }"
          @click="toggleDepGroup(g.kind)">
          <GlyphIcon class="dep-group-caret" name="chevron-down" :size="13" :weight="2.4" />
          <span class="dep-group-label">{{ t(DEP_KIND_LABEL[g.kind]) }}</span>
          <span class="dep-group-count">{{ g.items.length }}</span>
        </button>

        <div v-show="isDepOpen(g.kind)" class="dep-rows">
          <div v-for="(p, i) in g.items" :key="`${p.modid}-${i}`" class="dep-row">
            <div class="dep-body">
              <div class="dep-head">
                <span class="dep-modid">{{ p.modid }}</span>
                <span v-if="p.installed" class="dep-meta">
                  {{ t("resource.depInstalled", { version: p.installed }) }}
                </span>
                <span v-if="p.required !== '*'" class="dep-meta">
                  {{ t("resource.depRequired", { range: p.required }) }}
                </span>
              </div>
              <div v-if="p.requiredBy.length" class="dep-note">
                {{ t("resource.depBy", { list: p.requiredBy.join(", ") }) }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="modal-actions">
      <!-- 导出：把弹窗里这份结果整段复制走，方便贴给别人排查 -->
      <BaseButton @click="exportDepReport">
        <GlyphIcon name="document" :size="14" />
        {{ t("resource.exportClipboard") }}
      </BaseButton>
      <BaseButton variant="accent" @click="depOpen = false">{{ t("actions.confirm") }}</BaseButton>
    </div>
  </BaseModal>

  <!--
    重复安装提示：列表加载完由后端事件推来，自动弹，与手动触发的依赖检测分开。
    同名文件同时启用属安装冲突，不必等用户自己去点检测
  -->
  <BaseModal v-if="modDuplicates.length" :title="t('resource.modDupTitle')" :closable="false"
    @close="dismissModDuplicates">
    <p class="dep-note">{{ t("resource.modDupHint") }}</p>
    <div class="dep-list mod-dup-list">
      <div v-for="d in modDuplicates" :key="d.modid" class="dep-row">
        <span class="dep-modid">{{ d.modid }}</span>
        <div class="dep-body">
          <div v-for="f in d.files" :key="f.file" class="dep-note">{{ f.file }}</div>
        </div>
      </div>
    </div>

    <div class="modal-actions">
      <BaseButton @click="exportDuplicates">
        <GlyphIcon name="document" :size="14" />
        {{ t("resource.exportClipboard") }}
      </BaseButton>
      <BaseButton variant="accent" @click="dismissModDuplicates">{{ t("actions.confirm") }}</BaseButton>
    </div>
  </BaseModal>

  <!-- 更新下载：有更新才开；没更新在 runUpdateCheck 里用一条轻提示，不留空窗 -->
  <ModUpdateModal v-if="updateResult && updateOpen" :result="updateResult" @close="updateOpen = false"
    @download="applyUpdates" />
</template>

<!-- 备注弹窗的样式走 scoped：BaseModal 是 `Teleport to="body"` 的，弹窗不在
     `.resource-layout` 里，窗口级 resource.css 那套前缀规则够不着它。
     同窗口的 ServerFormModal 也是这么处理的 -->
<style scoped>
/* 文件名（落盘的键）压暗一行 */
.mod-note-file {
  margin: 0 0 8px;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mod-note-input {
  min-height: 96px;
  resize: vertical;
  line-height: 1.5;
}

/* ---------- 批量「移到分组」弹窗 ---------- */

/* 分组列表：整行可点，一行一组（比下拉一次只看得见一项强） */
.group-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  /* 分组可能很多：给个上限自己滚，别把弹窗撑出屏幕 */
  max-height: 320px;
  overflow-y: auto;
  /* 这里**不预留**滚动条槽位：预留的话行会整体缩进 9px，左端就与标题、
     右端就与下面的按钮错开。槽位由 BaseModal 的 `.modal` 统一在两侧留 */
  /* 见 styles/scrollbar.css */
}

/* 一行一个分组：整行可点。
   底色取 `--bg-hover` 而不是 `--bg-raised` —— 亮色主题下后者与弹窗底 `--bg-card`
   都是纯白，行就只剩一圈浅边框，"看不出是个能点的项目" */
.group-pick-row {
  display: flex;
  align-items: center;
  gap: 8px;
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

.group-pick-row:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
}

.group-pick-row:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 分组名可被挤压，数量不缩 */
.group-pick-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-pick-count {
  flex-shrink: 0;
  color: var(--text-dim);
  font-size: 11.5px;
}

/* ---------- 依赖检测 / 重复安装结果 ---------- */

/* 问题多时弹窗自己滚，别把动作按钮顶出屏幕 */
.dep-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 52vh;
  overflow-y: auto;
}

/* 重复那一档上面还有一行说明：列表再收一点，保证说明与按钮都留在视野里 */
.mod-dup-list {
  max-height: 46vh;
}

/* 依赖检测跑起来时图标转起来：要重扫一遍模组，几秒里得让人看出是在跑 */
.dep-spin {
  animation: dep-spin 0.8s linear infinite;
}

@keyframes dep-spin {
  to {
    transform: rotate(360deg);
  }
}

.dep-row {
  display: flex;
  gap: 10px;
  align-items: flex-start;
}

/* 下拉组的组头：箭头 + 种类名 + 条数；点整行收起 / 展开 */
.dep-group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 2px 0;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  text-align: left;
}

.dep-group-caret {
  transition: transform 0.12s;
}

.dep-group-head.collapsed .dep-group-caret {
  transform: rotate(-90deg);
}

.dep-group-count {
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 400;
}

.dep-rows {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 6px 0 2px 21px;
}

.dep-body {
  min-width: 0;
}

.dep-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 3px 10px;
}

.dep-modid {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

.dep-meta {
  color: var(--text-dim);
  font-size: 12px;
}

.dep-note {
  color: var(--text-dim);
  font-size: 11.5px;
  word-break: break-all;
}
</style>
