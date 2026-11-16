<script setup lang="ts">
// 账户详情视图：全账户表格，操作按钮在左侧
import { accountLocked } from "../../../lib/accountStore";
import { t } from "../../../lib/i18n";
import {
  accountSkinUrl,
  hasNoSkin,
  markImageFailed,
  markImageLoaded,
} from "../../../lib/accountImages";
import AccountActions from "../../../components/AccountActions.vue";
import AccountTypeBadge from "../../../components/AccountTypeBadge.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import AccountEmpty from "../AccountEmpty.vue";
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
  /** 搜索词，用来给命中的名字 / uuid 加底色 */
  query: string;
}>();

const emit = defineEmits<{
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
  /** 右键账户：打开操作菜单 */
  (e: "menu", payload: { event: MouseEvent; acc: AccountStoreDto }): void;
  /** 空状态里点「添加账户」 */
  (e: "add"): void;
}>();

/** 是否当前使用的账户 */
function isCurrent(acc: AccountStoreDto): boolean {
  return acc.uuid === props.currentUuid;
}
</script>

<template>
  <div class="acc-detail flex min-h-0 flex-1 flex-col gap-3.5">
    <div v-if="accounts.length > 0" class="table-card flex min-h-0 flex-1 flex-col overflow-hidden rounded-panel border border-border bg-card">
      <div class="table-wrap">
        <table class="detail-table wide w-full border-collapse">
          <thead>
            <tr>
              <th class="col-actions w-[108px]">{{ t("account.actions") }}</th>
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
            <tr v-for="acc in accounts" :key="acc.uuid" :class="{ current: isCurrent(acc) }"
              @dblclick="!accountLocked && emit('switch', acc)"
              @contextmenu.prevent="emit('menu', { event: $event, acc })">
              <td class="col-actions w-[108px]">
                <!-- 不可见的皮肤探测图：表格不展示皮肤，但「查看皮肤 / 刷新皮肤」要知道有没有皮肤。
                   没有它就永远不会产生 skin 的失败标记，那俩按钮在没皮肤的账户上照样显示。
                   1px + opacity 0：不占位、不可见，只为了让 @error / @load 能触发 -->
                <img class="skin-probe pointer-events-none absolute h-px w-px opacity-0" :src="accountSkinUrl(acc)" alt="" aria-hidden="true" data-no-fallback
                  @error="markImageFailed(acc, 'skin')" @load="markImageLoaded(acc, 'skin')" />
                <AccountActions :locked="accountLocked && acc.uuid === currentUuid" :can-refresh="acc.canRefresh"
                  :can-relogin="acc.canRelogin" :can-edit="acc.canEdit" :has-skin="!hasNoSkin(acc)"
                  @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)"
                  @refresh-skin="emit('refreshSkin', acc)" @relogin="emit('relogin', acc)" @edit="emit('edit', acc)"
                  @delete="emit('delete', acc)" />
              </td>
              <td class="cell-name max-w-[200px] overflow-hidden text-ellipsis font-bold">
                <HighlightText :text="acc.userName" :query="query" />
                <span v-if="isCurrent(acc)" class="current-tag ml-1.5 shrink-0 whitespace-nowrap rounded-pill bg-accent px-2 py-0.5 text-caption text-on-accent">{{ t("account.current") }}</span>
              </td>
              <td class="mono [font-family:'Cascadia Code',Consolas,monospace]">
                <HighlightText :text="acc.uuid" :query="query" />
              </td>
              <td>
                <AccountTypeBadge :auth-type="acc.authType" />
              </td>
              <td>{{ acc.loginTime }}</td>
              <td>
                <span class="token-tag shrink-0 whitespace-nowrap rounded-pill px-[9px] py-0.5" :class="acc.tokenStatus">{{ tokenLabel(acc) }}</span>
              </td>
              <td class="cell-ext max-w-[180px] overflow-hidden text-ellipsis text-meta [font-family:'Cascadia Code',Consolas,monospace]" v-tip="acc.ext1 ?? ''">{{ acc.ext1 ?? "—" }}</td>
              <td class="cell-ext max-w-[180px] overflow-hidden text-ellipsis text-meta [font-family:'Cascadia Code',Consolas,monospace]" v-tip="acc.ext2 ?? ''">{{ acc.ext2 ?? "—" }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <AccountEmpty v-else class="detail-empty" :title="filtering ? t('account.searchEmpty') : t('account.emptyTitle')"
      :hint="filtering ? undefined : t('account.emptyHint')" :show-add="!filtering" @add="emit('add')" />
  </div>
</template>

<style scoped>
/* 表格元素选择器留在 CSS：th / td 数量多，逐个挂工具类反而更难读 */
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

.detail-table tbody tr:last-child td {
  border-bottom: none;
}

/* 当前使用的账户：整行铺强调色（状态类，主题可覆盖） */
.detail-table tbody tr.current td {
  background: var(--accent-soft);
}

/* 表格滚动容器：横向不预留、纵向预留一条滚动条槽，避免行随账户数量左右跳 */
.table-wrap {
  overflow: auto;
  overflow-y: scroll;
  scrollbar-gutter: stable;
  flex: 1;
  min-height: 0;
}

/* 表格单元格的字号由上面那条统一给（12.5），这两处要更小，必须留在无层里，
   否则会被表格规则盖掉 */
.cell-ext {
  font-size: 11.5px;
}

/* 11px 不在六档字号里，保留原值 */
.token-tag {
  font-size: 11px;
}

/* 令牌状态色：语义态，主题可覆盖 */
.token-tag.valid {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.token-tag.expired {
  background: color-mix(in srgb, var(--red) 14%, transparent);
  color: var(--red);
}
</style>
