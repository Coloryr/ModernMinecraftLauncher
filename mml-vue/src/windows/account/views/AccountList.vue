<script setup lang="ts">
// 账户列表视图：头像 / 名字 / 类型 / UUID + 操作
import { ref } from "vue";
import { accountLocked } from "../../../lib/accountStore";
import { t } from "../../../lib/i18n";
import { accountAvatarUrl, hasNoSkin, imageFailed, imageLoading, markImageFailed, markImageLoaded } from "../../../lib/accountImages";
import AccountActions from "../../../components/AccountActions.vue";
import AccountTypeBadge from "../../../components/AccountTypeBadge.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import AccountEmpty from "../AccountEmpty.vue";
import type { AccountStoreDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要展示的账户，调用方已按锁定 / 筛选 / 搜索过滤好 */
  accounts: AccountStoreDto[];
  /** 列表是不是被筛过：空结果给对应文案 */
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

// 悬停头像浮动显示头像大图（头部渲染），与平铺视图一致
const preview = ref<{ x: number; y: number; url: string } | null>(null);

/** 悬停时定位并展示头像大图；该图已加载失败就不展示 */
function showPreview(e: MouseEvent, acc: AccountStoreDto) {
  if (imageFailed(acc, "avatar")) return;
  preview.value = { x: e.clientX, y: e.clientY, url: accountAvatarUrl(acc) };
}

function movePreview(e: MouseEvent) {
  if (!preview.value) return;
  // 右/下缘内翻，避免大图出窗：大图 160px + 边距
  preview.value.x = Math.min(e.clientX + 14, window.innerWidth - 200);
  preview.value.y = Math.min(e.clientY + 14, window.innerHeight - 200);
}

function hidePreview() {
  preview.value = null;
}
</script>

<template>
  <div class="acc-list flex flex-col gap-2" :class="{ empty: accounts.length === 0 }">
    <div v-for="acc in accounts" :key="acc.uuid" class="acc-row flex cursor-default items-center gap-3 rounded-panel border border-border bg-card px-3.5 py-2.5" :class="{ current: isCurrent(acc) }"
      @dblclick="!accountLocked && emit('switch', acc)" @contextmenu.prevent="emit('menu', { event: $event, acc })">
      <!-- 加载中：图上叠转圈，onload 后消失 -->
      <div v-if="!imageFailed(acc, 'avatar')" class="row-avatar-box relative h-10 w-10 shrink-0">
        <img :src="accountAvatarUrl(acc)" class="row-avatar h-10 w-10 [image-rendering:pixelated]" :class="{ pending: imageLoading(acc, 'avatar') }" alt=""
          data-no-fallback @load="markImageLoaded(acc, 'avatar')" @error="markImageFailed(acc, 'avatar')"
          @mouseenter="showPreview($event, acc)" @mousemove="movePreview" @mouseleave="hidePreview" />
        <div v-if="imageLoading(acc, 'avatar')" class="img-spin absolute inset-0 flex items-center justify-center" />
      </div>
      <!-- 头像拉取失败回退字母头像（与主页面下拉一致），不显示"无头像"占位 -->
      <span v-else class="row-avatar row-letter flex h-10 w-10 shrink-0 items-center justify-center text-heading font-bold text-on-accent select-none" :style="{ background: acc.avatarColor }">
        {{ acc.userName.charAt(0).toUpperCase() }}
      </span>
      <div class="row-meta flex min-w-0 flex-1 flex-col gap-[3px]">
        <span class="row-name overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-bold text-fg">
          <HighlightText :text="acc.userName" :query="query" />
        </span>
        <span class="row-sub flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap text-meta text-dim [font-family:Cascadia_Code,Consolas,monospace]">
          <AccountTypeBadge :auth-type="acc.authType" />
          <span class="row-uuid overflow-hidden text-ellipsis">
            <HighlightText :text="acc.uuid" :query="query" />
          </span>
        </span>
      </div>
      <span v-if="isCurrent(acc)" class="current-tag shrink-0 whitespace-nowrap rounded-pill bg-accent px-2 py-0.5 text-caption text-on-accent">{{ t("account.current") }}</span>
      <span class="token-tag shrink-0 whitespace-nowrap rounded-pill px-[9px] py-0.5 text-[11px]" :class="acc.tokenStatus">{{ tokenLabel(acc) }}</span>
      <AccountActions :locked="accountLocked && acc.uuid === currentUuid" :can-refresh="acc.canRefresh"
        :can-relogin="acc.canRelogin" :can-edit="acc.canEdit" :has-skin="!hasNoSkin(acc)"
        @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)" @refresh-skin="emit('refreshSkin', acc)"
        @relogin="emit('relogin', acc)" @edit="emit('edit', acc)" @delete="emit('delete', acc)" />
    </div>
    <AccountEmpty v-if="accounts.length === 0" :title="filtering ? t('account.searchEmpty') : t('account.emptyTitle')"
      :hint="filtering ? undefined : t('account.emptyHint')" :show-add="!filtering" @add="emit('add')" />

    <!-- 悬停头像时的头像大图 -->
    <Teleport to="body">
      <div v-if="preview" class="skin-float pointer-events-none fixed z-[300] rounded-card border border-border bg-card p-2 shadow-[var(--shadow-lg)]" :style="{ left: preview.x + 'px', top: preview.y + 'px' }">
        <img class="block h-[160px] w-[160px] [image-rendering:pixelated]" :src="preview.url" alt="" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
/* 空状态占满视图区剩余高度（.empty 由调用方按数据状态挂） */
.acc-list.empty {
  flex: 1;
  min-height: 0;
}

/* 当前使用的账户：描边 + 一圈光晕（状态类，主题可覆盖） */
.acc-row.current {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

/* 头像加载中先压透明，onload 后出现 */
.row-avatar.pending {
  opacity: 0;
}

/* 加载中转圈：定位与居中在工具类上，这里只剩伪元素与关键帧 */
.img-spin::after {
  content: "";
  width: 16px;
  height: 16px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: img-spin 0.8s linear infinite;
}

@keyframes img-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 令牌状态色：语义态 */
.token-tag.valid {
  background: color-mix(in srgb, var(--green) 14%, transparent);
  color: var(--green);
}

.token-tag.expired {
  background: color-mix(in srgb, var(--red) 14%, transparent);
  color: var(--red);
}
</style>
