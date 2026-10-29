<script setup lang="ts">
// 映射大厅页签：列出可加入的房间，按房间名过滤；后端未接好时只显示提示
import { computed, onMounted, ref } from "vue";
import { commands, type MappingLobbyDto } from "../../lib/bindings";
import { t, tErr } from "../../lib/i18n";
import BaseButton from "../../components/ui/BaseButton.vue";
import HighlightText from "../../components/ui/HighlightText.vue";

/** 大厅快照；未加载时为 null */
const view = ref<MappingLobbyDto | null>(null);
/** 房间名过滤词，空串表示不过滤 */
const keyword = ref("");
/** 正在拉列表 */
const busy = ref(false);
/** 最近一次操作的错误文案，成功刷新时清空 */
const error = ref("");
/** 按过滤词筛出的房间 */
const rooms = computed(() => (view.value?.rooms ?? []).filter(room =>
  room.name.toLowerCase().includes(keyword.value.trim().toLowerCase())));

/** 重新拉一次房间列表 */
async function refresh() {
  busy.value = true;
  error.value = "";
  try { view.value = await commands.lobby.mappingList(); }
  catch (cause) { error.value = tErr(cause); }
  finally { busy.value = false; }
}

/**
 * 执行一个房间操作
 *
 * - `operation`: 后端认识的动作名，如 host / join
 * - `room`: 目标房间 id，建房时传空串
 */
async function action(operation: string, room: string) {
  try { await commands.lobby.mappingAction(operation, room); }
  catch (cause) { error.value = tErr(cause); }
}
onMounted(refresh);
</script>

<template>
  <section class="mapping">
    <p class="notice">{{ t('lobby.mappingPending') }}</p>
    <div class="toolbar">
      <input v-model="keyword" :aria-label="t('lobby.searchRooms')" :placeholder="t('lobby.searchRooms')" />
      <BaseButton :disabled="busy" @click="refresh">{{ t('lobby.refreshRooms') }}</BaseButton>
      <BaseButton :disabled="!view?.available || busy" @click="action('host', '')">{{ t('lobby.createRoom') }}</BaseButton>
    </div>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="!rooms.length" class="empty">{{ t('lobby.noRooms') }}</p>
    <article v-for="room in rooms" :key="room.id">
      <strong><HighlightText :text="room.name" :query="keyword" /></strong>
      <span>{{ t('lobby.players', { count: room.players }) }}</span>
      <BaseButton :disabled="!view?.available" @click="action('join', room.id)">{{ t('lobby.joinRoom') }}</BaseButton>
    </article>
  </section>
</template>

<style scoped>
.mapping { display: grid; gap: 14px; }
.notice, .empty { color: var(--text-dim); font-size: 13px; line-height: 1.7; }
.toolbar, article { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.toolbar input { flex: 1; min-width: 180px; padding: 8px 12px; border: 1px solid var(--border); border-radius: 8px; color: var(--text); background: var(--bg-card); }
.empty { padding: 48px 16px; text-align: center; border: 1px dashed var(--border); border-radius: 12px; }
article { padding: 16px; border: 1px solid var(--border); border-radius: 12px; }
article strong { flex: 1; }
.error { color: var(--red); }
</style>
