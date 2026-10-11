<script setup lang="ts">
// 启动器设置窗口：左侧标签导航 + 右侧内容区
// 标签：界面（含窗口设置）/ Java / 网络与下载 / 游戏启动 / 皮肤与头像 / 客户端设置
//
// 这里只做编排：状态与保存逻辑全在 composables/（每个标签一份），
// 排版在 parts/tabs/，每个标签一个组件；窗口级样式在 settings.css，非 scoped、子组件共用。
// 本文件负责：标签切换（记忆上次所在页）、Esc 关窗、分组级「恢复默认」的派发与高亮。
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import SettingsRail from "./parts/SettingsRail.vue";
import SettingsSearch from "./parts/SettingsSearch.vue";
import UiTab from "./parts/tabs/UiTab.vue";
import JavaTab from "./parts/tabs/JavaTab.vue";
import NetworkTab from "./parts/tabs/NetworkTab.vue";
import LaunchTab from "./parts/tabs/LaunchTab.vue";
import SkinTab from "./parts/tabs/SkinTab.vue";
import ClientTab from "./parts/tabs/ClientTab.vue";
import { t } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { KEYS, readString, writeString } from "../../lib/storage";
import { useSettingsUi } from "./composables/useSettingsUi";
import { useSettingsSkin } from "./composables/useSettingsSkin";
import { useSettingsNetwork } from "./composables/useSettingsNetwork";
import { useSettingsLaunch } from "./composables/useSettingsLaunch";
import { useSettingsJava } from "./composables/useSettingsJava";
import { useSettingsClient } from "./composables/useSettingsClient";
import { isSettingsTab, SETTINGS_TABS, type SettingsTab } from "./types";
import { groupsOfTab, RESETTABLE_GROUPS, type SettingsHit } from "./search";
import "./settings.css";

const emit = defineEmits<{ (e: "close"): void }>();

// ================= 标签（记住上次所在页） =================

function storedTab(): SettingsTab {
  const raw = readString(KEYS.settingsTab);
  return isSettingsTab(raw) ? raw : "ui";
}

const tab = ref<SettingsTab>(storedTab());

watch(tab, (v) => {
  writeString(KEYS.settingsTab, v);
});

// ================= 各区域状态 =================

const ui = useSettingsUi();
const skin = useSettingsSkin();
const network = useSettingsNetwork();
const launch = useSettingsLaunch();
const java = useSettingsJava();
const client = useSettingsClient();

// 游戏标题那组显示在客户端设置页，但数据属于 window 设置：这里取出给 ClientTab
const { win: launchWin } = launch;

/** 刚被搜索命中的分组：短暂高亮 */
const flashGroup = ref("");
let flashTimer: number | null = null;

function flash(id: string) {
  flashGroup.value = id;
  if (flashTimer !== null) clearTimeout(flashTimer);
  flashTimer = window.setTimeout(() => {
    flashTimer = null;
    flashGroup.value = "";
  }, 1600);
}

/** 各区域的 composable（顺序即"恢复默认"的派发顺序） */
function handlers() {
  return [ui, skin, network, launch, java, client];
}

/** 本页包含哪些分组（由搜索索引派生：能搜到的分组 = 会被重置的分组） */
const pageGroups = computed(() => groupsOfTab(tab.value));

/** 本页有没有可恢复的默认值（Java 页没有：列表是系统扫描出来的） */
const canResetPage = computed(() => pageGroups.value.some((id) => RESETTABLE_GROUPS.includes(id)));

/**
 * 恢复本页默认
 *
 * 只在内容区右上角保留这一个重置入口：分组级按钮撤掉了（原来皮肤页会同时出现"整页"与"分组"
 * 两个重置按钮，分不清该点哪个）。范围＝本页包含的分组，其它页不受影响。
 */
const confirmResetPage = ref(false);

async function onResetPage() {
  confirmResetPage.value = false;
  for (const id of pageGroups.value) {
    for (const h of handlers()) {
      if (await h.resetGroup(id)) break;
    }
  }
  showToast(t("winSettings.resetDone"));
}

// ================= 设置项搜索 =================

/** 命中搜索项：切到它的标签页，滚到分组锚点，短暂高亮 */
async function onSearchSelect(hit: SettingsHit) {
  tab.value = hit.tab;
  // 目标标签页是 v-if 挂上来的：等它渲染完再找锚点
  await nextTick();
  const el = document.getElementById(`set-${hit.group}`);
  el?.scrollIntoView({ block: "start", behavior: "smooth" });
  flash(hit.group);
}

// ================= 初始化 =================

/** 键盘监听：单窗口模式下窗口被 KeepAlive 缓存，切走时 onUnmounted 不会跑 ——
 *  必须自己在停用 / 启用之间装卸，否则在别的窗口按 Esc 会被当成"关闭本窗口" */
function bindKeys() {
  document.addEventListener("keydown", onKeyDown);
}

function unbindKeys() {
  document.removeEventListener("keydown", onKeyDown);
}

onMounted(async () => {
  bindKeys();
  // 各区域的加载互不依赖，一起发（失败的各自留空）
  await Promise.all([
    network.load(),
    launch.load(),
    java.load(),
    client.load(),
    client.refreshLockInstances(),
    client.refreshCustomHome(),
  ]);
});

// 单窗口模式：切走卸掉键盘监听，切回再装上（onActivated 首次挂载也会跑，重复 add 同一函数无副作用）
onActivated(bindKeys);
onDeactivated(unbindKeys);

onUnmounted(() => {
  unbindKeys();
  if (flashTimer !== null) clearTimeout(flashTimer);
});

/** 切到客户端设置页时刷新实例列表 / 自定义主页状态（设置窗口开着时它们可能已被改动） */
watch(tab, (v) => {
  if (v !== "client") return;
  void client.refreshLockInstances();
  void client.refreshCustomHome();
});

// ================= Esc 关窗 =================

function onKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  // 有弹窗时先让弹窗处理（Java 页的确认框自己听 Esc），别把整扇窗关掉
  if (document.querySelector(".modal-mask")) return;
  emit("close");
}
</script>

<template>
  <WindowFrame :title="t('features.settings')" body-fill @close="emit('close')">
    <!-- 设置项搜索放在标题栏（全局可用：切哪个标签都在，且不随内容区滚动） -->
    <template #head-right>
      <SettingsSearch @select="onSearchSelect" />
    </template>

    <div class="settings-layout">
      <SettingsRail v-model="tab" :tabs="SETTINGS_TABS" />

      <!-- 右侧内容区：每个标签一个面板，独立滚动 -->
      <div class="tab-content">
        <!-- 当前页大标题 + 恢复本页默认（整页只保留这一个重置入口） -->
        <header class="set-content-head">
          <div>
            <h2 class="set-content-title">{{ t(`winSettings.tab.${tab}`) }}</h2>
            <p class="set-content-sub">{{ t(`winSettings.tabDesc.${tab}`) }}</p>
          </div>
          <BaseButton v-if="canResetPage" size="sm" v-tip="t('winSettings.resetPageHint')"
            @click="confirmResetPage = true">
            {{ t("winSettings.resetPage") }}
          </BaseButton>
        </header>

        <section class="set-panel">
          <UiTab v-if="tab === 'ui'" :settings="ui" :flash-group="flashGroup" />
          <JavaTab v-else-if="tab === 'java'" :settings="java" :flash-group="flashGroup" />
          <NetworkTab v-else-if="tab === 'network'" :settings="network" :flash-group="flashGroup" />
          <LaunchTab v-else-if="tab === 'launch'" :settings="launch" :flash-group="flashGroup" />
          <SkinTab v-else-if="tab === 'skin'" :settings="skin" :flash-group="flashGroup" />
          <ClientTab v-else :settings="client" :win="launchWin" :flash-group="flashGroup" />
        </section>
      </div>
    </div>

    <!-- 恢复本页默认：只影响当前页，仍先确认一次 -->
    <BaseModal v-if="confirmResetPage" :title="t('winSettings.resetPageTitle')" @close="confirmResetPage = false">
      <p class="modal-text">
        {{ t("winSettings.resetPageConfirm", { page: t(`winSettings.tab.${tab}`) }) }}
      </p>
      <div class="modal-actions">
        <BaseButton @click="confirmResetPage = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="danger" @click="onResetPage">{{ t("winSettings.resetPage") }}</BaseButton>
      </div>
    </BaseModal>
  </WindowFrame>
</template>
