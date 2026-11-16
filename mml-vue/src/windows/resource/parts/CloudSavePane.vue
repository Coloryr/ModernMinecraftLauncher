<script setup lang="ts">
// 云同步对话框：连接配置与存档状态分区，覆盖和删除必须先确认。
import { computed, ref, watch } from "vue";
import { commands, type CloudConfigDto, type CloudStatusDto, type SaveItemDto } from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { formatSize, formatTime } from "../../../lib/format";
import { stripFormatting } from "../../../lib/formatting";
import { showToast } from "../../../lib/toast";
import BaseButton from "../../../components/ui/BaseButton.vue";
import BaseModal from "../../../components/ui/BaseModal.vue";

const props = defineProps<{
  /** 当前本地实例 UUID。 */
  uuid: string;
  /** 本地存档，按目录名与云端匹配。 */
  saves: SaveItemDto[];
  /** 本地资源操作正在执行。 */
  disabled: boolean;
}>();
const emit = defineEmits<{
  /** 文件变化后刷新本地存档列表。 */
  (event: "changed"): void;
  /** 同步时锁住资源页上的其它操作。 */
  (event: "busy", value: boolean): void;
}>();
const open = ref(false);
const busy = ref(false);
const error = ref("");
const config = ref<CloudConfigDto | null>(null);
const status = ref<CloudStatusDto | null>(null);
const saved = ref("");
const pending = ref<{ name: string; direction: "upload" | "download" | "delete" } | null>(null);
const dirty = computed(() => config.value && JSON.stringify(config.value) !== saved.value);
const rows = computed(() => {
  const names = new Set([...props.saves.map((save) => save.dir), ...(status.value?.worlds.map((world) => world.name) ?? [])]);
  return [...names].sort((a, b) => a.localeCompare(b)).map((name) => ({
    name,
    local: props.saves.find((save) => save.dir === name),
    remote: status.value?.worlds.find((world) => world.name === name),
  }));
});

/** 请求始终使用当前对话框的连接，避免其它窗口保存配置后切换目的地。 */
function connectionSnapshot(): CloudConfigDto {
  if (!config.value) throw "err.cloudSettings";
  return { ...config.value };
}

/** 串行执行操作，异常留在对话框中方便修正配置。 */
async function run(action: () => Promise<void>) {
  if (busy.value || props.disabled) return;
  busy.value = true;
  error.value = "";
  emit("busy", true);
  try {
    await action();
  } catch (reason) {
    error.value = tErr(reason);
  } finally {
    busy.value = false;
    emit("busy", false);
  }
}

/** 每次打开读取后端配置，防止其它实例改了服务器却仍显示旧值。 */
async function show() {
  open.value = true;
  status.value = null;
  pending.value = null;
  await run(async () => {
    config.value = await commands.resource.cloudGetConfig(props.uuid);
    saved.value = JSON.stringify(config.value);
    if (config.value.server) status.value = await commands.resource.cloudStatus(props.uuid, connectionSnapshot());
  });
}

/** 保存连接后立即验证密钥并刷新云端状态。 */
async function connect() {
  await run(async () => {
    if (!config.value) return;
    const snapshot = connectionSnapshot();
    await commands.resource.cloudSaveConfig(props.uuid, snapshot);
    saved.value = JSON.stringify(snapshot);
    status.value = null;
    status.value = await commands.resource.cloudStatus(props.uuid, snapshot);
    showToast(t("cloud.connected"));
  });
}

/** 刷新配额和云存档。 */
async function refresh() {
  await run(async () => { status.value = await commands.resource.cloudStatus(props.uuid, connectionSnapshot()); });
}

/** 暂停实例同步时仍保留云端存档。 */
async function toggle() {
  await run(async () => {
    const snapshot = connectionSnapshot();
    await commands.resource.cloudSetEnabled(props.uuid, snapshot, !status.value?.enabled);
    status.value = await commands.resource.cloudStatus(props.uuid, snapshot);
  });
}

/** 先确认上传覆盖、下载替换或云端删除。 */
function request(name: string, direction: "upload" | "download" | "delete") {
  pending.value = { name, direction };
}

/** 确认后执行同步；下载原目录已由后端保留备份。 */
async function transfer() {
  const action = pending.value;
  if (!action) return;
  await run(async () => {
    const snapshot = connectionSnapshot();
    const changed = await commands.resource.cloudTransfer(props.uuid, snapshot, action.name, action.direction);
    pending.value = null;
    emit("changed");
    showToast(t(changed ? "cloud.done" : "cloud.unchanged"));
    status.value = await commands.resource.cloudStatus(props.uuid, snapshot);
  });
}

/** 打开云同步自动备份目录。 */
async function backups() {
  await run(async () => { await commands.resource.cloudOpenBackups(props.uuid); });
}

watch(() => props.uuid, () => {
  open.value = false;
  config.value = null;
  status.value = null;
});
</script>

<template>
  <BaseButton size="sm" :disabled="disabled" @click="show">{{ t("cloud.title") }}</BaseButton>
  <BaseModal v-if="open" :title="t('cloud.title')" :width="820" :close-disabled="busy" @close="open = false">
    <div class="cloud-layout grid grid-cols-[minmax(200px,260px)_minmax(0,1fr)] gap-6">
      <aside v-if="config" class="cloud-connection flex flex-col gap-2 border-r border-border pr-5">
        <h4>{{ t("cloud.connection") }}</h4>
        <label class="field-label" for="cloud-server">{{ t("cloud.server") }}</label>
        <input id="cloud-server" v-model="config.server" class="field-input" :disabled="busy" type="url"
          spellcheck="false" />
        <label class="field-label" for="cloud-server-key">{{ t("cloud.serverKey") }}</label>
        <input id="cloud-server-key" v-model="config.serverKey" class="field-input" :disabled="busy" type="password"
          autocomplete="off" />
        <label class="field-label" for="cloud-client-key">{{ t("cloud.clientKey") }}</label>
        <input id="cloud-client-key" v-model="config.clientKey" class="field-input" :disabled="busy" type="password"
          autocomplete="off" />
        <label class="field-label" for="cloud-uuid">{{ t("cloud.remoteUuid") }}</label>
        <input id="cloud-uuid" v-model="config.remoteUuid" class="field-input" :disabled="busy" spellcheck="false" />
        <p class="field-hint">{{ t("cloud.bindingHint") }}</p>
        <BaseButton :disabled="busy || !config.server || !config.remoteUuid" @click="connect">
          {{ t("cloud.saveConnect") }}
        </BaseButton>
        <p class="field-hint">{{ t("cloud.manualHint") }}</p>
      </aside>
      <section class="cloud-worlds flex min-w-0 flex-col gap-3" :aria-label="t('cloud.worlds')">
        <div v-if="status" class="cloud-status flex items-center justify-between gap-3 rounded-card bg-accent-soft p-3">
          <div>
            <strong>{{ t(status.enabled ? "cloud.enabled" : "cloud.paused") }}</strong>
            <p class="field-hint">{{ t("cloud.storage", {
              used: formatSize(status.used), quota: formatSize(status.quota)
              }) }}</p>
          </div>
          <BaseButton size="sm" :disabled="busy || !!dirty" @click="toggle">
            {{ t(status.enabled ? "cloud.pause" : "cloud.enable") }}
          </BaseButton>
        </div>
        <div class="cloud-toolbar flex items-center justify-between gap-3">
          <h4>{{ t("cloud.worlds") }}</h4>
          <BaseButton size="sm" :disabled="busy || !config?.server || !!dirty" @click="refresh">{{ t("cloud.refresh") }}
          </BaseButton>
        </div>
        <p v-if="dirty && status" class="field-hint">{{ t("cloud.unsaved") }}</p>
        <p v-if="!status" class="empty-tip">{{ t("cloud.connectHint") }}</p>
        <p v-else-if="!status.enabled" class="empty-tip">{{ t("cloud.enableHint") }}</p>
        <div v-else class="cloud-list flex max-h-[360px] flex-col gap-2.5 overflow-y-auto">
          <p v-if="!rows.length" class="empty-tip">{{ t("cloud.empty") }}</p>
          <article v-for="row in rows" :key="row.name" class="cloud-world flex flex-col gap-1.5 rounded-card border border-border p-3 [overflow-wrap:anywhere]">
            <strong>{{ stripFormatting(row.local?.levelName || row.name) }}</strong>
            <span class="cloud-dir text-meta text-dim">{{ row.name }}</span>
            <p class="field-hint">{{ t(row.local ? "cloud.localPresent" : "cloud.localAbsent") }} ·
              {{ row.remote ? t("cloud.remoteTime", { time: formatTime(Date.parse(row.remote.time)) }) :
                t("cloud.remoteAbsent") }}
            </p>
            <div class="cloud-actions flex flex-wrap gap-2">
              <BaseButton size="sm" :disabled="busy || !!dirty || !row.local" @click="request(row.name, 'upload')">{{
                t("cloud.upload") }}</BaseButton>
              <BaseButton size="sm" :disabled="busy || !!dirty || !row.remote" @click="request(row.name, 'download')">{{
                t("cloud.download") }}</BaseButton>
              <BaseButton size="sm" :disabled="busy || !!dirty || !row.remote" @click="request(row.name, 'delete')">{{
                t("cloud.delete") }}</BaseButton>
            </div>
          </article>
        </div>
        <BaseButton size="sm" :disabled="busy" @click="backups">{{ t("cloud.backups") }}</BaseButton>
      </section>
    </div>
    <p v-if="busy" class="field-hint" role="status">{{ t("cloud.working") }}</p>
    <p v-if="error" class="cloud-error mt-3 text-ctl text-red" role="alert">{{ error }}</p>
  </BaseModal>
  <BaseModal v-if="open && pending" :title="t('cloud.confirmTitle')" :close-disabled="busy" @close="pending = null">
    <p>{{ t(`cloud.confirm.${pending.direction}`, { name: pending.name }) }}</p>
    <div class="cloud-actions cloud-confirm flex flex-wrap gap-2 mt-5 justify-end">
      <BaseButton :disabled="busy" @click="pending = null">{{ t("actions.cancel") }}</BaseButton>
      <BaseButton :disabled="busy" @click="transfer">{{ t(busy ? "cloud.working" : "cloud.continue") }}</BaseButton>
    </div>
    <p v-if="error" class="cloud-error mt-3 text-ctl text-red" role="alert">{{ error }}</p>
  </BaseModal>
</template>

<style scoped>
/* 小标题是后代选择器、窄窗是媒体查询，两类都留在 CSS；其余排版在模板上的工具类。 */
.cloud-layout h4 {
  margin: 0 0 8px;
  font-size: 13px;
}

@media (max-width: 620px) {
  .cloud-layout {
    grid-template-columns: 1fr;
    gap: 16px;
  }

  .cloud-connection {
    padding-right: 0;
    padding-bottom: 16px;
    border-right: 0;
    border-bottom: 1px solid var(--border);
  }
}
</style>
