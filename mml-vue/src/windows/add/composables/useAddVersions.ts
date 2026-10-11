// 添加实例 · 「从头新建」模式的数据源
//
// 抽自 AddInstanceWindow.vue，只负责三件事：
// 1. 首次加载 + 核心尚未就绪时的轮询重试（关窗立即停止，不再空转）；
// 2. 显式刷新（成功才提示；联动查询不提示）；
// 3. 版本类型多选过滤：空数组不过滤，且只过滤不重排，保持后端给的分组顺序。
import { computed, ref } from "vue";
import { api } from "../../../lib/api";
import type { VersionInfoDto } from "../../../lib/bindings";
import { showToast } from "../../../lib/toast";
import { t } from "../../../lib/i18n";

/** 版本列表轮询间隔与次数，合计约 60s：核心加载慢时等它把版本清单写完 */
const POLL_INTERVAL_MS = 2000;
const POLL_TIMES = 30;

/**
 * 「从头新建」模式的数据源：版本列表、版本类型与压缩包类型
 *
 * - `isLeaving`: 询问是否正在关窗；关窗后在途结果一律丢弃，轮询也停下
 *
 * # 返回值
 *
 * - `versions`: 全部版本，后端已按类型分组、组内新旧排序
 * - `versionTypes` / `packTypes`: 版本类型与压缩包类型的 ID 列表
 * - `verTypes`: 已选版本类型，空数组表示不过滤
 * - `verLoading`: 版本列表刷新中
 * - `filteredVersions`: 按 `verTypes` 过滤后的版本列表
 * - `fetchOptionLists` / `loadVersions` / `startPolling` / `refreshVersions`: 拉取与刷新方法
 */
export function useAddVersions(isLeaving: () => boolean) {
  /** 全部版本 */
  const versions = ref<VersionInfoDto[]>([]);
  /** 版本类型 ID 列表（mml-core 独立 ID，显示名走 i18n） */
  const versionTypes = ref<string[]>([]);
  /** 压缩包类型 ID 列表（「导入压缩包」模式用） */
  const packTypes = ref<string[]>([]);
  /** 已选版本类型（多选，空 = 不过滤） */
  const verTypes = ref<string[]>(["release"]);
  /** 版本列表刷新中（刷新按钮转圈并禁用） */
  const verLoading = ref(false);

  /** 按版本类型过滤；不重排，重排会打乱后端给的分组顺序 */
  const filteredVersions = computed(() => {
    const sel = verTypes.value;
    return versions.value.filter((v) => sel.length === 0 || sel.includes(v.versionType));
  });

  /** 拉取下拉数据源：版本类型 / 压缩包类型（独立 ID，显示名走 i18n） */
  async function fetchOptionLists() {
    try {
      const [types, packs] = await Promise.all([api.addGetVersionTypes(), api.addGetPackTypes()]);
      if (isLeaving()) return;
      versionTypes.value = types;
      packTypes.value = packs;
    } catch {
      // 核心未加载完时可能失败，留空，需要时可重开窗口
    }
  }

  /** 首次加载版本列表（失败留空，由 startPolling 继续重试） */
  async function loadVersions() {
    try {
      const list = await api.getVersions();
      if (isLeaving()) return;
      versions.value = list;
    } catch {
      versions.value = [];
    }
  }

  /** 核心尚未就绪时轮询重试；关窗（或已拿到列表）立即停止 */
  async function startPolling() {
    for (let i = 0; i < POLL_TIMES; i++) {
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
      if (isLeaving() || versions.value.length) return;
      try {
        const list = await api.getVersions();
        if (isLeaving()) return;
        if (list.length) {
          versions.value = list;
          return;
        }
      } catch {
        // 忽略，继续重试
      }
    }
  }

  /** 强制刷新版本列表（清后端缓存重新拉取）；不改动已选版本，成功才提示 */
  async function refreshVersions() {
    if (verLoading.value) return;
    verLoading.value = true;
    try {
      const list = await api.refreshVersions();
      if (isLeaving()) return;
      versions.value = list;
      showToast(t("tip.refreshed"));
    } catch {
      // 保留旧列表
    } finally {
      verLoading.value = false;
    }
  }

  return {
    versions,
    versionTypes,
    packTypes,
    verTypes,
    verLoading,
    filteredVersions,
    fetchOptionLists,
    loadVersions,
    startPolling,
    refreshVersions,
  };
}
