<script setup lang="ts">
// 方块列表窗口：独立窗口承载方块网格，支持搜索 / 分类 / 设为实例图标
// 当前实例从 gui_config 的选中实例恢复（与主窗口共享同一份配置）
// 渲染进行中后端拒绝关闭，即 close-blocked 事件；这里弹二次确认，中断渲染再关
//
// 搜索、尺寸滑杆、添加玩家皮肤都在内容区，标题栏只留标题与窗口按钮。
import { onMounted, onUnmounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BlockPanel from "./BlockPanel.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useInstanceChange } from "../../composables/useInstanceChange";
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
const list = useBlockList({
  inGroup: groups.inGroup,
  isCustomGroup: groups.isCustomGroup,
  isFavorite: groups.isFavorite,
});
/** 主窗口当前选中的实例；设图标时高亮它 */
const currentInstance = ref<InstanceInfoDto | null>(null);
/** close-blocked 事件的取消订阅 */
let unlisten: (() => void) | null = null;

/** 多选模式：方块右键菜单的「多选」进入，勾选与设置分组在 BlockPanel 里 */
const multiSelect = ref(false);

/** 关闭被渲染拦下时的二次确认框 */
const confirmClose = ref(false);

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
    // 主窗口没有选中实例、或选中的已被删掉时一并清空，别留着上一份快照
    currentInstance.value = uuid ? (instances.find((i) => i.uuid === uuid) ?? null) : null;
  } catch {
    currentInstance.value = null;
  }
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(syncInstance);

// 实例在别的窗口里被增删改：当前实例跟着重算，删掉的直接清空
useInstanceChange(syncInstance);

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
    <BlockPanel :settings="list" :groups="groups" :current-instance="currentInstance" :multi-select="multiSelect"
      @update:multi-select="multiSelect = $event" />

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
