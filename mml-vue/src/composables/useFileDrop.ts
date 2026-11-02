// 拖拽外部文件 / 文本到窗口
//
// 按平台二选一，两套不能同时开，建窗那边的开关见
// `mml-gui/src-tauri/src/windows/lifecycle/create.rs`：
//
// - **Windows**：建窗时关掉了 wry 的拖放句柄，走 **DOM 拖放**（`dragenter` / `dragover` /
//   `dragleave` / `drop`）。框架那份实现会顶掉 WebView2 的拖放目标、只认文件，网页上
//   拖下来的卡片连 `dragenter` 都收不到。DOM 全功能，只缺**文件真实路径** —— 由 Rust 侧
//   的"路径桥"补：注入脚本把 `File` 对象交给原生，原生取路径后发 `file-drop`，
//   见 `windows/file_drop.rs`。
// - **macOS**：wry 的句柄还在，DOM 收不到拖放事件，路径与进出都来自
//   `DragDrop` 原生事件（`onDragDropEvent`）。
// - **Linux**：同一个原生句柄管文件，但 wry 的 `drag-motion` 放行，WebKit 照旧把
//   **文本 / 链接**派给页面。文件拖入两条都会到，原生先给出路径，DOM 那条让开。
//
// 几个来源合成同一个回调，调用方只看 `paths` 与 `url`。
import { onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { isArchivePath } from "../lib/archiveFile";
import { commands } from "../lib/bindings";
import type { FileDropDto } from "../lib/bindings";
import { FileDrop } from "../lib/listens";
import { detectOs } from "../lib/titlebar";
import { isTauri } from "../lib/util";
import { useUnlisteners } from "./useUnlisteners";

/** 一次拖入的数据 */
export interface FileDropPayload {
  /** 拖入文件的绝对路径；DOM 里拿不到，由 `file-drop` 事件补，纯文本拖入时为空数组 */
  paths: string[];
  /** 从文本 / 链接里解析出的地址；没有则空串 */
  url: string;
  /** 拖入数据的类型清单，含 `Files` 表示拖的是文件；**悬停阶段只有它可用** */
  types: string[];
  /**
   * 悬停阶段的"是不是目录"；**只有悬停时有值**，松手后为 `null`
   *
   * 靠 Chromium 的 `webkitGetAsEntry()` 认，拿不到就退回按 MIME 猜。松手后以 `paths` 为准。
   */
  isDirectory: boolean | null;
}

/** 拖放判定选项 */
export interface FileDropOptions {
  /**
   * 这次拖入能不能接
   *
   * **只在悬停阶段问一次**：结果决定遮罩显不显示、`dragover` 放不放行。不接的内容
   * 连拖都拖不进来 —— 不 `preventDefault`，系统会画"禁止"光标、松手也没有 drop。
   * 此时 `paths` 必为空，判定只能看 `types` 与 `isDirectory`。松手后的内容过滤与
   * 提示由 `onDrop` 负责。
   */
  accept?: (payload: FileDropPayload) => boolean | Promise<boolean>;
}

/** 拖入数据里的文本：优先 `text/uri-list`（浏览器拖链接时最干净），再 `text/plain`，最后从 html 里找 */
function readDropText(data: DataTransfer | null): string {
  if (!data) return "";
  const uri = data.getData("text/uri-list").trim();
  if (uri) return uri;
  const plain = data.getData("text/plain").trim();
  if (plain) return plain;
  return data.getData("text/html");
}

/**
 * 这次拖入的细分类型，遮罩文案按它变
 *
 * 悬停阶段 `getData` / `files` 都是空的，浏览器安全设计如此，只能靠
 * `webkitGetAsEntry()` 与每项的 MIME 凑：它认不出目录时，MIME 全为空才按 `folder` 猜。
 */
export type DragKind = "" | "text" | "archive" | "folder" | "files";

/**
 * 这一平台的文件拖放走 wry 原生事件
 *
 * Windows 要关掉 wry 的句柄，WebView2 才会把拖放事件交给页面；其余平台反过来，
 * 路径只在原生事件里。开关在建窗那边，见 `mml-gui/src-tauri/src/windows/lifecycle/create.rs`。
 *
 * Linux 上 WebKit 与原生句柄会同时看到同一次拖入，所以 DOM 那条不能整个关掉：
 * 文本 / 链接只有 DOM 拿得到，文件要避让原生（见 `wryDropAt`）。
 */
const USE_WRY_DROP = isTauri() && detectOs() !== "windows";

/** 认得出是压缩包的 MIME；Chromium 按扩展名映射，未知扩展名给空串 */
const ARCHIVE_MIME = [
  "application/zip",
  "application/x-zip-compressed",
  "application/x-7z-compressed",
  "application/x-rar-compressed",
  "application/x-compressed",
  "application/gzip",
  "application/x-tar",
];

/** 按类型清单与 MIME 判这次拖入是哪一档 */
function dragKindOf(
  types: string[],
  items: DataTransferItem[],
  isDirectory: boolean | null,
): DragKind {
  if (!types.length) return "";
  if (!types.includes("Files")) return "text";
  // 能认出来是目录就直接算目录（比 MIME 准）
  if (isDirectory === true) return "folder";
  const mimes = items.filter((item) => item.kind === "file").map((item) => item.type);
  if (!mimes.length) return "files";
  if (mimes.some((mime) => ARCHIVE_MIME.includes(mime))) return "archive";
  // MIME 全为空且 isDirectory === null 时按目录报：目录是常见情况；
  // 能确定不是目录那就是扩展名认不出的普通文件
  if (isDirectory === null && mimes.every((mime) => !mime)) return "folder";
  return "files";
}

/**
 * 悬停阶段判断"是不是目录"，尽力而为
 *
 * 拿不到就返回 `null` = 认不出来，调用方退回按 MIME 猜。`webkitGetAsEntry()` 不可用
 * 或返回 null 都算拿不到。多个文件里只要有一个是目录就按目录算。
 */
function hoverIsDirectory(items: DataTransferItem[]): boolean | null {
  const entries = items
    .filter((item) => item.kind === "file")
    .map((item) => {
      // Chromium 私有 API：抛异常也不能把 dragenter 打断（否则遮罩与判定全没了）
      try {
        return item.webkitGetAsEntry ? item.webkitGetAsEntry() : null;
      } catch {
        return null;
      }
    });
  if (!entries.length || entries.some((entry) => !entry)) return null;
  return entries.some((entry) => entry!.isDirectory);
}

/**
 * 给本窗口挂上拖放监听
 *
 * - `onDrop`: 松手时回调，文件路径与文本地址都在这份数据里
 * - `options.accept`: 自定义"这次拖入接不接"
 *
 * # 返回值
 *
 * - `fileDragOver`: 是否正拖在窗口上，给遮罩层用
 * - `dragKind`: 这次拖的是哪一档，见 `DragKind`；`""` 表示未知或拖入已结束
 */
export function useFileDrop(
  onDrop: (payload: FileDropPayload) => void | Promise<void>,
  options: FileDropOptions = {},
) {
  const fileDragOver = ref(false);
  const dragKind = ref<DragKind>("");
  const { track } = useUnlisteners();
  /** 本次拖入的世代号：异步判定回来时若已经换了新的一次，结果作废 */
  let dragSeq = 0;
  /** 等路径桥那一步的暂存：DOM 的 drop 有文件时先不发，等 `file-drop` 送来真实路径 */
  let pending: { url: string; types: string[]; timer: number } | null = null;
  /**
   * 最近一次原生 drop 的时刻
   *
   * Linux 上 wry 的 `drag-motion` 放行，WebKit 仍会把拖放派给页面，同一次文件拖入
   * 原生与 DOM 都会到；原生已经给出路径时 DOM 那条要让开，免得处理两遍
   */
  let wryDropAt = 0;

  /** 这次拖入接不接（没传 accept 就一律接） */
  async function accepted(payload: FileDropPayload): Promise<boolean> {
    if (!options.accept) return true;
    try {
      return await options.accept(payload);
    } catch (err) {
      console.error("[useFileDrop] 判定拖入是否可接受失败", err);
      return false;
    }
  }

  /** 撤掉等路径桥的暂存：原生已经给出路径时不必再等 */
  function clearPending() {
    if (!pending) return;
    window.clearTimeout(pending.timer);
    pending = null;
  }

  /** 原生拖入的细分类型：拿不到目录与 MIME，只按后缀认压缩包 */
  function nativeKindOf(paths: string[]): DragKind {
    return paths.some(isArchivePath) ? "archive" : "files";
  }

  /**
   * wry 原生拖放事件（macOS / Linux）
   *
   * 没有路径的拖入是文本 / 链接：原生这边读不到内容，直接忽略，别亮一个最后什么也做不了的遮罩。
   *
   * - `event`: Tauri 的 `DragDrop` 事件，进 / 出 / 悬停 / 松手四种
   */
  function onWryDrop(event: { payload: DragDropEvent }) {
    const payload = event.payload;
    if (payload.type === "enter") {
      if (!payload.paths.length) return;
      dragKind.value = nativeKindOf(payload.paths);
      // 同一世代号才落状态：悬停期间会连发 enter，慢的那次不能盖掉后一次
      const seq = ++dragSeq;
      void accepted({
        paths: payload.paths,
        url: "",
        types: ["Files"],
        isDirectory: null,
      }).then((ok) => {
        if (seq === dragSeq && ok) fileDragOver.value = true;
      });
      return;
    }
    // over 只是位置变化，遮罩状态由 enter / leave 定
    if (payload.type === "over") return;

    dragSeq++;
    fileDragOver.value = false;
    dragKind.value = "";
    if (payload.type === "leave" || !payload.paths.length) return;

    // DOM 那条可能已经先把这次拖入记成"等路径"，这里给出路径后它不必再等
    wryDropAt = Date.now();
    clearPending();
    const data: FileDropPayload = {
      paths: payload.paths,
      url: "",
      types: ["Files"],
      isDirectory: null,
    };
    void (async () => {
      // 原生句柄拿到的 drop 无法拒绝，悬停时判过的这里要再判一次
      if (!(await accepted(data))) return;
      await onDrop(data);
    })();
  }

  /** 文本 → 地址；规则与 Rust 侧的 `window_url_of_text` 共用一份 */
  async function urlOfText(text: string): Promise<string> {
    try {
      return await commands.windows.urlOfText(text);
    } catch (err) {
      console.error("[useFileDrop] 解析拖入文本失败", err);
      return "";
    }
  }

  function onDomEnter(event: DragEvent) {
    const types = event.dataTransfer ? [...event.dataTransfer.types] : [];
    const items = event.dataTransfer ? [...event.dataTransfer.items] : [];
    const isDirectory = hoverIsDirectory(items);
    dragKind.value = dragKindOf(types, items, isDirectory);
    if (!options.accept) {
      // 没有判定就是一律接：同步亮遮罩，免得第一帧 dragover 被当成"不接"闪一下光标
      fileDragOver.value = true;
      return;
    }
    const seq = ++dragSeq;
    void accepted({ paths: [], url: "", types, isDirectory }).then((ok) => {
      if (seq === dragSeq && ok) {
        fileDragOver.value = true;
      }
    });
  }

  function onDomOver(event: DragEvent) {
    // 不接的内容**不 preventDefault**：系统画"禁止"光标、松手也没有 drop（"不符合就拖不进来"）
    if (!fileDragOver.value) {
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = "none";
      }
      return;
    }
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "copy";
    }
  }

  /**
   * 原生 dragstart 一律挡掉
   *
   * 本应用的拖动都走指针模拟；但**图片与文字默认能发起原生拖拽** —— 拖一行存档
   * （起点落在图标上）就会触发它，窗口的 dragenter / dragover 随即把"拖入文件"的遮罩
   * 亮出来。从窗口外拖进来的文件不经过 dragstart，不受影响。
   *
   * - `event`: 原生拖拽事件
   */
  function onDomDragStart(event: DragEvent) {
    event.preventDefault();
  }

  function onDomLeave(event: DragEvent) {
    // 在子元素之间移动也会派发 dragleave：relatedTarget 为 null 才是真的离开窗口
    if (event.relatedTarget) return;
    dragSeq++;
    fileDragOver.value = false;
    dragKind.value = "";
  }

  function onDomDrop(event: DragEvent) {
    event.preventDefault();
    dragSeq++;
    fileDragOver.value = false;
    dragKind.value = "";
    // 松手时不再问一遍 accept：不接的内容 dragover 就没放行，根本不会有 drop
    const data = event.dataTransfer;
    const types = data ? [...data.types] : [];
    const hasFiles = !!data && data.files.length > 0;
    // 原生句柄刚处理过这次文件拖入：路径已经用它那份，DOM 这条再来一遍就是重复
    if (hasFiles && Date.now() - wryDropAt < 500) return;
    const text = readDropText(data);

    void (async () => {
      const url = text ? await urlOfText(text) : "";
      if (!hasFiles) {
        // 纯文本 / 链接（网页卡片就是这种）
        await onDrop({ paths: [], url, types, isDirectory: null });
        return;
      }
      // 有文件：真实路径只有原生拿得到 —— Windows 等 `file-drop` 路径桥，macOS / Linux
      // 等原生 DragDrop。都没等到就按"没拿到路径"提示，不静默丢
      const entry = { url, types, timer: 0 };
      entry.timer = window.setTimeout(() => {
        if (pending !== entry) return;
        pending = null;
        void onDrop({ paths: [], url: entry.url, types: entry.types, isDirectory: null });
      }, 500);
      pending = entry;
    })();
  }

  /** 路径桥送来真实路径（文件拖入） */
  function onNativePaths(paths: string[]) {
    const entry = pending;
    clearPending();
    void onDrop({
      paths,
      url: entry?.url ?? "",
      types: entry?.types ?? [],
      isDirectory: null,
    });
  }

  onMounted(() => {
    // 挡在 dragenter 之前：不挡的话内部元素的原生拖拽会把"拖入文件"的遮罩带出来
    window.addEventListener("dragstart", onDomDragStart);
    window.addEventListener("dragenter", onDomEnter);
    window.addEventListener("dragover", onDomOver);
    window.addEventListener("dragleave", onDomLeave);
    window.addEventListener("drop", onDomDrop);

    // 订阅交给 track：卸载早于订阅完成时就地退订，别把监听留到窗口关掉之后
    if (USE_WRY_DROP) {
      const subscribing = getCurrentWebview().onDragDropEvent(onWryDrop);
      subscribing.catch((err) => {
        console.error("[useFileDrop] 注册原生拖放失败（文件拖入将没有反应）", err);
      });
      track(subscribing);
      return;
    }
    // 路径桥只在 Windows 的 Tauri 下有：浏览器预览里文件拖入拿不到路径（文本那条照旧可用）
    if (!isTauri()) return;
    const subscribing = listen<FileDropDto>(FileDrop, (event) => {
      onNativePaths(event.payload.paths ?? []);
    });
    subscribing.catch((err) => {
      console.error("[useFileDrop] 注册路径桥事件失败（文件拖入将拿不到路径）", err);
    });
    track(subscribing);
  });

  onUnmounted(() => {
    window.removeEventListener("dragstart", onDomDragStart);
    window.removeEventListener("dragenter", onDomEnter);
    window.removeEventListener("dragover", onDomOver);
    window.removeEventListener("dragleave", onDomLeave);
    window.removeEventListener("drop", onDomDrop);
    dragKind.value = "";
    if (pending) {
      window.clearTimeout(pending.timer);
      pending = null;
    }
  });

  return { fileDragOver, dragKind };
}
