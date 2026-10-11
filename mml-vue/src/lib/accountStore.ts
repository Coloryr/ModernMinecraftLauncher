// 账户共享存储：主窗口、账户选择器、账户窗口共用
// 数据由 Rust 提供，见 windows/account.rs，持久化到 accounts.json，
// 操作通过 IPC；跨窗口通过 account-change 事件同步。
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { AccountStoreDto } from "./bindings";
import { AccountChange } from "./listens";
import { commands } from "./bindings";

export const accounts = ref<AccountStoreDto[]>([]);
export const currentAccount = ref<AccountStoreDto | null>(null);

/** 账户类型显示名 */
export const ACCOUNT_TYPES: Array<{ value: string; labelKey: string }> = [
  { value: "offline", labelKey: "account.typeOffline" },
  { value: "microsoft", labelKey: "account.typeMicrosoft" },
  { value: "littleskin", labelKey: "account.typeLittleSkin" },
  { value: "selflittleskin", labelKey: "account.typeSelflittleskin" },
  { value: "authlib", labelKey: "account.typeAuthlib" },
  { value: "nide8", labelKey: "account.typeNide8" },
];

/**
 * 取账户类型对应的 i18n 文案键
 *
 * - `type`: 账户添加类型
 *
 * # 返回值
 *
 * 文案键；未知类型回落离线账户那一档
 */
export function typeLabelKey(type: string): string {
  return ACCOUNT_TYPES.find((t) => t.value === type)?.labelKey ?? "account.typeOffline";
}

/** 从 Rust 加载账户列表 */
export async function loadAccounts(): Promise<void> {
  try {
    const view = await commands.account.getAccounts();
    accounts.value = view.accounts;
    // currentUuid 为 null = 后端明确记录了"未选择"（登录方式锁定清除后），
    // 不能再回退到第一个账户，否则取消选择会被撤销
    currentAccount.value =
      view.accounts.find((a) => a.uuid === view.currentUuid) ?? null;
  } catch {
    /* 浏览器环境：保持为空 */
  }
}


/**
 * 设置当前使用账户
 *
 * 乐观更新：本地状态先改，后端失败也不回滚 —— 失败时后端不会再广播
 * `account-change`，界面与后端的不一致会持续到下一次重拉。
 *
 * - `acc`: 目标账户
 */
export async function setCurrentAccount(acc: AccountStoreDto) {
  try {
    await commands.account.setCurrentAccount(acc.uuid);
  } catch {
    /* 忽略 */
  }
  currentAccount.value = acc;
}

/** 取消当前选择，后端清空记录并广播 account-change */
export async function clearCurrentAccount() {
  try {
    await commands.account.setCurrentAccount("");
  } catch {
    /* 忽略 */
  }
  currentAccount.value = null;
}

/**
 * 删除账户
 *
 * - `uuid`: 目标账户 uuid
 */
export async function removeAccount(uuid: string) {
  try {
    await commands.account.removeAccount(uuid);
  } catch {
    /* 忽略 */
  }
  const idx = accounts.value.findIndex((a) => a.uuid === uuid);
  if (idx >= 0) accounts.value.splice(idx, 1);
  if (currentAccount.value?.uuid === uuid) {
    // 删掉的正是当前账户：回退到第一个剩余账户，与后端 account_remove_account 同一口径
    currentAccount.value = accounts.value[0] ?? null;
  }
}

/**
 * 刷新账户 Token
 *
 * 失败静默忽略；成功由后端广播 account-change 重拉真实状态，含过期标记。
 *
 * - `uuid`: 目标账户 uuid
 */
export async function refreshAccountToken(uuid: string) {
  try {
    await commands.account.refreshAccountToken(uuid);
  } catch {
    /* 忽略：状态以后端为准 */
  }
}

/**
 * 添加账户，真实由 Rust 创建
 *
 * - `type`: 账户添加类型
 * - `name`: 账户显示名
 * - `server`: 认证服务器地址或服务器 ID，缺省为不指定
 * - `password`: 密码，缺省为不传
 *
 * # 返回值
 *
 * 新创建的账户；失败抛出后端错误文案
 */
export async function addAccount(
  type: string,
  name: string,
  server?: string,
  password?: string,
): Promise<AccountStoreDto> {
  const acc = await commands.account.addAccount(name, type, server ?? null, password ?? null);
  accounts.value.push(acc);
  if (!currentAccount.value) currentAccount.value = acc;
  return acc;
}

/** 微软登录：触发后端设备码流程；弹窗交互与结果经 account-oauth / account-oauth-state 事件通知。
 *  失败时抛出后端错误，如网络不通，由调用方提示 */
export async function loginMicrosoft(): Promise<void> {
  await commands.account.addAccount("", "microsoft", null, null);
}

// 跨窗口同步：某个窗口改了账户后，其它窗口重新加载
listen(AccountChange, () => loadAccounts());
