<script setup lang="ts">
// 启动器设置窗口：左侧标签导航 + 右侧内容区
// 标签：界面（含窗口设置）/ Java / 网络与下载 / 游戏启动 / 皮肤与头像 / 客户端设置 / 本地文件
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
import LocalFilesTab from "./parts/tabs/LocalFilesTab.vue";
import ExperimentalTab from "./parts/tabs/ExperimentalTab.vue";
import { t } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { KEYS, readString, writeString } from "../../lib/storage";
import { useSettingsUi } from "./composables/useSettingsUi";
import { useSettingsSkin } from "./composables/useSettingsSkin";
import { useSettingsNetwork } from "./composables/useSettingsNetwork";
import { useSettingsLaunch } from "./composables/useSettingsLaunch";
import { useSettingsJava } from "./composables/useSettingsJava";
import { useSettingsClient } from "./composables/useSettingsClient";
import { useSettingsLocalFiles } from "./composables/useSettingsLocalFiles";
import { isSettingsTab, SETTINGS_TABS, type SettingsTab } from "./types";
import { groupsOfTab, RESETTABLE_GROUPS, type SettingsHit } from "./search";
import "./settings.css";

const emit = defineEmits<{
  /** 请求关闭本窗口，由标题栏关闭按钮与 Esc 触发 */
  (e: "close"): void;
}>();

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
// 本地文件的统计只在打开那一页时做，见 LocalFilesTab 的 onMounted
const files = useSettingsLocalFiles();

// ================= 锁定客户端设置 =================

/** 本次打开设置窗口是否已用密码解锁；不跨窗口、不落盘 */
const clientUnlocked = ref(false);
const unlockOpen = ref(false);
const unlockPassword = ref("");
const unlockError = ref("");
/** 搜索命中锁定分组时，解锁成功后再跳过去 */
const pendingHit = ref<SettingsHit | null>(null);

/** 客户端设置是否被锁定 */
const clientLocked = computed(() => client.client.value.lockClient);

/** 锁定时要隐藏的设置分组：服主定制内容 + 高级 / 危险项，见 docs 的锁定说明 */
const LOCKED_GROUPS = [
  "servers", "loginLock", "instanceLock", "autoSave", "customHome",
  "customCss", "customPage", "music", "gameTitle", "serverPack",
  "theme", "window-look", "bgImage", "jvm", "gameArgs", "launchCmd", "experimental",
];

/** 本次是否处于锁定状态；解锁后本次设置窗口全部放开 */
const locked = computed(() => clientLocked.value && !clientUnlocked.value);

/**
 * 这个分组本次是否隐藏
 *
 * - `id`: 分组 id，见 search.ts 的索引
 *
 * # 返回值
 *
 * 锁着且属于锁定范围时返回 `true`
 */
function isLockedGroup(id: string): boolean {
  return locked.value && LOCKED_GROUPS.includes(id);
}

/** 左侧导航：未锁定，或本次已解锁时才显示「客户端设置」 */
const visibleTabs = computed(() =>
  SETTINGS_TABS.filter(
    (item) => item.id !== "client" || !clientLocked.value || clientUnlocked.value,
  ),
);

/** 第一次加载配置期间不把「刚锁定」误判成需要踢回首页 */
let loadingConfig = true;

watch(
  () => client.client.value.lockClient,
  (value) => {
    if (loadingConfig || !value) return;
    // 刚在本页设置完密码：本次留在客户端设置页，让用户能看到「已锁定 / 解除」
    clientUnlocked.value = true;
  },
  // 加载配置时同步触发，避免 loadingConfig 复位后才跑到回调、把初始锁定误判成刚解锁
  { flush: "sync" },
);

/** 打开解锁弹窗 */
function openUnlock(hit: SettingsHit | null = null) {
  pendingHit.value = hit;
  unlockPassword.value = "";
  unlockError.value = "";
  unlockOpen.value = true;
}

/** 校验密码并放行本次设置窗口 */
async function unlockClientSettings() {
  if (!unlockPassword.value) {
    unlockError.value = t("winSettings.clientLockWrong");
    return;
  }
  const ok = await client.verifyClientLock(unlockPassword.value);
  if (!ok) {
    unlockError.value = t("winSettings.clientLockWrong");
    return;
  }
  clientUnlocked.value = true;
  unlockOpen.value = false;
  unlockPassword.value = "";
  unlockError.value = "";
  const hit = pendingHit.value;
  pendingHit.value = null;
  if (hit) {
    await onSearchSelect(hit);
  } else {
    tab.value = "client";
  }
}

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

/** 各区域的 composable，顺序即"恢复默认"的派发顺序 */
function handlers() {
  return [ui, skin, network, launch, java, client];
}

/** 本页包含哪些分组；由搜索索引派生，锁定中的分组不参与"恢复本页默认" */
const pageGroups = computed(() =>
  groupsOfTab(tab.value).filter((id) => !isLockedGroup(id)),
);

/** 本页有没有可恢复的默认值；Java 页没有，列表是系统扫描出来的 */
const canResetPage = computed(() => pageGroups.value.some((id) => RESETTABLE_GROUPS.includes(id)));

/**
 * 恢复本页默认
 *
 * 只在内容区右上角保留这一个重置入口：分组级按钮撤掉了，原来皮肤页会同时出现"整页"与
 * "分组"两个重置按钮，分不清该点哪个。范围＝本页包含的分组，其它页不受影响。
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
  // 目标分组或整个客户端设置页锁着：先要密码，解锁后再跳转
  if (locked.value && (hit.tab === "client" || LOCKED_GROUPS.includes(hit.group))) {
    openUnlock(hit);
    return;
  }
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
  loadingConfig = false;
  if (clientLocked.value && tab.value === "client") {
    tab.value = "ui";
  }
});

// 单窗口模式：切走卸掉键盘监听，切回再装上；onActivated 首次挂载也会跑，重复 add 同一函数无副作用
onActivated(() => {
  bindKeys();
  // 单窗口模式下组件被缓存：切走再回来要重新锁上
  if (clientLocked.value && tab.value === "client") tab.value = "ui";
});
onDeactivated(() => {
  unbindKeys();
  clientUnlocked.value = false;
  unlockOpen.value = false;
  unlockPassword.value = "";
  unlockError.value = "";
});

onUnmounted(() => {
  unbindKeys();
  if (flashTimer !== null) clearTimeout(flashTimer);
});

/** 切到客户端设置页时刷新实例列表 / 自定义主页状态；设置窗口开着时它们可能已被改动 */
watch(tab, (v) => {
  if (v !== "client") return;
  void client.refreshLockInstances();
  void client.refreshCustomHome();
});

// ================= Esc 关窗 =================

function onKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  // 有弹窗时先让弹窗处理：Java 页的确认框自己听 Esc，别把整扇窗关掉
  if (document.querySelector(".modal-mask")) return;
  emit("close");
}
</script>

<template>
  <WindowFrame :title="t('features.settings')" body-fill body-flush @close="emit('close')">
    <!-- 设置项搜索放在标题栏：全局可用，切哪个标签都在，且不随内容区滚动 -->
    <template #head-right>
      <SettingsSearch @select="onSearchSelect" />
    </template>

    <div class="settings-layout">
      <SettingsRail v-model="tab" :tabs="visibleTabs" />

      <!-- 右侧内容区：每个标签一个面板，独立滚动 -->
      <div class="tab-content">
        <!-- 当前页大标题 + 恢复本页默认（整页只保留这一个重置入口） -->
        <header class="set-content-head">
          <div>
            <h2 class="set-content-title">{{ t(`winSettings.tab.${tab}`) }}</h2>
            <p class="set-content-sub">{{ t(`winSettings.tabDesc.${tab}`) }}</p>
            <p v-if="locked && tab !== 'experimental'" class="locked-hint">{{ t("winSettings.lockedHint") }}</p>
          </div>
          <BaseButton v-if="canResetPage" size="sm" v-tip="t('winSettings.resetPageHint')"
            @click="confirmResetPage = true">
            {{ t("winSettings.resetPage") }}
          </BaseButton>
        </header>

        <section class="set-panel">
          <UiTab v-if="tab === 'ui'" :settings="ui" :look="client" :locked="locked" :flash-group="flashGroup" />
          <JavaTab v-else-if="tab === 'java'" :settings="java" :flash-group="flashGroup" />
          <NetworkTab v-else-if="tab === 'network'" :settings="network" :flash-group="flashGroup" />
          <LaunchTab v-else-if="tab === 'launch'" :settings="launch" :locked="locked" :flash-group="flashGroup" />
          <SkinTab v-else-if="tab === 'skin'" :settings="skin" :flash-group="flashGroup" />
          <ClientTab v-else-if="tab === 'client' && (clientUnlocked || !clientLocked)" :settings="client"
            v-model:win="launchWin" :flash-group="flashGroup" />
          <LocalFilesTab v-else-if="tab === 'files'" :files="files" :flash-group="flashGroup" />
          <ExperimentalTab v-else :settings="client" :client-unlocked="clientUnlocked" :flash-group="flashGroup"
            @unlock="openUnlock()" />
        </section>
      </div>
    </div>

    <!-- 解锁客户端设置：密码错误只在本弹窗里提示，不关窗 -->
    <BaseModal v-if="unlockOpen" :title="t('winSettings.clientLockUnlockTitle')" @close="unlockOpen = false">
      <p class="modal-text">{{ t("winSettings.clientLockUnlockPrompt") }}</p>
      <input v-model="unlockPassword" class="field-input" type="password" spellcheck="false"
        @keyup.enter="unlockClientSettings" />
      <p v-if="unlockError" class="error-text">{{ unlockError }}</p>
      <div class="modal-actions">
        <BaseButton @click="unlockOpen = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton variant="primary" @click="unlockClientSettings">
          {{ t("winSettings.clientLockUnlock") }}
        </BaseButton>
      </div>
    </BaseModal>

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
