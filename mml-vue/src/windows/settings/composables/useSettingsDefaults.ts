// 设置窗口 · 出厂默认值（「恢复默认」用）
//
// 默认值的真源在 Rust 的 `HttpObj::default` / `RunArgObj::new` / `WindowSettingObj::new`，
// 前端不复制一份（两份口径必然漂移）。这里只负责拉一次并缓存：
// 恢复默认是低频操作，但同一轮里多个分组可能连续恢复，缓存省掉重复 IPC。
import { commands, type SettingsDefaultsDto } from "../../../lib/bindings";

let cache: SettingsDefaultsDto | null = null;
let inflight: Promise<SettingsDefaultsDto | null> | null = null;

/**
 * 取出厂默认值，只拉一次并缓存
 *
 * # 返回值
 *
 * 出厂默认值；后端不可用时为 `null`，调用方据此放弃恢复
 */
export async function loadDefaults(): Promise<SettingsDefaultsDto | null> {
  if (cache) return cache;
  if (!inflight) {
    inflight = commands.settings
      .getDefaults()
      .then((dto) => {
        cache = dto;
        return dto;
      })
      .catch(() => null)
      .finally(() => {
        inflight = null;
      });
  }
  return inflight;
}
