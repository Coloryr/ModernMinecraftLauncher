<script setup lang="ts">
// 账户列表视图：头像 / 名字 / 类型 / UUID + 操作
import { ref } from "vue";
import { t } from "../../../lib/i18n";
import { accountAvatarUrl, hasNoSkin, imageFailed, imageLoading, markImageFailed, markImageLoaded } from "../../../lib/accountImages";
import AccountActions from "../../../components/AccountActions.vue";
import AccountTypeBadge from "../../../components/AccountTypeBadge.vue";
import type { AccountStoreDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要展示的账户，调用方已按锁定 / 筛选 / 搜索过滤好 */
  accounts: AccountStoreDto[];
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

// 悬停头像浮动显示头像大图（头部渲染），与平铺视图一致
const preview = ref<{ x: number; y: number; url: string } | null>(null);

/** 悬停时定位并展示头像大图；该图已加载失败就不展示 */
function showPreview(e: MouseEvent, acc: AccountStoreDto) {
  if (imageFailed(acc, "avatar")) return;
  preview.value = { x: e.clientX, y: e.clientY, url: acc.avatar || accountAvatarUrl(acc) };
}

function movePreview(e: MouseEvent) {
  if (!preview.value) return;
  // 右/下缘内翻，避免大图出窗：大图最大 256px 高 + 边距
  preview.value.x = Math.min(e.clientX + 14, window.innerWidth - 200);
  preview.value.y = Math.min(e.clientY + 14, window.innerHeight - 200);
}

function hidePreview() {
  preview.value = null;
}
</script>

<template>
  <div class="acc-list">
    <!-- 添加账户：作为列表的第一行，与账户行同一套布局 -->
    <button class="acc-row add-row" v-tip="t('account.addTitle')" :aria-label="t('account.addTitle')"
      @click="emit('add')">
      <span class="add-icon">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round">
          <path d="M12 5v14M5 12h14" />
        </svg>
      </span>
      <span class="row-name add-text">{{ t("account.add") }}</span>
    </button>

    <div v-for="acc in accounts" :key="acc.uuid" class="acc-row" :class="{ current: isCurrent(acc) }"
      @dblclick="emit('switch', acc)">
      <!-- 加载中：图上叠转圈，onload 后消失 -->
      <div v-if="!imageFailed(acc, 'avatar')" class="row-avatar-box">
        <img :src="acc.avatar || accountAvatarUrl(acc)" class="row-avatar"
          :class="{ pending: imageLoading(acc, 'avatar') }" alt="" data-no-fallback
          @load="markImageLoaded(acc, 'avatar')" @error="markImageFailed(acc, 'avatar')"
          @mouseenter="showPreview($event, acc)" @mousemove="movePreview" @mouseleave="hidePreview" />
        <div v-if="imageLoading(acc, 'avatar')" class="img-spin" />
      </div>
      <!-- 头像拉取失败回退字母头像（与主页面下拉一致），不显示"无头像"占位 -->
      <span v-else class="row-avatar row-letter" :style="{ background: acc.avatarColor }">
        {{ acc.userName.charAt(0).toUpperCase() }}
      </span>
      <div class="row-meta">
        <span class="row-name" v-tip="acc.userName">{{ acc.userName }}</span>
        <span class="row-sub">
          <AccountTypeBadge :auth-type="acc.authType" />
          <span class="row-uuid">{{ acc.uuid }}</span>
        </span>
      </div>
      <span v-if="isCurrent(acc)" class="current-tag">{{ t("account.current") }}</span>
      <span class="token-tag" :class="acc.tokenStatus">{{ tokenLabel(acc) }}</span>
      <AccountActions :can-refresh="acc.canRefresh" :can-relogin="acc.canRelogin" :can-edit="acc.canEdit"
        :has-skin="!hasNoSkin(acc)" @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)"
        @refresh-skin="emit('refreshSkin', acc)" @relogin="emit('relogin', acc)" @edit="emit('edit', acc)"
        @delete="emit('delete', acc)" />
    </div>
    <div v-if="accounts.length === 0" class="empty-tip">{{ t("account.searchEmpty") }}</div>

    <!-- 悬停头像时的头像大图 -->
    <Teleport to="body">
      <div v-if="preview" class="skin-float" :style="{ left: preview.x + 'px', top: preview.y + 'px' }">
        <img :src="preview.url" alt="" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.acc-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.acc-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  cursor: default;
}

.acc-row.current {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

/* 添加账户行：虚线边框，与账户行同布局（图标占头像槽的位置）。
   底色取 --bg-side（比账户行的 --bg-card 暗一档）：全透明显得不能点，
   --accent-soft 又太抢眼（那是选中态的浓度）。
   悬停时虚线转实线：虚线是"这里还空着"的语义，鼠标上来就该是实心的选中感 */
.add-row {
  border-style: dashed;
  border-color: var(--accent-border);
  background: var(--bg-side);
  color: var(--accent);
  cursor: pointer;
  font-family: inherit;
  font-size: 13px;
  text-align: left;
  transition: border-color 0.15s, color 0.15s, background 0.15s, box-shadow 0.15s;
}

.add-row:hover {
  border-style: solid;
  border-color: var(--accent);
  background: var(--accent-soft);
  box-shadow: 0 0 0 1px var(--accent);
}

.add-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
  border-radius: 50%;
  border: 1px dashed currentColor;
}

.add-text {
  font-weight: 600;
}

/* 头像槽容器：相对定位，加载中时上面叠转圈 */
.row-avatar-box {
  position: relative;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.row-avatar {
  width: 40px;
  height: 40px;
  image-rendering: pixelated;
}

.row-avatar.pending {
  opacity: 0;
}

/* 加载中转圈（img 上层居中） */
.img-spin {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

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

/* 头像拉取失败时的字母头像（取名字首字符，背景用账户配色） */
.row-letter {
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 700;
  font-size: 15px;
  user-select: none;
}

.row-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.row-name {
  font-size: 14px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-sub {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  font-family: "Cascadia Code", Consolas, monospace;
  white-space: nowrap;
  overflow: hidden;
}

.row-uuid {
  overflow: hidden;
  text-overflow: ellipsis;
}

.current-tag {
  font-size: 10.5px;
  padding: 2px 8px;
  border-radius: 20px;
  background: var(--accent);
  color: #fff;
  white-space: nowrap;
  flex-shrink: 0;
}

.token-tag {
  font-size: 11px;
  padding: 2px 9px;
  border-radius: 20px;
  white-space: nowrap;
  flex-shrink: 0;
}

.token-tag.valid {
  background: rgba(62, 207, 142, 0.14);
  color: var(--green);
}

.token-tag.expired {
  background: rgba(255, 95, 86, 0.14);
  color: var(--red);
}

/* 悬停头像时的头像大图：Teleport 到 body，跟随鼠标 */
.skin-float {
  position: fixed;
  z-index: 999;
  pointer-events: none;
  padding: 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-lg);
}

.skin-float img {
  display: block;
  width: 160px;
  height: 160px;
  image-rendering: pixelated;
}
</style>
