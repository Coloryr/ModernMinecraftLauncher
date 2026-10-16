<script setup lang="ts">
// 方块列表窗口：独立窗口承载方块网格，支持搜索 / 分类 / 设为实例图标
// 当前实例从 gui_config 的选中实例恢复（与主窗口共享同一份配置）
// 渲染进行中后端拒绝关闭，即 close-blocked 事件；这里弹二次确认，中断渲染再关
//
// 工具条放在标题栏的 head-right 槽里，含搜索 / 图标尺寸档 / 添加玩家头颅 / 重新渲染：
// 所以状态在这一层创建，面板与工具条共用同一份；BlockPanel 只接收它做排版。
import { onMounted, onUnmounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BlockPanel from "./BlockPanel.vue";
import BlockToolbar from "./parts/BlockToolbar.vue";
import SkinAddModal from "./parts/SkinAddModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t } from "../../lib/i18n";
import { api, blockRenderCancel, onCloseBlocked } from "../../lib/api";
import { loadGuiConfig } from "../../lib/guiConfig";
import { useBlockList } from "./composables/useBlockList";
import { useBlockGroups } from "./composables/useBlockGroups";
import type { InstanceInfoDto } from "../../lib/bindings";

const emit = defineEmits<{
  /** 请求关闭本窗口，由标题栏关闭按钮与 Esc 触发 */
  (e: "close"): void;
}>();

/** 自建分组 / 收藏 / 左栏顺序（用户数据，存本地） */
const groups = useBlockGroups();
/** 方块列表状态（面板与标题栏工具条共用）；分组与收藏从这里接进列表 */
const list = useBlockList({ inGroup: groups.inGroup, isFavorite: groups.isFavorite });
const { rendered, keyword, size, running, startRender, addSkin } = list;

const currentInstance = ref<InstanceInfoDto | null>(null);
let unlisten: (() => void) | null = null;

/** 关闭被渲染拦下时的二次确认框 */
const confirmClose = ref(false);

// ---------- 添加玩家头颅：入口在标题栏工具条上，所以弹窗跟着放这一层 ----------

const skinOpen = ref(false);
const skinBusy = ref(false);

async function onSkinSubmit(input: string) {
  if (skinBusy.value) return;
  skinBusy.value = true;
  try {
    // 成功才关弹窗：失败时提示已弹，输入内容留着让用户改
    if (await addSkin(input)) skinOpen.value = false;
  } finally {
    skinBusy.value = false;
  }
}

onMounted(async () => {
  await syncInstance();
  unlisten = await onCloseBlocked(() => {
    confirmClose.value = true;
  });
});

/** 当前实例；首次挂载与切回本窗口都走这里，实例可能已被主窗口切走 */
async function syncInstance() {
  try {
    const [cfg, instances] = await Promise.all([loadGuiConfig(), api.getInstances()]);
    const uuid = cfg?.mainWindow.selectedInstance;
    if (uuid) currentInstance.value = instances.find((i) => i.uuid === uuid) ?? null;
  } catch {
    currentInstance.value = null;
  }
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(syncInstance);

onUnmounted(() => unlisten?.());

/** 中断渲染并关闭：取消后 render_running 立即放行，再次关闭不会被拦 */
async function confirmCloseRender() {
  confirmClose.value = false;
  try {
    await blockRenderCancel();
  } catch {
    // 取消失败（渲染已结束等）不挡关窗
  }
  emit("close");
}
</script>

<template>
  <WindowFrame body-fill :title="t('home.blocks')" @close="emit('close')">
    <!-- 工具条放进标题栏：不占内容区高度，也不随内容滚动；未渲染时整条不出现 -->
    <template #head-right>
      <BlockToolbar v-if="rendered" :keyword="keyword" :size="size" :running="running"
        @update:keyword="keyword = $event" @update:size="size = $event" @add-skin="skinOpen = true"
        @re-render="startRender(true)" />
    </template>

    <BlockPanel :settings="list" :groups="groups" :current-instance="currentInstance" />

    <!-- 添加玩家头颅（用户名或 UUID） -->
    <SkinAddModal v-if="skinOpen" :busy="skinBusy" @submit="onSkinSubmit" @close="skinOpen = false" />

    <!-- 渲染中关窗：确认后中断渲染再关 -->
    <BaseModal v-if="confirmClose" :title="t('blocks.closeRenderTitle')" :closable="false"
      @close="confirmClose = false">
      <p class="delete-tip">{{ t("blocks.closeRenderConfirm") }}</p>
      <div class="modal-actions">
        <BaseButton @click="confirmClose = false">{{ t("add.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="confirmCloseRender">
          {{ t("blocks.closeRenderOk") }}
        </BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>
