// Modrinth 分类图标的内联 svg 消毒
//
// `TagDto.svg` 是 Modrinth 给的内联 svg 源码，stroke=currentColor，渲染前必须过 DOMPurify 再
// `v-html`。模板里直接调用函数，每次重渲染都会重跑一遍消毒，而同一批图标的源码是不变的，
// 所以这里按源码缓存结果。
import DOMPurify from "dompurify";

/** 消毒结果缓存，键是原始 svg 源码 */
const cache = new Map<string, string>();

/**
 * 消毒一段 Modrinth 内联 svg 源码
 *
 * - `svg`: 后端下发的内联 svg 源码
 *
 * # 返回值
 *
 * 可安全 `v-html` 的 svg 源码；同一段源码只消毒一次
 */
export function svgIcon(svg: string): string {
  const hit = cache.get(svg);
  if (hit !== undefined) return hit;
  const safe = DOMPurify.sanitize(svg);
  cache.set(svg, safe);
  return safe;
}
