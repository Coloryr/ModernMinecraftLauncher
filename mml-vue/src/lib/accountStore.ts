// 账户共享存储：主窗口、账户选择器、账户窗口共用
// 数据由 Rust 提供，账户列表与当前选择分别由内核持久化，
// 操作通过 IPC；跨窗口通过 account-change 事件同步。
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { AccountStoreDto } from "./bindings";
import { AccountChange } from "./listens";
import { commands } from "./bindings";
import { tErr } from "./i18n";
import { showToast } from "./toast";

/** 全部账户，顺序由后端定 */
export const accounts = ref<AccountStoreDto[]>([]);

/** 账户操作是否被联机大厅锁定 */
export const accountLocked = ref(false);

/** 当前账户；null 表示未选择 */
export const currentAccount = ref<AccountStoreDto | null>(null);

/** 账户类型显示名；微软放第一个，离线放最后一个 */
export const ACCOUNT_TYPES: Array<{ value: string; labelKey: string }> = [
  { value: "microsoft", labelKey: "account.typeMicrosoft" },
  { value: "littleskin", labelKey: "account.typeLittleSkin" },
  { value: "selflittleskin", labelKey: "account.typeSelflittleskin" },
  { value: "authlib", labelKey: "account.typeAuthlib" },
  { value: "nide8", labelKey: "account.typeNide8" },
  { value: "offline", labelKey: "account.typeOffline" },
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
    accountLocked.value = view.locked;
    // currentUuid 为 null = 后端明确记录了"未选择"（登录方式锁定清除后），
    // 不能再回退到第一个账户，否则取消选择会被撤销
    currentAccount.value =
      view.accounts.find((a) => a.uuid === view.currentUuid) ?? null;
  } catch {
    /* 浏览器环境：保持为空 */
  }
}
/**
 * 设置当前账户，后端拒绝时保持原选择
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 后端确实切换了返回 true
 */
export async function setCurrentAccount(acc: AccountStoreDto): Promise<boolean> {
  return updateSelection(acc.uuid);
}

/**
 * 清除当前选择，联机大厅锁定期间由后端拒绝
 *
 * # 返回值
 *
 * 后端确实清掉了返回 true
 */
export async function clearCurrentAccount(): Promise<boolean> {
  return updateSelection("");
}

async function updateSelection(uuid: string): Promise<boolean> {
  try {
    const changed = await commands.account.setCurrentAccount(uuid);
    await loadAccounts();
    return changed;
  } catch (error) {
    showToast(tErr(error));
    return false;
  }
}

/**
 * 删除成功后重拉账户，保持选择与后端一致
 *
 * - `uuid`: 账户 uuid
 *
 * # 返回值
 *
 * 删除成功返回 true
 */
export async function removeAccount(uuid: string): Promise<boolean> {
  try {
    const removed = await commands.account.removeAccount(uuid);
    await loadAccounts();
    return removed;
  } catch (error) {
    showToast(tErr(error));
    return false;
  }
}

/**
 * 刷新账户 Token
 *
 * 失败静默忽略；成功由后端广播 account-change 重拉真实状态，含过期标记。
 *
 * - `uuid`: 目标账户 uuid
 */
export async function refreshAccountToken(uuid: string): Promise<void> {
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
  // account-change 广播可能比这条命令的返回值先到，那条路已经重拉过一次列表：
  // 直接 push 会多出一条，所以按 uuid 去重
  const index = accounts.value.findIndex((item) => item.uuid === acc.uuid);
  if (index >= 0) {
    accounts.value.splice(index, 1, acc);
  } else {
    accounts.value.push(acc);
  }
  if (!currentAccount.value) currentAccount.value = acc;
  return acc;
}

/**
 * 微软登录：触发后端设备码流程
 *
 * 弹窗交互与结果经 account-oauth / account-oauth-state 事件通知；失败时抛出后端错误，
 * 如网络不通，由调用方提示
 */
export async function loginMicrosoft(): Promise<void> {
  await commands.account.addAccount("", "microsoft", null, null);
}

// 跨窗口同步：某个窗口改了账户后，其它窗口重新加载；浏览器预览下 listen 会拒绝，忽略
listen(AccountChange, () => loadAccounts()).catch(() => {});
