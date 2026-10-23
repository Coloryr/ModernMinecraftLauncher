<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { commands, type MappingLobbyDto } from "../../lib/bindings";
import { t, tErr } from "../../lib/i18n";
import BaseButton from "../../components/ui/BaseButton.vue";

const view = ref<MappingLobbyDto | null>(null);
const keyword = ref("");
const busy = ref(false);
const error = ref("");
const rooms = computed(() => (view.value?.rooms ?? []).filter(room =>
  room.name.toLowerCase().includes(keyword.value.trim().toLowerCase())));

async function refresh() {
  busy.value = true;
  error.value = "";
  try { view.value = await commands.lobby.mappingList(); }
  catch (cause) { error.value = tErr(cause); }
  finally { busy.value = false; }
}

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
      <strong>{{ room.name }}</strong>
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
