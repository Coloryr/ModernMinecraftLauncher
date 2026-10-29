<script setup lang="ts">
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import MappingLobby from "./MappingLobby.vue";
import { commands, type InstanceInfoDto, type LobbyViewDto } from "../../lib/bindings";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { isTauri, multiWindow, prepareLobby, releaseLobby } from "../windowManager";
import { useInstanceChange } from "../../composables/useInstanceChange";
import { copyText } from "../../lib/clipboard";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

/** 当前页签：陶瓦联机 / 映射大厅 */
const tab = ref<"terracotta" | "mapping">("terracotta");
const view = ref<LobbyViewDto | null>(null);
const instances = ref<InstanceInfoDto[]>([]);
const instance = ref("");
const room = ref("");
const selectedNodes = ref<string[]>([]);
const busy = ref(false);
const loading = ref(true);
const error = ref("");
let active = false;
let generation = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

/** 后端大厅状态机当前状态；拿不到快照时按 stopped 处理 */
const state = computed(() => view.value?.state.state || "stopped");
/** 处于可建房 / 可加入的等待态 */
const waiting = computed(() => state.value === "waiting");
/** 实例已选、联机组件就绪且状态允许时才放行「启动游戏」 */
const canLaunch = computed(() => !!instance.value && view.value?.ready &&
  ["waiting", "host-scanning", "host-starting", "host-ok", "guest-ok"].includes(state.value));
/** 状态机文案，键为 lobby.state.* */
const stateText = computed(() => t(`lobby.state.${state.value}`));

/**
 * 应用一帧大厅快照
 *
 * - `next`: 后端返回的最新快照
 *
 * 轮询请求可能乱序回来，旧会话或旧状态的一帧直接丢掉，免得界面倒退。
 */
function applyView(next: LobbyViewDto) {
  const previous = view.value;
  if (previous && next.sessionGeneration < previous.sessionGeneration) return;
  // 同一会话中的慢轮询不能覆盖更新的房间状态。
  if (previous && next.sessionGeneration === previous.sessionGeneration && next.state.index < previous.state.index) {
    next.state = previous.state;
  }
  if (!previous?.nodes.length && next.nodes.length) selectedNodes.value = [...next.nodes];
  else selectedNodes.value = selectedNodes.value.filter((node) => next.nodes.includes(node));
  view.value = next;
}

/**
 * 轮询一次大厅状态，结束后再排下一轮
 *
 * - `ticket`: 本次激活的世代号，与 generation 不符说明页面已切走或换过会话，直接停
 */
async function poll(ticket: number) {
  if (!active || ticket !== generation) return;
  try {
    const next = await commands.lobby.status();
    if (active && ticket === generation) { applyView(next); error.value = ""; }
  } catch (cause) {
    if (active && ticket === generation) error.value = tErr(cause);
  } finally {
    if (active && ticket === generation) timer = setTimeout(() => void poll(ticket), 700);
  }
}

/** 页面激活：准备大厅会话、拉实例列表，然后开始轮询 */
async function activate() {
  if (active) return;
  active = true;
  const ticket = ++generation;
  loading.value = true;
  error.value = "";
  view.value = null;
  busy.value = false;
  try {
    if (!isTauri()) throw new Error(t("lobby.desktopOnly"));
    const prepared = await prepareLobby();
    const list = await commands.main.getInstances();
    if (!active || ticket !== generation) return;
    applyView(prepared);
    instances.value = list;
    if (!list.some((item) => item.uuid === instance.value)) instance.value = "";
    void poll(ticket);
  } catch (cause) {
    if (active && ticket === generation) error.value = tErr(cause);
  } finally {
    if (active && ticket === generation) loading.value = false;
  }
}

/** 页面离开：停轮询、作废在途请求，单窗口模式下释放大厅会话 */
function deactivate() {
  if (!active) return;
  active = false;
  generation++;
  clearTimeout(timer);
  // 真实窗口由 Rust 关闭事件收尾，单窗口页面由离开动作释放。
  if (!multiWindow.value && isTauri()) releaseLobby();
}

/**
 * 执行一个大厅操作，随后刷新一次快照
 *
 * - `operation`: 要执行的后端命令
 */
async function run(operation: () => Promise<unknown>) {
  if (busy.value) return;
  const ticket = generation;
  busy.value = true;
  error.value = "";
  try {
    await operation();
    const next = await commands.lobby.status();
    if (active && ticket === generation) applyView(next);
  } catch (cause) {
    if (active && ticket === generation) error.value = tErr(cause);
  } finally {
    if (active && ticket === generation) busy.value = false;
  }
}

/**
 * 房间操作：建房 / 加入 / 离开
 *
 * - `action`: 后端认识的房间动作名
 */
function roomAction(action: string) {
  return run(() => commands.lobby.roomAction(action, room.value.trim(), selectedNodes.value));
}

/** 下载 / 刷新联机组件 */
function start() {
  return run(() => commands.lobby.start());
}

/** 用选中实例启动游戏并进入联机 */
function launchGame() {
  return run(() => commands.lobby.launchGame(instance.value));
}

/**
 * 复制房间码 / 地址到剪贴板
 *
 * - `value`: 要复制的文本
 */
async function copy(value: string) {
  if (await copyText(value)) showToast(t("account.copied"));
  else showToast(t("account.copyFail"));
}

/** 实例在别处被增删改：重拉下拉，删掉的那个自动取消选中 */
async function syncInstanceList() {
  const list = await commands.main.getInstances().catch(() => [] as InstanceInfoDto[]);
  instances.value = list;
  if (!list.some((item) => item.uuid === instance.value)) instance.value = "";
}

useInstanceChange(syncInstanceList);

onMounted(activate);
onActivated(activate);
onDeactivated(deactivate);
onBeforeUnmount(deactivate);
</script>

<template>
  <WindowFrame :title="t('winTitle.lobby')" body-gutter @close="$emit('close')">
    <div class="lobby-page">
      <p v-if="loading" role="status">{{ t('lobby.verifying') }}</p>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <template v-if="view && !loading">
        <div class="account-banner">
          <strong>{{ view.account }}</strong>
          <span>{{ t('lobby.accountLocked') }}</span>
        </div>
        <nav class="tabs" :aria-label="t('winTitle.lobby')">
          <button :class="{ selected: tab === 'terracotta' }" :aria-pressed="tab === 'terracotta'"
            @click="tab = 'terracotta'">{{ t('lobby.terracotta') }}</button>
          <button :class="{ selected: tab === 'mapping' }" :aria-pressed="tab === 'mapping'"
            @click="tab = 'mapping'">{{ t('lobby.mapping') }}</button>
        </nav>
        <MappingLobby v-if="tab === 'mapping'" />
        <template v-else>
          <section class="panel setup-panel">
            <div class="section-head">
              <div>
                <h2>{{ t('lobby.terracotta') }}</h2>
                <span class="muted">{{ view.version || t('lobby.downloadHint') }}</span>
              </div>
              <BaseButton :disabled="busy || view.downloading" @click="start">
                {{ t(view.ready ? 'lobby.refreshNodes' : 'lobby.start') }}
              </BaseButton>
            </div>
            <div v-if="view.downloading" class="download" role="status">
              <progress max="100" :value="view.downloadPercent" />
              <span>{{ t('lobby.downloading', { percent: view.downloadPercent }) }}</span>
            </div>
            <p class="state" role="status">{{ stateText }}</p>
            <p v-if="view.state.exception !== null" class="error">{{ t(`lobby.exception.${view.state.exception}`) }}</p>
          </section>
          <section class="panel">
            <label class="field" for="lobby-instance">
              <span>{{ t('lobby.instance') }}</span>
              <select id="lobby-instance" v-model="instance" :disabled="busy">
                <option value="" disabled>{{ t('lobby.selectInstance') }}</option>
                <option v-for="item in instances" :key="item.uuid" :value="item.uuid">{{ item.name }}</option>
              </select>
            </label>
            <p v-if="!instances.length" class="muted">{{ t('lobby.noInstances') }}</p>
            <div class="actions">
              <BaseButton variant="primary" :disabled="busy || !canLaunch"
                @click="launchGame">{{ t('lobby.launchGame') }}</BaseButton>
            </div>
          </section>
          <div class="room-grid">
            <section class="panel">
              <h2>{{ t('lobby.host') }}</h2>
              <p class="muted">{{ t('lobby.hostHint') }}</p>
              <BaseButton :disabled="busy || !waiting" @click="roomAction('host')">{{ t('lobby.createRoom') }}</BaseButton>
            </section>
            <section class="panel">
              <h2>{{ t('lobby.join') }}</h2>
              <label class="field" for="lobby-room">
                <span>{{ t('lobby.roomCode') }}</span>
                <input id="lobby-room" v-model="room" maxlength="256" :disabled="busy || !waiting"
                  placeholder="U/XXXX-XXXX-XXXX-XXXX" @keydown.enter="waiting && room.trim() && roomAction('join')" />
              </label>
              <BaseButton :disabled="busy || !waiting || !room.trim()" @click="roomAction('join')">{{ t('lobby.joinRoom') }}</BaseButton>
            </section>
          </div>
          <section v-if="view.ready && !waiting" class="panel">
            <div class="section-head">
              <h2>{{ t('lobby.currentRoom') }}</h2>
              <BaseButton :disabled="busy" @click="roomAction('leave')">{{ t('lobby.leave') }}</BaseButton>
            </div>
            <div v-if="view.state.room" class="copy-row">
              <code>{{ view.state.room }}</code>
              <BaseButton @click="copy(view.state.room)">{{ t('lobby.copyCode') }}</BaseButton>
            </div>
            <div v-if="view.state.address" class="copy-row">
              <span>{{ t('lobby.address') }} <code>{{ view.state.address }}</code></span>
              <BaseButton @click="copy(view.state.address)">{{ t('lobby.copyAddress') }}</BaseButton>
            </div>
            <p v-if="view.state.address" class="muted">{{ t('lobby.joinHint') }}</p>
            <p v-if="view.state.difficulty" class="muted">{{ t('lobby.difficulty') }} {{ view.state.difficulty }}</p>
            <ul v-if="view.state.profiles.length" class="members">
              <li v-for="member in view.state.profiles" :key="member.id">
                <strong>{{ member.name }}</strong>
                <span class="muted">{{ member.kind === 'HOST' ? t('lobby.host') : member.vendor }}</span>
              </li>
            </ul>
          </section>
          <details v-if="view.nodes.length" class="panel">
            <summary>{{ t('lobby.nodes') }} · {{ selectedNodes.length }}</summary>
            <p class="muted">{{ t('lobby.nodesHint') }}</p>
            <label v-for="node in view.nodes" :key="node" class="node">
              <input v-model="selectedNodes" type="checkbox" :value="node" :disabled="busy || !waiting" />
              <span>{{ node }}</span>
            </label>
          </details>
          <button class="credit" @click="commands.main.openUrl('https://github.com/burningtnt/Terracotta').catch(cause => showToast(tErr(cause)))">
            Terracotta / 陶瓦联机 © burningtnt · AGPL-3.0 + IPC exception
          </button>
        </template>
        <p class="muted close-hint">{{ t('lobby.closeHint') }}</p>
      </template>
    </div>
  </WindowFrame>
</template>

<style scoped>
.lobby-page { display: flex; flex-direction: column; gap: 14px; max-width: 1040px; margin: 0 auto; }
.account-banner, .section-head, .copy-row, .actions { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.account-banner { padding: 12px 16px; background: var(--accent-soft); border: 1px solid var(--accent-border); border-radius: 10px; }
.account-banner span, .muted { color: var(--text-dim); font-size: var(--ctl-font); line-height: 1.6; }
.tabs { display: flex; gap: 6px; border-bottom: 1px solid var(--border); padding-bottom: 8px; }
.tabs button { padding: 8px 16px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--text-dim); cursor: pointer; }
.tabs button.selected { background: var(--accent-soft); border-color: var(--accent-border); color: var(--accent); }
.panel { padding: 16px; border: 1px solid var(--border); background: var(--bg-card); border-radius: 12px; display: flex; flex-direction: column; align-items: stretch; gap: 12px; min-width: 0; }
.panel h2 { font-size: 15px; margin: 0; }
.panel p { margin: 0; }
.section-head { justify-content: space-between; }
.section-head > div { display: grid; gap: 6px; }
.room-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
.field { display: flex; flex-direction: column; gap: 8px; font-size: var(--ctl-font); }
.field input, .field select { width: 100%; min-width: 0; min-height: var(--ctl-h); padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-raised); color: var(--text); font: inherit; }
.state { color: var(--accent); font-weight: 600; font-size: 13px; }
.error { color: var(--red); overflow-wrap: anywhere; }
.download { display: flex; align-items: center; gap: 12px; font-size: var(--ctl-font); }
.download progress { flex: 1; min-width: 0; accent-color: var(--accent); }
.copy-row { justify-content: space-between; }
.copy-row code { overflow-wrap: anywhere; user-select: text; }
.members { padding: 0; margin: 0; list-style: none; display: grid; gap: 8px; }
.members li { display: flex; justify-content: space-between; gap: 12px; font-size: 13px; }
details.panel { display: block; }
summary { cursor: pointer; font-size: 13px; }
details.panel p { margin: 10px 0; }
.node { display: flex; gap: 8px; margin-top: 8px; font-size: var(--ctl-font); overflow-wrap: anywhere; }
.credit { border: 0; padding: 4px; background: transparent; color: var(--accent); font-size: var(--ctl-font); cursor: pointer; align-self: center; }
.close-hint { text-align: center; margin: 0; }
@media (max-width: 620px) { .room-grid { grid-template-columns: 1fr; } }
</style>
