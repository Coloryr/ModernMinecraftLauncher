// 实例图标的跨组件缓存
//
// 普通行 / 占满后的多列行 / 折叠展开都会重建 InstanceIcon，缓存必须放在
// <script setup> 外面；写在组件里的顶层变量是每个实例一份，重建就没了。
import { getImageBaseUrl } from "./api";

/** mml-image 前缀；null 表示还没取过 */
let base: string | null = null;
let basePromise: Promise<string> | null = null;

/** 已加载成功的图标地址 */
export const loadedIconUrls = new Set<string>();
/** 已加载成功的实例 uuid，地址拼接变化时也能认出是同一张图 */
export const loadedIconUuids = new Set<string>();
/** 已取到的 Blob URL，重建时直接画缓存 */
export const iconObjectUrls = new Map<string, string>();

/** 取当前缓存的前缀；没取过为 null */
export function cachedImageBase(): string | null {
  return base;
}

/** 取 mml-image 前缀；成功后缓存，重建组件不再等 IPC */
export function loadImageBase(): Promise<string> {
  if (base !== null) return Promise.resolve(base);
  basePromise ??= getImageBaseUrl()
    .then((value) => {
      base = value;
      return value;
    })
    .catch(() => {
      base = "";
      return "";
    });
  return basePromise;
}

/**
 * 预取一组实例图标
 *
 * 折叠分组里的实例行还没挂载，先按 uuid 把图取进浏览器缓存；
 * 第一次展开时 InstanceIcon 直接命中已加载状态，不会先闪占位。
 *
 * - `uuids`: 实例 uuid 列表
 */
export function preloadInstanceIcons(uuids: string[]) {
  void loadImageBase().then((base) => {
    if (!base) return;
    for (const uuid of uuids) {
      const link = `${base}/instance/${uuid}?v=0`;
      if (loadedIconUrls.has(link) || iconObjectUrls.has(link)) continue;
      const image = new Image();
      image.onload = () => {
        loadedIconUrls.add(link);
        loadedIconUuids.add(uuid);
        cacheIconBlob(link);
      };
      image.src = link;
    }
  });
}

/** 把协议地址取成 Blob URL，供下一次重建直接使用 */
export function cacheIconBlob(link: string) {
  if (!link || iconObjectUrls.has(link)) return;
  fetch(link)
    .then((response) => response.blob())
    .then((blob) => {
      if (!iconObjectUrls.has(link)) iconObjectUrls.set(link, URL.createObjectURL(blob));
    })
    .catch(() => {
      /* 取不到就继续用协议地址 */
    });
}
