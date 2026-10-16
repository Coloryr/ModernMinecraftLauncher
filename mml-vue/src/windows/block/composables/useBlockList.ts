// 方块列表窗口的数据与状态
//
// 抽自 BlockPanel.vue，负责：
// 1. 渲染状态查询 / 事件订阅（含下载任务补开下载窗口）/ 列表加载；
// 2. 三态派生与进度：未渲染 / 渲染中 / 已渲染；
// 3. 分类聚合、搜索过滤、详情选中与前后切换；
// 4. 渲染与玩家头颅操作（带提示）；
// 5. 视图偏好记忆：分类 / 图标尺寸档存本地存储，**搜索词离开本页即清空**。
import { computed, onDeactivated, onMounted, ref, watch } from "vue";
import { locale, t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { useUnlisteners } from "../../../composables/useUnlisteners";
import { KEYS, readJson, writeJson } from "../../../lib/storage";
import {
  api,
  blockRenderCancel,
  blockRenderStart,
  blockSetIcon,
  blockSkinAdd,
  blockSkinRemove,
  getBlockList,
  getBlockStatus,
  onBlockRender,
  onDownloadTask,
} from "../../../lib/api";
import { openWindow } from "../../windowManager";
import type { BlockItemDto, BlockStatusDto } from "../../../lib/bindings";
import { GROUP_PREFIX, isBlockSize, type BlockSize } from "../types";

interface ViewPref {
  cat: string;
  keyword: string;
  size: BlockSize;
}

/** 外部分组 / 收藏数据的接入点；不传就是"没有自建分组、没有收藏" */
export interface BlockListHooks {
  /**
   * 自建分组成员判定
   *
   * - `uuid`: 分组标识（分类键去掉 `g:` 前缀）
   * - `id`: 方块 id
   */
  inGroup?: (uuid: string, id: string) => boolean;
  /** 收藏判定：收藏过的方块在列表里排到最前 */
  isFavorite?: (id: string) => boolean;
}

function readPref(): Partial<ViewPref> {
  return readJson<ViewPref>(KEYS.blockView);
}

/**
 * 方块列表窗口的数据与状态
 *
 * # 返回值
 *
 * - `status` / `blocks` / `rendered` / `running` / `preparing` / `percent` / `cancelling`: 渲染状态与列表
 * - `startRender` / `cancelRender` / `loadBlocks`: 渲染操作与列表重拉
 * - `keyword` / `cat` / `size` / `cats` / `catCounts` / `catLabel` / `isFiltered` / `filtered` / `clearFilters`: 搜索、分类与尺寸档
 * - `detail` / `openDetail` / `closeDetail`: 详情选中，只存 id，列表刷新后按 id 现查
 * - `addSkin` / `removeSkin` / `setIcon`: 玩家头颅与设为实例图标，均返回是否成功
 */
export function useBlockList(hooks?: BlockListHooks) {
  const { track } = useUnlisteners();

  // ---------- 渲染状态与列表 ----------

  const status = ref<BlockStatusDto | null>(null);
  const blocks = ref<BlockItemDto[]>([]);

  const rendered = computed(() => !!status.value?.rendered);
  const running = computed(() => !!status.value?.running);
  /** 渲染前置阶段，即拉清单 / 下载核心 jar：进度还没开始步进，显示下载文案而非 0 / 0 */
  const preparing = computed(() => running.value && (status.value?.now ?? 0) === 0);
  /** 渲染进度百分比（前置阶段未知，显示 0） */
  const percent = computed(() => {
    const total = status.value?.total ?? 0;
    return total > 0 ? Math.min(100, ((status.value?.now ?? 0) / total) * 100) : 0;
  });

  /** 拉取方块列表（按当前界面语言取名字） */
  async function loadBlocks() {
    try {
      blocks.value = await getBlockList(locale.value);
    } catch {
      blocks.value = [];
    }
  }

  // 语言切换后重新取名字（后端侧的名字已按语言翻译）
  watch(locale, () => void loadBlocks());
  // 已渲染但列表还没拉（如渲染结束事件刚到）时补一次
  watch(rendered, (val) => {
    if (val && blocks.value.length === 0) void loadBlocks();
  });

  // ---------- 事件订阅 ----------

  /** 已发出取消、还没收到渲染结束事件；渲染循环要几毫秒才收手，期间禁用按钮防连点 */
  const cancelling = ref(false);
  /** 本轮渲染是否已弹过下载窗口（渲染结束复位，下一轮再触发） */
  let downloadWinOpened = false;
  /** 上一次见到的渲染错误（相同错误只弹一次） */
  let lastError: string | null = null;

  onMounted(async () => {
    try {
      status.value = await getBlockStatus();
    } catch {
      status.value = null;
    }
    // 渲染已在跑且还没步进（下载阶段）：若有活跃下载任务，补开下载窗口。
    // 任务的 add 事件可能发生在本窗口打开前，事件弹窗路径会漏
    if (status.value?.running && (status.value.now ?? 0) === 0) {
      try {
        const ds = await api.getDownloadStatus();
        if (ds.tasks.length > 0 && !downloadWinOpened) {
          downloadWinOpened = true;
          openWindow("download");
        }
      } catch {
        // 状态查不到就算了，不影响主流程
      }
    }

    track(
      onBlockRender((e) => {
        status.value = e;
        // 渲染刚结束且已有结果时刷新列表：首次渲染完成时列表还没拉过
        if (!e.running && e.rendered) void loadBlocks();
        if (!e.running) {
          downloadWinOpened = false;
          cancelling.value = false;
        }
        // 渲染失败主动弹提示：重新渲染失败时界面停在网格视图，错误没有落点，
        // 只有 toast 能让用户知道；首次渲染失败另有提示卡显示详情
        if (e.error && e.error !== lastError) {
          showToast(e.error, 4000);
        }
        lastError = e.error ?? null;
      }),
    );

    // 首次渲染要下载游戏核心 jar：渲染中收到新下载任务就打开下载窗口，
    // 否则下载在后台静默进行，用户只看到 0 / 0 的渲染进度无从得知。
    // 事件可能早于本地状态（本轮从别处触发），running 以现查为准
    track(
      onDownloadTask(async (e) => {
        if (e.type !== "add") return;
        let st = status.value;
        if (!st?.running) {
          try {
            st = await getBlockStatus();
            status.value = st;
          } catch {
            return;
          }
        }
        if (st?.running && !downloadWinOpened) {
          downloadWinOpened = true;
          openWindow("download");
        }
      }),
    );

    if (status.value?.rendered) void loadBlocks();
  });

  // ---------- 渲染操作 ----------

  /** 开始渲染（首渲染；重试同路径） */
  async function startRender(force: boolean) {
    try {
      const started = await blockRenderStart(force);
      if (!started) showToast(t("blocks.rendering"));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 取消本轮渲染：内核侧协作式取消，随后会推一次 running=false 的状态 */
  async function cancelRender() {
    if (cancelling.value) return;
    cancelling.value = true;
    try {
      // 返回 false = 没有在跑的渲染（刚好结束），不会有后续状态事件，就地复位
      if (await blockRenderCancel()) {
        showToast(t("blocks.renderCancelled"));
      } else {
        cancelling.value = false;
      }
    } catch (e) {
      cancelling.value = false;
      showToast(tErr(e));
    }
  }

  // ---------- 视图偏好 ----------

  const pref = readPref();
  const keyword = ref(typeof pref.keyword === "string" ? pref.keyword : "");
  const cat = ref(typeof pref.cat === "string" ? pref.cat : "");
  const size = ref<BlockSize>(isBlockSize(pref.size) ? pref.size : "md");

  watch([cat, keyword, size], () => {
    writeJson(KEYS.blockView, { cat: cat.value, keyword: keyword.value, size: size.value });
  });

  // 离开方块列表时清掉搜索词；分类 / 图标尺寸档留着，那是浏览上下文。
  // 单窗口模式下这一页被 KeepAlive 缓存，切走不销毁，不清的话再打开时
  // 列表还筛着上次搜的词，用户早忘了自己搜过什么，只会觉得"怎么东西少了"。
  // 清空会经上面的 watch 写回偏好，所以重启后也不会又冒出来。
  onDeactivated(() => {
    keyword.value = "";
  });

  // ---------- 分类与过滤 ----------

  /** 出现过的分类（保持后端排序），原版分组后端已按游戏语言翻译，自定义分组走前端键 */
  const cats = computed(() => [...new Set(blocks.value.map((b) => b.cat).filter(Boolean))]);

  /** 各分类条目数（一次聚合，供左侧分类栏显示） */
  const catCounts = computed(() => {
    const map = new Map<string, number>();
    for (const b of blocks.value) {
      if (b.cat) map.set(b.cat, (map.get(b.cat) ?? 0) + 1);
    }
    return map;
  });

  // 当前分类消失时回到「全部」：重渲染换版本，或删掉最后一个玩家头颅。
  // 自建分组的键不在这份游戏分类里，删组时由上层自己清（见 BlockPanel）
  watch(cats, (list) => {
    if (cat.value && !cat.value.startsWith(GROUP_PREFIX) && !list.includes(cat.value)) {
      cat.value = "";
    }
  });

  /** 分类显示名：先查 i18n，miss 时回退原文（原版分组即游戏语言翻译结果） */
  function catLabel(c: string): string {
    const key = `blocks.cat.${c}`;
    const text = t(key);
    return text === key ? c : text;
  }

  /** 是否处于筛选态（决定计数文案与空状态） */
  const isFiltered = computed(() => !!keyword.value.trim() || !!cat.value);

  const filtered = computed(() => {
    const kw = keyword.value.trim().toLowerCase();
    const list = blocks.value.filter((b) => {
      if (cat.value) {
        // 自建分组：成员表说了算；游戏分类按条目自己的 cat 比
        if (cat.value.startsWith(GROUP_PREFIX)) {
          if (!hooks?.inGroup?.(cat.value.slice(GROUP_PREFIX.length), b.id)) return false;
        } else if (b.cat !== cat.value) {
          return false;
        }
      }
      if (!kw) return true;
      return (
        b.id.toLowerCase().includes(kw) ||
        b.name.toLowerCase().includes(kw) ||
        catLabel(b.cat).toLowerCase().includes(kw)
      );
    });

    // 收藏置顶：稳定排序（Array.sort 稳定），其余保持内核给的顺序
    const isFav = hooks?.isFavorite;
    if (!isFav) return list;
    return [...list].sort((a, b) => Number(isFav(b.id)) - Number(isFav(a.id)));
  });

  /** 清掉搜索词与分类，回到全部 */
  function clearFilters() {
    keyword.value = "";
    cat.value = "";
  }

  // ---------- 详情（选中） ----------
  //
  // 只存 id：列表刷新（换语言 / 重渲染）后对象会换新的，用 id 现查才不会指向旧对象。

  const detailId = ref<string | null>(null);

  const detail = computed(() =>
    detailId.value ? (blocks.value.find((b) => b.id === detailId.value) ?? null) : null,
  );

  function openDetail(b: BlockItemDto) {
    detailId.value = b.id;
  }

  function closeDetail() {
    detailId.value = null;
  }

  // ---------- 玩家头颅 ----------

  /** 按用户名或 UUID 添加（同名覆盖），成功后刷新列表；返回是否成功 */
  async function addSkin(input: string): Promise<boolean> {
    try {
      await blockSkinAdd(input);
      showToast(t("blocks.skinAddOk"));
      await loadBlocks();
      return true;
    } catch (e) {
      showToast(tErr(e));
      return false;
    }
  }

  /** 删除玩家头颅；id 上的 `custom:` 前缀要去掉 */
  async function removeSkin(b: BlockItemDto) {
    try {
      await blockSkinRemove(b.id.slice("custom:".length));
      showToast(t("blocks.skinRemoveOk"));
      if (detailId.value === b.id) closeDetail();
      await loadBlocks();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 把方块设为某实例的图标；返回是否成功 */
  async function setIcon(uuid: string, blockId: string, instanceName: string): Promise<boolean> {
    try {
      await blockSetIcon(uuid, blockId);
      showToast(t("blocks.setIconOk", { name: instanceName }));
      return true;
    } catch (e) {
      showToast(tErr(e));
      return false;
    }
  }

  return {
    status,
    blocks,
    rendered,
    running,
    preparing,
    percent,
    cancelling,
    startRender,
    cancelRender,
    loadBlocks,
    keyword,
    cat,
    size,
    cats,
    catCounts,
    catLabel,
    isFiltered,
    filtered,
    clearFilters,
    detail,
    openDetail,
    closeDetail,
    addSkin,
    removeSkin,
    setIcon,
  };
}
