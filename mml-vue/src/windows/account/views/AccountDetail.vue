<script setup lang="ts">
// 账户详情视图：全账户表格，操作按钮在左侧
import { t } from "../../../lib/i18n";
import {
  accountSkinUrl,
  hasNoSkin,
  markImageFailed,
  markImageLoaded,
} from "../../../lib/accountImages";
import AccountActions from "../../../components/AccountActions.vue";
import AccountTypeBadge from "../../../components/AccountTypeBadge.vue";
import type { AccountStoreDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要展示的账户，调用方已按锁定 / 筛选 / 搜索过滤好 */
  accounts: AccountStoreDto[];
  /** 列表是不是被筛过：为真时不列「添加」入口，空结果给文案 */
  filtering: boolean;
  /** 当前使用的账户 uuid，空串表示没有 */
  currentUuid: string;
  /** 把账户的令牌状态翻成文案 */
  tokenLabel: (acc: AccountStoreDto) => string;
}>();

const emit = defineEmits<{
  /** 打开「添加账户」表单 */
  (e: "add"): void;
  /** 双击账户：切换为当前使用 */
  (e: "switch", acc: AccountStoreDto): void;
  /** 刷新该账户的令牌 */
  (e: "refresh", acc: AccountStoreDto): void;
  /** 预览该账户的皮肤 / 披风 */
  (e: "viewSkin", acc: AccountStoreDto): void;
  /** 重取该账户的皮肤渲染图 */
  (e: "refreshSkin", acc: AccountStoreDto): void;
  /** 重新登录该账户 */
  (e: "relogin", acc: AccountStoreDto): void;
  /** 编辑该账户 */
  (e: "edit", acc: AccountStoreDto): void;
  /** 删除该账户 */
  (e: "delete", acc: AccountStoreDto): void;
}>();

/** 是否当前使用的账户 */
function isCurrent(acc: AccountStoreDto): boolean {
  return acc.uuid === props.currentUuid;
}
</script>

<template>
  <div class="acc-detail">
    <div class="table-wrap">
      <table class="detail-table wide">
        <thead>
          <tr>
            <th class="col-actions">{{ t("account.actions") }}</th>
            <th>{{ t("account.name") }}</th>
            <th>{{ t("account.uuid") }}</th>
            <th>{{ t("account.type") }}</th>
            <th>{{ t("account.lastLogin") }}</th>
            <th>{{ t("account.tokenStatus") }}</th>
            <th>{{ t("account.ext1") }}</th>
            <th>{{ t("account.ext2") }}</th>
          </tr>
        </thead>
        <tbody>
          <!-- 添加账户：作为表格的第一行 -->
          <tr v-if="!filtering" class="add-tr" @click="emit('add')">
            <td class="col-actions">
              <button class="add-btn" v-tip="t('account.addTitle')" :aria-label="t('account.addTitle')">
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"
                  stroke-linecap="round">
                  <path d="M12 5v14M5 12h14" />
                </svg>
              </button>
            </td>
            <td class="cell-name add-text" colspan="7">{{ t("account.add") }}</td>
          </tr>
          <tr v-for="acc in accounts" :key="acc.uuid" :class="{ current: isCurrent(acc) }"
            @dblclick="emit('switch', acc)">
            <td class="col-actions">
              <!-- 不可见的皮肤探测图：表格不展示皮肤，但「查看皮肤 / 刷新皮肤」要知道有没有皮肤。
                 没有它就永远不会产生 skin 的失败标记，那俩按钮在没皮肤的账户上照样显示。
                 1px + opacity 0：不占位、不可见，只为了让 @error / @load 能触发 -->
              <img class="skin-probe" :src="accountSkinUrl(acc)" alt="" aria-hidden="true" data-no-fallback
                @error="markImageFailed(acc, 'skin')" @load="markImageLoaded(acc, 'skin')" />
              <AccountActions :can-refresh="acc.canRefresh" :can-relogin="acc.canRelogin" :can-edit="acc.canEdit"
                :has-skin="!hasNoSkin(acc)" @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)"
                @refresh-skin="emit('refreshSkin', acc)" @relogin="emit('relogin', acc)" @edit="emit('edit', acc)"
                @delete="emit('delete', acc)" />
            </td>
            <td class="cell-name" v-tip="acc.userName">
              {{ acc.userName }}
              <span v-if="isCurrent(acc)" class="current-tag">{{ t("account.current") }}</span>
            </td>
            <td class="mono">{{ acc.uuid }}</td>
            <td>
              <AccountTypeBadge :auth-type="acc.authType" />
            </td>
            <td>{{ acc.loginTime }}</td>
            <td>
              <span class="token-tag" :class="acc.tokenStatus">{{ tokenLabel(acc) }}</span>
            </td>
            <td class="cell-ext" v-tip="acc.ext1 ?? ''">{{ acc.ext1 ?? "—" }}</td>
            <td class="cell-ext" v-tip="acc.ext2 ?? ''">{{ acc.ext2 ?? "—" }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-if="accounts.length === 0 && filtering" class="empty-tip">{{ t("account.searchEmpty") }}</div>
  </div>
</template>

<style scoped>
.acc-detail {
  display: flex;
  flex-direction: column;
  gap: 14px;
  /* 占满视图区剩余高度：工具栏留在外面，上下滚动发生在表格容器里 */
  flex: 1;
  min-height: 0;
}

/* 表格在容器内滚动：上下滚动看行，左右滚动看列；列都是 nowrap，长内容不压缩 */
.table-wrap {
  overflow: auto;
  /* 只预留**纵向**那条：scrollbar-gutter 管不到横向；表格列多是 nowrap，
     横向滚动条基本常驻，纵向那条随账户数量出现 / 消失，预留它免得行整体左移 */
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  flex: 1;
  min-height: 0;
}

.detail-table.wide {
  border-collapse: collapse;
  width: 100%;
}

.detail-table th,
.detail-table td {
  padding: 10px 14px;
  font-size: 12.5px;
  border-bottom: 1px solid var(--border);
  text-align: left;
  white-space: nowrap;
}

.detail-table thead th {
  color: var(--text-dim);
  font-weight: 600;
  background: var(--bg-side);
}

.detail-table tbody tr:last-child th,
.detail-table tbody tr:last-child td {
  border-bottom: none;
}

.detail-table tbody tr.current td {
  background: var(--accent-soft);
}

/* 添加账户行：虚线下边框，整行可点，观感与列表/网格视图的添加项一致 */
.add-tr {
  cursor: pointer;
  color: var(--text-dim);
}

.add-tr:hover {
  background: var(--accent-soft);
  color: var(--accent);
}

.add-tr td {
  border-bottom-style: dashed !important;
}

.add-tr .add-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ctl-h);
  height: var(--ctl-h);
  padding: 0;
  border: 1px dashed currentColor;
  border-radius: 8px;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.add-text {
  font-weight: 600;
}

.col-actions {
  width: 108px;
}

/* 皮肤探测图：只用于触发加载 / 失败事件，本身不可见也不占位 */
.skin-probe {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

.cell-name {
  font-weight: 700;
  /* 名字过长省略显示，不把表格撑出横向滚动条；悬停 title 看全名 */
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 自定义字段同样省略，悬停看全文；微软 ext1 是 refresh_token，很长 */
.cell-ext {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
}

.current-tag {
  font-size: 10.5px;
  padding: 2px 8px;
  border-radius: 20px;
  background: var(--accent);
  color: #fff;
  white-space: nowrap;
  flex-shrink: 0;
  margin-left: 6px;
}

.token-tag {
  font-size: 11px;
  padding: 2px 9px;
  border-radius: 20px;
  white-space: nowrap;
  flex-shrink: 0;
}

.token-tag.valid {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.token-tag.expired {
  background: color-mix(in srgb, var(--red) 14%, transparent);
  color: var(--red);
}

.mono {
  font-family: "Cascadia Code", Consolas, monospace;
}
</style>
