// 拖拽外部文件 / 文本到窗口
//
// 走 **DOM 拖放**：监听 `dragenter` / `dragover` / `dragleave` / `drop` 四个事件。
// 框架 wry 那份原生实现会顶掉 WebView2 自己的拖放目标、而且只认文件，网页上拖下来的
// 卡片是文本 / 链接，连 `dragenter` 都收不到，拖动时鼠标显示"禁止"。现在建窗时已经
// 把它关掉，见 `mml-gui/src-tauri/src/windows/lifecycle.rs`，DOM 拖放因此是全功能的。
//
// 唯一缺的是**文件的真实路径** —— DOM 的 `File` 对象按浏览器安全设计不给路径，
// 由 Rust 侧的"路径桥"补：注入脚本把 `File` 对象交给原生，原生取出路径后发 `file-drop`
// 事件，见 `windows/file_drop.rs`。于是这里有两个来源：
//
//   ① DOM 的 `drop`    → 文本 / 链接、`Files` 类型清单
//   ② `file-drop` 事件 → 文件真实路径
//
// 两者合成同一个回调，调用方只看 `paths` 与 `url`。
import { onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { commands } from "../lib/bindings";
import type { FileDropDto } from "../lib/bindings";
import { FileDrop } from "../lib/listens";
import { isTauri } from "../lib/util";

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
 * 悬停阶段"是不是目录"（best effort）
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
 * - `onDrop`: 松手时回调，文件路径与文本地址都在里面
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
  const unlisteners: Array<() => void> = [];
  /** 本次拖入的世代号：异步判定回来时若已经换了新的一次，结果作废 */
  let dragSeq = 0;
  /** 等路径桥那一步的暂存：DOM 的 drop 有文件时先不发，等 `file-drop` 送来真实路径 */
  let pending: { url: string; types: string[]; timer: number } | null = null;

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

  /**
   * 交付一次拖入
   *
   * 松手时**不再问一遍 accept**：能不能接在悬停阶段已经定过 —— 不接的内容 `dragover`
   * 就不放行、根本不会有 drop。再判一次只会多一轮探测，还会在判定不通过时静默丢弃。
   */
  async function deliver(payload: FileDropPayload) {
    await onDrop(payload);
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

    const data = event.dataTransfer;
    const types = data ? [...data.types] : [];
    const hasFiles = !!data && data.files.length > 0;
    const text = readDropText(data);

    void (async () => {
      const url = text ? await urlOfText(text) : "";
      if (!hasFiles) {
        // 纯文本 / 链接（网页卡片就是这种）
        await deliver({ paths: [], url, types, isDirectory: null });
        return;
      }
      // 有文件：路径只有原生拿得到，等 `file-drop`
      const entry = { url, types, timer: 0 };
      entry.timer = window.setTimeout(() => {
        if (pending !== entry) return;
        pending = null;
        void deliver({ paths: [], url: entry.url, types: entry.types, isDirectory: null });
      }, 500);
      pending = entry;
    })();
  }

  /** 路径桥送来真实路径（文件拖入） */
  function onNativePaths(paths: string[]) {
    const entry = pending;
    if (entry) {
      window.clearTimeout(entry.timer);
      pending = null;
    }
    void deliver({
      paths,
      url: entry?.url ?? "",
      types: entry?.types ?? [],
      isDirectory: null,
    });
  }

  onMounted(async () => {
    window.addEventListener("dragenter", onDomEnter);
    window.addEventListener("dragover", onDomOver);
    window.addEventListener("dragleave", onDomLeave);
    window.addEventListener("drop", onDomDrop);

    // 路径桥只在 Tauri 下有：浏览器预览里文件拖入拿不到路径（文本那条照旧可用）
    if (!isTauri()) return;
    try {
      unlisteners.push(
        await listen<FileDropDto>(FileDrop, (event) => {
          onNativePaths(event.payload.paths ?? []);
        }),
      );
    } catch (err) {
      console.error("[useFileDrop] 注册路径桥事件失败（文件拖入将拿不到路径）", err);
    }
  });

  onUnmounted(() => {
    window.removeEventListener("dragenter", onDomEnter);
    window.removeEventListener("dragover", onDomOver);
    window.removeEventListener("dragleave", onDomLeave);
    window.removeEventListener("drop", onDomDrop);
    unlisteners.forEach((fn) => fn());
    unlisteners.length = 0;
    dragKind.value = "";
    if (pending) {
      window.clearTimeout(pending.timer);
      pending = null;
    }
  });

  return { fileDragOver, dragKind };
}
