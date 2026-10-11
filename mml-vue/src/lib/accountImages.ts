// 账户皮肤 / 头像 / 披风图片
// 优先走 mml-image 协议由 Rust 渲染真实皮肤：后端按账户类型从对应认证服务器拉取，
// 离线或拉取失败时该 URI 返回 400，前端监听 img @error 回退到 SVG 占位图
import { ref } from "vue";
import type { AccountStoreDto } from "./bindings";
import { getImageBaseUrl } from "./api";

/** mml-image 协议前缀：进窗口后异步取一次，浏览器预览取不到时保持空串走占位图 */
export const imageBase = ref("");

void getImageBaseUrl()
  .then((url) => {
    imageBase.value = url;
  })
  .catch(() => {
    /* 浏览器环境：保持空串，始终用占位图 */
  });

/** 图片版本号：皮肤 / 头像显示模式变化后 +1，URL 带 v= 让浏览器丢弃旧渲染图 */
export const imageVersion = ref(0);

/** 皮肤或头像显示设置变化后调用，所有账户图片立即按新模式重取 */
export function bumpImageVersion() {
  imageVersion.value += 1;
}

/**
 * 清掉某账户的加载失败 / 已加载标记，刷新皮肤后让它重走一遍加载流程
 *
 * - `acc`: 目标账户
 */
export function resetImageState(acc: AccountStoreDto): void {
  const keep = (set: Set<string>) =>
    new Set([...set].filter((key) => !key.startsWith(`${acc.uuid}:`)));
  failedImages.value = keep(failedImages.value);
  loadedImages.value = keep(loadedImages.value);
}

/**
 * 头像地址，后端按头像配置选渲染方式
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 渲染图 URL；协议前缀未就绪时为空串
 */
export function accountAvatarUrl(acc: AccountStoreDto): string {
  return imageBase.value
    ? `${imageBase.value}/head/${acc.authType}/${acc.uuid}?v=${imageVersion.value}`
    : "";
}

/**
 * 皮肤全身图地址，后端按全局显示模式配置选 2D 展开图 / 2D 大图 / 3D 等距模型
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 渲染图 URL；协议前缀未就绪时为空串
 */
export function accountSkinUrl(acc: AccountStoreDto): string {
  if (!imageBase.value) return "";
  return `${imageBase.value}/skin/${acc.authType}/${acc.uuid}/auto?v=${imageVersion.value}`;
}

/**
 * 披风渲染图地址，后端渲染 2D 正面
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 渲染图 URL；协议前缀未就绪时为空串
 */
export function accountCapeUrl(acc: AccountStoreDto): string {
  return imageBase.value ? `${imageBase.value}/cape/${acc.authType}/${acc.uuid}` : "";
}

/**
 * 披风背面渲染图地址，后端渲染 2D 背面，悬浮大图与正面并排用
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 渲染图 URL；协议前缀未就绪时为空串
 */
export function accountCapeBackUrl(acc: AccountStoreDto): string {
  return imageBase.value ? `${imageBase.value}/capeback/${acc.authType}/${acc.uuid}` : "";
}

/** 图片加载失败标记，键为 `uuid:kind`，标记后回退占位图 */
export const failedImages = ref(new Set<string>());

/**
 * 某账户某张图是否已回退占位
 *
 * 协议前缀未就绪时也算"失败"，但此时 img 根本不渲染，不会像 src="" 那样
 * 立刻触发 @error 把图永久标记失败。之前 base 是异步取的，网格先渲染出 src="" 的 img，
 * error 事件抢在 base 到手前把所有图标记掉，导致皮肤 / 披风请求从未真正发出。
 *
 * - `acc`: 目标账户
 * - `kind`: 图片种类，`avatar` / `skin` / `cape`
 *
 * # 返回值
 *
 * 已标记失败或协议前缀未就绪时为 true
 */
export function imageFailed(acc: AccountStoreDto, kind: string): boolean {
  return !imageBase.value || failedImages.value.has(`${acc.uuid}:${kind}`);
}

/**
 * 标记某账户某张图加载失败，触发 Set 更新以重渲染
 *
 * - `acc`: 目标账户
 * - `kind`: 图片种类，`avatar` / `skin` / `cape`
 */
export function markImageFailed(acc: AccountStoreDto, kind: string): void {
  const next = new Set(failedImages.value);
  next.add(`${acc.uuid}:${kind}`);
  failedImages.value = next;
}

/**
 * 某账户是否确认"没有皮肤"，用于隐藏依赖皮肤的按钮
 *
 * 与 [`imageFailed`] 的区别在于**协议前缀未就绪时不算没有皮肤**：
 * `imageFailed` 把 base 为空串也当作失败，那是给渲染逻辑用的；而 base 是进窗口后
 * 异步取的，页面刚打开那一瞬间它还是空的，直接拿 imageFailed 当判据会让
 * 「查看皮肤 / 刷新皮肤」在加载完成前先闪一下或干脆不出现。
 *
 * 这里只在图片真的加载失败过时才认定没有皮肤，服务端返回 400 或解码失败都算。
 * 宁可多显示两个按钮，也不要让它们抖动。
 *
 * - `acc`: 目标账户
 *
 * # 返回值
 *
 * 协议前缀未就绪时为 false
 */
export function hasNoSkin(acc: AccountStoreDto): boolean {
  return !imageBase.value ? false : failedImages.value.has(`${acc.uuid}:skin`);
}

/** 已成功加载的图，键为 `uuid:kind`，加载中转圈用 */
export const loadedImages = ref(new Set<string>());

/**
 * 某账户某张图是否正在加载：协议前缀就绪、未失败、还没 onload
 *
 * - `acc`: 目标账户
 * - `kind`: 图片种类，`avatar` / `skin` / `cape`
 *
 * # 返回值
 *
 * 正在加载中时为 true
 */
export function imageLoading(acc: AccountStoreDto, kind: string): boolean {
  return (
    !!imageBase.value &&
    !imageFailed(acc, kind) &&
    !loadedImages.value.has(`${acc.uuid}:${kind}`)
  );
}

/**
 * 标记某账户某张图加载完成
 *
 * - `acc`: 目标账户
 * - `kind`: 图片种类，`avatar` / `skin` / `cape`
 */
export function markImageLoaded(acc: AccountStoreDto, kind: string): void {
  if (loadedImages.value.has(`${acc.uuid}:${kind}`)) return;
  const next = new Set(loadedImages.value);
  next.add(`${acc.uuid}:${kind}`);
  loadedImages.value = next;
}

function svg(w: number, h: number, body: string): string {
  const data =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`;
  return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(data);
}

function seedColor(seed: number, salt: number): string {
  const h = ((seed * 2654435761 + salt * 40503) >>> 0).toString(16).padStart(6, "0");
  return h.slice(0, 6);
}

/**
 * 头像占位图，64x64，皮肤拉取失败 / 无皮肤时兜底
 *
 * - `seed`: 配色种子，同一账户固定
 * - `skin`: 皮肤底色，`#rrggbb`
 *
 * # 返回值
 *
 * SVG data URL
 */
export function avatarImage(seed: number, skin: string): string {
  const hair = seedColor(seed, 1);
  const bg = seedColor(seed, 2);
  return svg(
    64,
    64,
    `<rect width="64" height="64" fill="#${bg}"/>` +
    `<rect x="14" y="12" width="36" height="38" rx="7" fill="${skin}"/>` +
    `<rect x="14" y="12" width="36" height="12" rx="4" fill="#${hair}"/>` +
    `<rect x="22" y="28" width="8" height="10" rx="2" fill="#fff"/><rect x="34" y="28" width="8" height="10" rx="2" fill="#fff"/>` +
    `<rect x="24" y="30" width="4" height="6" rx="1" fill="#1f2937"/><rect x="36" y="30" width="4" height="6" rx="1" fill="#1f2937"/>` +
    `<rect x="26" y="40" width="12" height="4" rx="2" fill="#1f2937"/>`,
  );
}

/**
 * 皮肤占位图，64x128，正面全身
 *
 * - `seed`: 配色种子，同一账户固定
 * - `skin`: 皮肤底色，`#rrggbb`
 *
 * # 返回值
 *
 * SVG data URL
 */
export function skinImage(seed: number, skin: string): string {
  const hair = seedColor(seed, 3);
  const shirt = seedColor(seed, 4);
  const bg = seedColor(seed, 5);
  return svg(
    64,
    128,
    `<rect width="64" height="128" fill="#${bg}"/>` +
    `<rect x="20" y="6" width="24" height="24" rx="3" fill="${skin}"/>` +
    `<rect x="20" y="6" width="24" height="8" rx="3" fill="#${hair}"/>` +
    `<rect x="24" y="18" width="5" height="5" rx="1" fill="#fff"/><rect x="35" y="18" width="5" height="5" rx="1" fill="#fff"/>` +
    `<rect x="18" y="32" width="28" height="38" rx="4" fill="#${shirt}"/>` +
    `<rect x="6" y="32" width="10" height="36" rx="4" fill="#${shirt}"/>` +
    `<rect x="48" y="32" width="10" height="36" rx="4" fill="#${shirt}"/>` +
    `<rect x="20" y="72" width="11" height="30" rx="4" fill="#334155"/>` +
    `<rect x="33" y="72" width="11" height="30" rx="4" fill="#334155"/>`,
  );
}

/**
 * 披风占位图，64x32
 *
 * - `seed`: 配色种子，同一账户固定
 * - `skin`: 皮肤底色，`#rrggbb`
 *
 * # 返回值
 *
 * SVG data URL
 */
export function capeImage(seed: number, skin: string): string {
  const c1 = seedColor(seed, 6);
  const c2 = seedColor(seed, 7);
  return svg(
    64,
    32,
    `<rect width="64" height="32" fill="#${c1}"/>` +
    `<rect x="10" y="6" width="12" height="22" rx="3" fill="${skin}"/>` +
    `<rect x="26" y="6" width="12" height="22" rx="3" fill="#${c2}"/>` +
    `<rect x="42" y="6" width="12" height="22" rx="3" fill="#${c2}"/>`,
  );
}
