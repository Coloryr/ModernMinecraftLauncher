<script setup lang="ts">
// 主窗口顶栏的账户切换器：当前账户按钮 + 下拉账户菜单（含「管理账户」入口）
import { computed, ref } from "vue";
import { t } from "../lib/i18n";
import { openWindow } from "../windows/windowManager";
import type { AccountStoreDto } from "../lib/bindings";
import { accountAvatarUrl, imageFailed, imageLoading, markImageFailed, markImageLoaded } from "../lib/accountImages";
import { accountLocked, typeLabelKey } from "../lib/accountStore";

const props = defineProps<{
  /** 当前账户；`null` 表示还没选账户 */
  account: AccountStoreDto | null;
  /** 可切换的账户列表 */
  accounts: AccountStoreDto[];
}>();

const emit = defineEmits<{
  /** 切换账户：回传新选中的账户 */
  (e: "update:account", account: AccountStoreDto): void;
}>();

const open = ref(false);

const typeText = computed(() =>
  props.account ? t(typeLabelKey(props.account.authType)) : "",
);

const typeClass = computed(() =>
  props.account?.authType === "microsoft"
    ? "type-ms"
    : props.account?.authType === "offline"
      ? "type-offline"
      : "type-external",
);

function toggle() {
  if (accountLocked.value) return;
  open.value = !open.value;
}

function pick(account: AccountStoreDto) {
  if (accountLocked.value) return;
  emit("update:account", account);
  open.value = false;
}
</script>

<template>
  <div class="account-wrap">
    <button class="account-btn" :disabled="accountLocked" v-tip="accountLocked ? t('err.lobbyAccountLocked') : ''" @click="toggle">
      <template v-if="account">
        <!-- 加载中：头像上叠转圈，onload 后消失 -->
        <span v-if="!imageFailed(account, 'avatar')" class="avatar-box">
          <img class="avatar avatar-img" :class="{ pending: imageLoading(account, 'avatar') }"
            :src="accountAvatarUrl(account)" alt="" data-no-fallback @load="markImageLoaded(account, 'avatar')"
            @error="markImageFailed(account, 'avatar')" />
          <span v-if="imageLoading(account, 'avatar')" class="img-spin" />
        </span>
        <span v-else class="avatar" :style="{ background: account.avatarColor }">
          {{ account.userName.charAt(0).toUpperCase() }}
        </span>
        <span class="account-meta">
          <span class="account-name">{{ account.userName }}</span>
          <span class="account-type" :class="typeClass">{{ typeText }}</span>
        </span>
      </template>
      <template v-else>
        <!-- 加号用 SVG 而非全角“＋”字符：中文字体把该字形画在 em 框偏上位置，
             盒子居中了笔画仍显偏上，SVG 由 flex 居中不受字体度量影响 -->
        <span class="avatar placeholder">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.2"
            stroke-linecap="round">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </span>
        <span class="account-meta">
          <span class="account-name dim">{{ t("account.noAccount") }}</span>
        </span>
      </template>
      <svg class="chevron" :class="{ flip: open }" viewBox="0 0 24 24" width="14" height="14" fill="none"
        stroke="currentColor" stroke-width="2">
        <path d="m6 9 6 6 6-6" />
      </svg>
    </button>

    <!-- 点外面收起菜单。不加 data-no-drag：遮罩全屏铺开，标了会把整个标题栏
         的拖拽也挡住（菜单开着时窗口拖不动）。用 pointerdown 收起——
         在标题栏上按下时拖拽与收菜单一并进行，互不干扰 -->
    <div v-if="open" class="menu-backdrop" @pointerdown="open = false"></div>

    <!-- data-no-drag：菜单在标题栏拖拽区里，footer 等非 button 区域
         的 click 会被 startDragging 吞掉，须整体豁免 -->
    <Transition name="drop">
      <div v-if="open" class="account-menu" data-no-drag>
        <div class="menu-title">{{ t("account.switch") }}</div>
        <div v-if="accounts.length === 0" class="empty-tip">{{ t("account.noAccount") }}</div>
        <button v-for="acc in accounts" :key="acc.uuid" class="menu-item"
          :disabled="accountLocked" :class="{ active: acc.uuid === account?.uuid }" @click="pick(acc)">
          <span v-if="!imageFailed(acc, 'avatar')" class="avatar-box">
            <img class="avatar small avatar-img" :class="{ pending: imageLoading(acc, 'avatar') }"
              :src="accountAvatarUrl(acc)" alt="" data-no-fallback @load="markImageLoaded(acc, 'avatar')"
              @error="markImageFailed(acc, 'avatar')" />
            <span v-if="imageLoading(acc, 'avatar')" class="img-spin" />
          </span>
          <span v-else class="avatar small" :style="{ background: acc.avatarColor }">
            {{ acc.userName.charAt(0).toUpperCase() }}
          </span>
          <span class="menu-meta">
            <span class="menu-name" v-tip="acc.userName">{{ acc.userName }}</span>
            <span class="menu-type">{{ t(typeLabelKey(acc.authType)) }}</span>
          </span>
        </button>
        <div class="menu-footer" @click="open = false; openWindow('account')">{{ t("account.manage") }}</div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.account-wrap {
  position: relative;
}

/* 点击菜单外区域收起 */
.menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 190;
}

.account-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 12px 6px 8px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text);
  cursor: pointer;
  transition: all 0.15s;
}

.account-btn:hover {
  background: var(--bg-hover);
  border-color: var(--accent);
}

.avatar {
  width: 34px;
  height: 34px;
  border-radius: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 700;
  font-size: 15px;
  flex-shrink: 0;
}

/* 阴影只给字母 / 占位头像：3D 头像 PNG 带透明区，img 上投影会成方形托板 */
.avatar:not(.avatar-img) {
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.35);
}

.avatar.small {
  width: 30px;
  height: 30px;
  font-size: 13px;
}

.avatar.placeholder {
  background: var(--bg-hover);
  border: 1px dashed var(--border);
  color: var(--text-dim);
  box-shadow: none;
}

/* 未选择账户的文字弱化 */
.account-name.dim {
  color: var(--text-dim);
  font-weight: 500;
}

.account-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  line-height: 1.25;
  text-align: left;
}

.account-name {
  font-size: 13.5px;
  font-weight: 600;
}

.account-type {
  font-size: 11px;
  padding: 1px 7px;
  border-radius: 20px;
}

.type-ms {
  color: #8fd0ff;
  background: rgba(63, 140, 255, 0.16);
}

.type-offline {
  color: var(--text-dim);
  background: rgba(154, 163, 175, 0.14);
}

.type-external {
  color: #22d3ee;
  background: rgba(6, 182, 212, 0.16);
}

/* 亮色主题：浅色文字在白底上看不清，换深色系 */
[data-theme="Light"] .type-ms {
  color: #2563eb;
}

[data-theme="Light"] .type-external {
  color: #0e7490;
}

/* 真实头像图（加载失败回退字母头像）。
 * 阴影用 drop-shadow：跟随头像轮廓投影，不会像 box-shadow 那样透出方形底板 */
.avatar-img {
  object-fit: cover;
  image-rendering: pixelated;
}

/* 头像槽容器：相对定位，加载中时上面叠转圈 */
.avatar-box {
  position: relative;
  flex-shrink: 0;
  display: inline-flex;
}

.avatar-img.pending {
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
  width: 14px;
  height: 14px;
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

.chevron {
  color: var(--text-dim);
  transition: transform 0.15s;
}

.chevron.flip {
  transform: rotate(180deg);
}

.account-menu {
  position: absolute;
  right: 0;
  top: calc(100% + 8px);
  width: 240px;
  max-height: calc(100vh - var(--titlebar-h) - 16px);
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.22);
  padding: 8px;
  z-index: 200;
  /* 菜单贴右缘，展开时以右上角为原点，避免向左"甩" */
  transform-origin: top right;
}

.menu-title {
  font-size: 12px;
  color: var(--text-dim);
  padding: 6px 8px;
}

/* 菜单空状态（还没有任何账户） */
.empty-tip {
  font-size: 12px;
  color: var(--text-dim);
  text-align: center;
  padding: 10px 8px;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px;
  border: none;
  border-radius: 9px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  text-align: left;
}

.menu-item:hover {
  background: var(--bg-hover);
}

.menu-item.active {
  background: rgba(79, 140, 255, 0.14);
  outline: 1px solid rgba(79, 140, 255, 0.4);
}

.menu-meta {
  display: flex;
  flex-direction: column;
  line-height: 1.25;
  min-width: 0;
}

.menu-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.menu-type {
  font-size: 11px;
  color: var(--text-dim);
}

.menu-footer {
  font-size: 12px;
  color: var(--text-dim);
  text-align: center;
  padding: 8px;
  border-top: 1px solid var(--border);
  margin-top: 4px;
  cursor: pointer;
}

.menu-footer:hover {
  color: var(--accent);
}
</style>
