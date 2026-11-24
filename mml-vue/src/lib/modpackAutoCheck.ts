// 本次启动器运行里已经自动查过整合包更新的实例
//
// 必须放模块级：实例设置面板随实例切换重建，写在 <script setup> 里就是每个实例一份，
// 切走再切回来又会查一次。判据只有实例 uuid —— 同一个实例一次运行只自动查一次，
// 查失败也不重查，重查是用户点「检查更新」的事。

/** 已经自动查过的实例 uuid */
const checked = new Set<string>();

/**
 * 这次要不要给实例自动查更新；要查就顺手记下，保证每个实例只查一次
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * `true` = 本次运行还没查过，调用方去查；`false` = 已经查过，别再查
 */
export function takeModpackAutoCheck(uuid: string): boolean {
  if (checked.has(uuid)) return false;
  checked.add(uuid);
  return true;
}
