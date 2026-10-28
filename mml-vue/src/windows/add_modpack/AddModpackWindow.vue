<script setup lang="ts">
// 下载整合包窗口：CurseForge / Modrinth 在线搜索 + 选版本安装
// 安装是多任务的：命令立即返回，进度由 add-modpack-status 事件驱动，
// 顶部进度条显示任务数与详情，见 ModpackInstallBar。
import { onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import { api } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useModpackStatus } from "../../lib/modpackTasks";
import { useUnlisteners } from "../../composables/useUnlisteners";
import type { GroupDto } from "../../lib/bindings";
import ModpackMode from "./ModpackMode.vue";

defineEmits<{
  /** 请求关闭本窗口 */
  (e: "close"): void;
}>();

/** 详情是否展开：详情铺满窗口后，返回键放到标题栏（标题栏由本组件持有） */
const detailOpen = ref(false);
/** 详情状态在子组件里，标题栏的返回键只能通过它暴露的 closeDetail 收起 */
const modeRef = ref<InstanceType<typeof ModpackMode> | null>(null);

/** 分组（空 = 默认分组）：输入框里是组名，提交时由 api.resolveGroupId 转成 uuid */
const group = ref("");
const groups = ref<GroupDto[]>([]);

/** 安装任务状态：本窗口负责显示，终态 toast 由这里弹 */
const { status: modpackStatus, init: initModpackStatus } = useModpackStatus(true);

/** 事件订阅卸载时统一注销：onMounted 里的 await 之后登记 onUnmounted 是无效写法 */
const { track } = useUnlisteners();

/** 安装在线整合包：实例名取自整合包元数据，命令立即返回（任务后台安装） */
async function install(p: {
  source: string;
  projectId: string;
  fileId: string;
  fileName: string;
}) {
  try {
    // 分组按 uuid 传：手输的组名可能已有、也可能要现建一个
    const groupId = await api.resolveGroupId(group.value);
    await api.installModpack(p.source, p.projectId, p.fileId, groupId);
  } catch (e) {
    showToast(t("add.createFail", { msg: tErr(e) }));
  }
}

onMounted(async () => {
  track(initModpackStatus());

  try {
    // 默认分组（空白名字）不进下拉：输入框留空即默认分组
    groups.value = (await api.getGroups()).filter((g) => g.name.trim());
  } catch {
    groups.value = [];
  }
});
</script>

<template>
  <!-- hide-modpack-indicator 不再传：安装进度改成标题栏上的指示器 + 弹窗
       （ModpackTitleIndicator / ModpackPopup），与资源下载那一套一致，内容区不再内嵌进度卡片 -->
  <WindowFrame :title="t('winTitle.addModpack')" :back="detailOpen ? t('modpack.back') : ''" body-gutter
    @back="modeRef?.closeDetail()" @close="$emit('close')">
    <div class="modpack-body">
      <ModpackMode ref="modeRef" :group="group" :groups="groups" :status="modpackStatus" @update:group="group = $event"
        @install="install" @detail="detailOpen = $event" />
    </div>
  </WindowFrame>
</template>

<style scoped>
.modpack-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 安装进度条也是这一列里的卡片：边缘与筛选卡、项目卡一样落在窗口内容区边距（26px）上，
   不再单独回缩 8px —— 那 8px 是项目列表给悬停阴影留的，已在 ModpackMode 里用负外边距抵掉 */
</style>
