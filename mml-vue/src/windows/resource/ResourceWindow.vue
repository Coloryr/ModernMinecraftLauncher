<script setup lang="ts">
// 资源管理窗口：左侧分类导航 + 右侧内容区（存档分类下另有「数据包」子页）
// 数据来自 resource_* 命令；删除进回收站，操作后重拉当前分类。
//
// 这个文件只做编排：
// - 数据与加载        → composables/useResourceData；分类 / 子页选择也在那儿，加载跟着它走
// - 共用操作          → composables/useResourceOps；忙碌标记 / 确认弹窗 / 打开目录 / 重拉
// - 各分类的列表      → parts/*Pane.vue（每类一个，自带的弹窗跟上）
// - 窗口级样式        → resource.css；非 scoped，拆开的子组件共用，见该文件头部说明
import { computed, onMounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { commands } from "../../lib/bindings";
import { useFileDrop } from "../../composables/useFileDrop";
import type { FileDropPayload } from "../../composables/useFileDrop";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { openWindow } from "../windowManager";
import { useResourceData } from "./composables/useResourceData";
import { useResourceOps } from "./composables/useResourceOps";
import { useResourceView } from "./composables/useResourceView";
import CategoryRail from "./parts/CategoryRail.vue";
import ConfirmModal from "./parts/ConfirmModal.vue";
import DatapackPane from "./parts/DatapackPane.vue";
import DropConflictModal from "./parts/DropConflictModal.vue";
import DropOverlay from "../../components/ui/DropOverlay.vue";
import InstanceChip from "./parts/InstanceChip.vue";
import ModPane from "./parts/ModPane.vue";
import PackPane from "./parts/PackPane.vue";
import SavePane from "./parts/SavePane.vue";
import SchematicPane from "./parts/SchematicPane.vue";
import ScreenshotPane from "./parts/ScreenshotPane.vue";
import ServerPane from "./parts/ServerPane.vue";
import ShaderPane from "./parts/ShaderPane.vue";
import { dropKindOf, RESOURCE_CATEGORIES, type CategoryId } from "./types";
import "./resource.css";

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次"返回"会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
defineEmits<{
  /** 窗口要关掉，由外壳真正执行 */
  (e: "close"): void;
}>();

/**
 * 视图偏好（分类顺序 / 上次打开的类别 / 模组展示方式）
 *
 * 存在**实例**的 `gui_setting.json` 里，所以要先有实例才能读 —— 数据层建好之后
 * 把"读偏好"挂成它的实例就绪回调（见 useResourceData 的 setInstanceReadyHook），
 * 在它拉列表**之前**跑，第一次就按记住的顺序与类别显示。
 */
const data = useResourceData();
const view = useResourceView(data);
const ops = useResourceOps(data);
const { category, saveTab, instance, instanceUuid, dpSave, sync } = data;
const { confirmBox } = ops;
const { load: loadView } = view;

data.setInstanceReadyHook(async () => {
  await loadView();
  // 读回来的"上次类别"要落到当前选择上（数据层的 watch 会跟着拉对应列表）
  category.value = view.initialCategory.value;
});

// 记住这次看的是哪一类：下次打开直接停在这儿
watch(category, (id) => view.rememberCategory(id));

onMounted(sync);

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次（实例可能已被主窗口切走）
useWindowRefresh(sync);

/** 切分类：只改选择，加载由 useResourceData 里的 watch 负责 */
function selectCategory(id: CategoryId) {
  category.value = id;
}

/**
 * 内容区切换动画的 key（见模板里的 `<Transition name="pane-swap">`）
 *
 * 分类变了才换 key，动画随之触发。**存档那一类的两个子页共用一个分类 id**，
 * 所以子页也要进 key —— 否则在「存档 ↔ 数据包」之间切换时 key 没变、动画不触发。
 */
const paneKey = computed(() =>
  category.value === "saves" ? `saves:${saveTab.value}` : category.value,
);

/** 「下载资源」：开「添加资源」窗口，与主窗口侧边栏那个入口同一个去处 */
function openDownload() {
  openWindow("add_resource");
}

// ================= 拖拽导入（落点 = 当前面板对应的目录） =================

/** 待处理的拖入：已经预检出同名冲突，等用户在弹窗里选处理方式 */
const dropPending = ref<{
  kind: string;
  parent: string | null;
  paths: string[];
  conflicts: string[];
} | null>(null);
/** 有导入在跑：弹窗按钮置灰 */
const dropBusy = ref(false);

/** 当前面板对应的后端 kind */
const dropKind = computed(() => dropKindOf(category.value, saveTab.value));

/**
 * 当前面板的显示名（遮罩文案里报"落到哪一类"）
 *
 * 存档下的**数据包子页**要用子页的名字（「数据包」），不能报成「存档」——
 * 两者落点不同（`saves/` 与 `saves/<存档>/datapacks`）。
 */
const dropLabel = computed(() => {
  if (category.value === "saves" && saveTab.value === "datapacks") {
    return t("resource.datapacks");
  }
  const item = RESOURCE_CATEGORIES.find((c) => c.id === category.value);
  return item ? t(item.labelKey) : "";
});

/**
 * 数据包那一页的 parent（存档目录名）
 *
 * 数据包挂在某个存档下（`saves/<存档目录名>/datapacks`），落点必须连它一起传；
 * 其它分类传 null。数据包页的下拉框选的就是 `dpSave`（见 DatapackPane）。
 */
const dropParent = computed(() => (dropKind.value === "datapacks" ? dpSave.value : null));

/**
 * 这次拖入接不接，**只在悬停阶段问一次**，见 useFileDrop 的 accept 说明
 *
 * 不接的内容**连拖都拖不进来**：`dragover` 不放行 → 系统画"禁止"光标、松手也不会有 drop。
 * 悬停阶段 DOM 只给 `types` 与能认出来时的 `isDirectory`，所以这里只判得了两件事：
 * 认出来是目录 → 不接；没带文件（文本 / 网页卡片）→ 不接。
 * 目录与登录快捷方式在悬停时认不全，**松手后由 onFilesDropped 再过滤一次**。
 *
 * 数据包页还得先选中一个存档，否则无处可放。
 */
async function canAcceptDrop(payload: FileDropPayload) {
  if (!dropKind.value || !instanceUuid.value) return false;
  if (dropKind.value === "datapacks" && !dpSave.value) return false;
  if (payload.isDirectory === true) return false;
  return payload.types.includes("Files");
}

/** 松手：先滤掉目录 / 快捷方式，再预检同名冲突；有冲突就弹窗问，没有就直接导入 */
async function onFilesDropped(payload: FileDropPayload) {
  const kind = dropKind.value;
  const uuid = instanceUuid.value;
  const parent = dropParent.value;
  if (!kind || !uuid) return;
  if (kind === "datapacks" && !parent) return;

  // 这一页只落"资源文件"：目录与登录快捷方式，如 .url / .webloc，跳过
  const paths: string[] = [];
  try {
    for (const path of payload.paths) {
      const probe = await commands.windows.probeDrop(path);
      if (probe.kind !== "folder" && probe.kind !== "shortcut") {
        paths.push(path);
      }
    }
  } catch (e) {
    showToast(tErr(e));
    return;
  }
  if (!paths.length) {
    // 悬停时没认出目录、松手才滤空的，即 `webkitGetAsEntry` 拿不到：给一句说明，别静默
    showToast(t("resource.dropUnsupported"));
    return;
  }

  try {
    const check = await commands.resource.importFiles(uuid, kind, paths, "check", parent);
    if (check.conflicts.length) {
      dropPending.value = { kind, parent, paths, conflicts: check.conflicts };
      return;
    }
    await runImport(kind, parent, paths, "overwrite");
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 真正落盘并重拉当前列表 */
async function runImport(kind: string, parent: string | null, paths: string[], mode: string) {
  if (dropBusy.value) return;
  dropBusy.value = true;
  try {
    const result = await commands.resource.importFiles(instanceUuid.value, kind, paths, mode, parent);
    if (result.imported.length) {
      showToast(t("resource.dropImported", { count: result.imported.length }));
    } else if (result.skipped.length) {
      showToast(t("resource.dropSkipped", { count: result.skipped.length }));
    }
    // 磁盘上多/换了文件：重拉当前分类；模组会重解析 jar，但这是导入后必须付的代价
    await data.reloadCurrent();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    dropBusy.value = false;
  }
}

/** 冲突弹窗里选完 */
async function answerDrop(mode: string) {
  const pending = dropPending.value;
  dropPending.value = null;
  if (!pending) return;
  await runImport(pending.kind, pending.parent, pending.paths, mode);
}

const { fileDragOver } = useFileDrop(onFilesDropped, { accept: canAcceptDrop });
</script>

<template>
  <WindowFrame :title="t('resource.title')" @close="$emit('close')">
    <!-- 标题栏：当前实例（原先挤在左侧分类栏顶部）+ 下载资源入口 -->
    <template #head-right>
      <InstanceChip :instance="instance" />
      <!-- BaseButton 本身就是 inline-flex + gap，图标与文字自然成一行，不用额外类 -->
      <BaseButton size="sm" variant="accent" @click="openDownload">
        <GlyphIcon name="download" :size="14" />
        {{ t("resource.download") }}
      </BaseButton>
    </template>

    <div class="resource-layout">
      <CategoryRail :data="data" :view="view" @select="selectCategory" />

      <section class="cat-content">
        <!--
          切分类加一段淡入淡出（与主窗口的 view-swap 同一套观感）。
          `<Transition>` 不产生 DOM，所以 .cat-content 的 flex 布局不受影响；
          但里面的 `.pane-swap` 要**复刻**「顶栏 + 列表」之间的纵向间距（gap 12px），
          否则包一层后间距会丢（主窗口的 .detail-instance 也是这么处理的）。
        -->
        <Transition name="pane-swap" mode="out-in">
          <!-- 没有选中实例：只给这一条引导，不再顺带铺一个空列表，原先两段会同时出现 -->
          <div v-if="!instance" key="__no-instance" class="empty-tip">
            {{ t("resource.notSelected") }}
          </div>
          <div v-else :key="paneKey" class="pane-swap">
            <ScreenshotPane v-if="category === 'screenshots'" :data="data" :ops="ops" />
            <ServerPane v-else-if="category === 'servers'" :data="data" :ops="ops" />
            <ModPane v-else-if="category === 'mods'" :data="data" :ops="ops" :view="view" />
            <PackPane v-else-if="category === 'resourcepacks'" :data="data" :ops="ops" />
            <ShaderPane v-else-if="category === 'shaders'" :data="data" :ops="ops" />
            <SchematicPane v-else-if="category === 'schematics'" :data="data" :ops="ops" />
            <SavePane v-else-if="saveTab === 'saves'" :data="data" :ops="ops" />
            <DatapackPane v-else :data="data" :ops="ops" />
          </div>
        </Transition>
      </section>

      <!-- 删除 / 清空确认（全窗口共用一份，内容来自 ops） -->
      <ConfirmModal v-if="confirmBox" :ops="ops" @close="confirmBox = null" />

      <!-- 拖拽导入：同名冲突问一次（覆盖 / 跳过 / 重命名），选完才落盘 -->
      <DropConflictModal v-if="dropPending" :conflicts="dropPending.conflicts" :busy="dropBusy"
        @close="dropPending = null" @pick="answerDrop" />
    </div>

    <!-- 拖入文件时的遮罩：标题报出落到哪一类；目录 / 卡片不接，见 canAcceptDrop -->
    <DropOverlay v-if="fileDragOver" :title="t('resource.dropTitleTo', { name: dropLabel })" />
  </WindowFrame>
</template>
