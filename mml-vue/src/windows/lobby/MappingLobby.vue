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
  <section class="mapping grid gap-3.5">
    <p class="notice text-title leading-[1.7] text-dim">{{ t('lobby.mappingPending') }}</p>
    <div class="toolbar flex flex-wrap items-center gap-2.5">
      <input class="min-w-[180px] flex-1 rounded-ctl border border-border bg-card px-3 py-2 text-fg" v-model="keyword"
      :aria-label="t('lobby.searchRooms')" :placeholder="t('lobby.searchRooms')" />
      <BaseButton :disabled="busy" @click="refresh">{{ t('lobby.refreshRooms') }}</BaseButton>
      <BaseButton :disabled="!view?.available || busy" @click="action('host', '')">{{ t('lobby.createRoom') }}
      </BaseButton>
    </div>
    <p v-if="error" class="error text-red" role="alert">{{ error }}</p>
    <p v-if="!rooms.length" class="empty rounded-panel border border-dashed border-border px-4 py-12 text-center text-title leading-[1.7] text-dim">{{ t('lobby.noRooms') }}</p>
    <article v-for="room in rooms" :key="room.id" class="flex flex-wrap items-center gap-2.5 rounded-panel border border-border p-4">
      <strong class="flex-1">
        <HighlightText :text="room.name" :query="keyword" />
      </strong>
      <span>{{ t('lobby.players', { count: room.players }) }}</span>
      <BaseButton :disabled="!view?.available" @click="action('join', room.id)">{{ t('lobby.joinRoom') }}</BaseButton>
    </article>
  </section>
</template>
