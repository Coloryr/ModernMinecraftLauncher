<script setup lang="ts">
// 账户操作：查看皮肤 / 刷新皮肤 / 刷新 Token / 重新登录 / 编辑 / 删除（单色 SVG 图标按钮）
// 按钮显隐由后端给的能力标志决定 —— 哪些账户类型有什么操作是业务规则，不由前端硬编码
import { t } from "../lib/i18n";

defineProps<{
  /** 是否有可刷新的令牌，对应 AccountStoreDto.canRefresh */
  canRefresh?: boolean;
  /** 是否支持重新登录，对应 AccountStoreDto.canRelogin */
  canRelogin?: boolean;
  /** 是否可编辑，对应 AccountStoreDto.canEdit */
  canEdit?: boolean;
  /** 该账户是否有皮肤：false 时隐藏「查看皮肤 / 刷新皮肤」两个按钮；
   *  没皮肤时两个按钮点了也没内容，留着只是噪音 */
  hasSkin?: boolean;
}>();

defineEmits<{
  /** 刷新账户令牌 */
  (e: "refresh"): void;
  /** 打开皮肤查看弹窗 */
  (e: "viewSkin"): void;
  /** 重新拉取皮肤渲染图 */
  (e: "refreshSkin"): void;
  /** 重新登录该账户 */
  (e: "relogin"): void;
  /** 编辑离线账户信息 */
  (e: "edit"): void;
  /** 删除该账户 */
  (e: "delete"): void;
}>();
</script>

<template>
  <div class="acc-actions">
    <button v-if="canEdit" class="icon-btn" v-tip="t('account.editOffline')" @click="$emit('edit')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M17 3a2.8 2.8 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z" />
      </svg>
    </button>
    <!-- 查看皮肤：眼睛图标，"看一眼"的通用语义，与"刷新"的循环箭头不会混。
         没有皮肤时整个按钮不出现 -->
    <button v-if="hasSkin" class="icon-btn" v-tip="t('account.viewSkin')" @click="$emit('viewSkin')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" />
        <circle cx="12" cy="12" r="2.6" />
      </svg>
    </button>
    <!-- 刷新皮肤：循环箭头 + 中间一个小人，与「刷新 Token」的纯循环箭头区分开 -->
    <button v-if="hasSkin" class="icon-btn" v-tip="t('account.refreshSkin')" @click="$emit('refreshSkin')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M20.5 12a8.5 8.5 0 1 1-2.5-6M20.5 4v5h-5" />
        <circle cx="12" cy="11" r="1.9" />
        <path d="M8.6 16.4c.5-1.7 1.9-2.6 3.4-2.6s2.9.9 3.4 2.6" />
      </svg>
    </button>
    <button v-if="canRefresh" class="icon-btn" v-tip="t('account.refresh')" @click="$emit('refresh')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M20.5 12a8.5 8.5 0 1 1-2.5-6M20.5 4v5h-5" />
      </svg>
    </button>
    <button v-if="canRelogin" class="icon-btn" v-tip="t('account.relogin')" @click="$emit('relogin')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" />
        <path d="M10 17l5-5-5-5M15 12H3" />
      </svg>
    </button>
    <button class="icon-btn danger" v-tip="t('account.delete')" @click="$emit('delete')">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"
        stroke-linecap="round" stroke-linejoin="round">
        <path
          d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.acc-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.icon-btn {
  width: var(--ctl-h);
  height: var(--ctl-h);
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-dim);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  flex-shrink: 0;
}

.icon-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-soft);
}

.icon-btn.danger:hover {
  border-color: var(--red);
  color: var(--red);
  background: rgba(255, 95, 86, 0.1);
}
</style>
