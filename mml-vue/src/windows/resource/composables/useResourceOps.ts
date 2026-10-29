// 资源管理窗口的共用操作：忙碌标记 / 确认弹窗 / 打开目录 / 操作后重拉
//
// 各分类的增删改在各自的 parts/*Pane.vue 里（它们知道该调哪个命令），
// 这里只放"所有分类都要走一遍"的那几件：串行化、失败提示、确认、刷新列表、
// 以及"打开目录不重拉列表"这条容易写错的边界（见 openFolder）。
import { ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { openResourceFolder, renameResource } from "../../../lib/api";
import type { useResourceData } from "./useResourceData";

/** 确认弹窗的内容 */
interface ConfirmBox {
  /** 弹窗标题，调用方已 `t()` 好的成品 */
  title: string;
  /** 弹窗正文 */
  text: string;
  /** 点确认后执行的动作 */
  run: () => Promise<void>;
  /** 动作做完是否重拉当前列表 */
  reload: boolean;
}

/**
 * 资源管理窗口各分类共用的操作
 *
 * - `data`: 数据层，只为拿 `instanceUuid` 与 `reloadCurrent`
 *
 * # 返回值
 *
 * - `busy`: 是否有操作在跑，列表上的按钮据此置灰
 * - `confirmBox`: 待确认的弹窗内容，`null` 表示不显示
 * - `confirmBusy`: 确认动作是否在跑
 * - `renameAsk`: 待重命名的目标，`null` 表示没在问
 * - `askConfirm`: 弹确认框，可指定做完是否重拉
 * - `askDelete`: 删除类确认，文案统一
 * - `askRename`: 问一个新名字
 * - `doRename`: 执行重命名并重拉列表
 * - `runConfirm`: 执行确认框里的动作
 * - `act`: 串行化 + 失败提示 + 成功后重拉列表
 * - `actLocal`: 同上但不重拉，给调用方自己就地更新的操作用
 * - `openFolder`: 在文件管理器里打开某个分类目录
 */
export function useResourceOps(data: ReturnType<typeof useResourceData>) {
  const { instanceUuid, reloadCurrent } = data;

  /** 有操作在跑：列表上的按钮统一置灰，避免连点发两次 */
  const busy = ref(false);

  /** 待确认的弹窗，`null` 表示没有 */
  const confirmBox = ref<ConfirmBox | null>(null);
  /** 确认动作正在跑，防连点 */
  const confirmBusy = ref(false);

  /** 待重命名的目标，`null` 表示没在问 */
  const renameAsk = ref<{ kind: string; parent: string; name: string } | null>(null);

  /**
   * 弹一个确认框，确认后执行 `run`
   *
   * - `title`: 弹窗标题
   * - `text`: 弹窗正文
   * - `run`: 确认后要做的动作
   * - `opts.reload`: 做完是否重拉当前列表，缺省 `true`；分组这类**不在列表里**的数据传 `false`，重拉模组要重解析 jar
   */
  function askConfirm(
    title: string,
    text: string,
    run: () => Promise<void>,
    opts?: { reload?: boolean },
  ) {
    confirmBox.value = { title, text, run, reload: opts?.reload ?? true };
  }

  /**
   * 删除类确认，文案统一
   *
   * - `name`: 要删的东西的显示名，进确认文案
   * - `run`: 确认后执行的动作
   */
  function askDelete(name: string, run: () => Promise<void>) {
    askConfirm(t("resource.delete"), t("resource.deleteConfirm", { name }), run);
  }

  /** 执行确认框里的动作，按 `reload` 决定是否重拉列表 */
  async function runConfirm() {
    const box = confirmBox.value;
    if (!box || confirmBusy.value) return;
    confirmBusy.value = true;
    try {
      await box.run();
      confirmBox.value = null;
      // 动作改的是磁盘上的列表：不重拉的话，删掉的那一行还留在界面上。
      // 分组数据不在列表里，askConfirm 会明确传 reload: false
      if (box.reload) await reloadCurrent();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      confirmBusy.value = false;
    }
  }

  /**
   * 问一句新名字
   *
   * - `kind`: 资源类别，取值与 `resource_open_folder` 一致
   * - `parent`: 数据包的存档目录名，其余类别传空串
   * - `name`: 现在的名字
   */
  function askRename(kind: string, name: string, parent = "") {
    renameAsk.value = { kind, parent, name };
  }

  /**
   * 确认重命名：改名 + 重拉当前列表
   *
   * - `newName`: 用户填的新名字；与旧名字相同或为空时当作取消
   */
  async function doRename(newName: string) {
    const ask = renameAsk.value;
    renameAsk.value = null;
    if (!ask) return;
    const name = newName.trim();
    if (!name || name === ask.name) return;
    await act(() => renameResource(instanceUuid.value, ask.kind, ask.parent, ask.name, name));
  }

  /**
   * 轻操作（备份 / 切换光影…）：串行化 + 失败提示 + 成功后重拉
   *
   * - `run`: 要执行的动作
   */
  async function act(run: () => Promise<void>) {
    if (busy.value) return;
    busy.value = true;
    try {
      await run();
      await reloadCurrent();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      busy.value = false;
    }
  }

  /**
   * 只做"串行化 + 失败提示"，**不重拉列表**
   *
   * 给那些"改完之后调用方自己就地更新"的操作用：打开目录只读，启用 / 禁用模组
   * 只改文件名、元数据一个字没变，调用方拿后端回的新身份改那一行即可。
   *
   * 重拉一次模组要把每个 jar 的元数据重新解析一遍（几百个包要好几秒），
   * 这两种操作都不需要它。
   *
   * - `run`: 要执行的动作
   */
  async function actLocal(run: () => Promise<void>) {
    if (busy.value) return;
    busy.value = true;
    try {
      await run();
    } catch (e) {
      showToast(tErr(e));
    } finally {
      busy.value = false;
    }
  }

  /**
   * 在文件管理器里打开目录
   *
   * **不重拉列表**：这是只读操作，列表不会变；而重拉一次模组要重新解析每个 jar 的元数据
   * （数秒），原先它和增删走同一条 act()，点一下"打开文件夹"整个列表就白转一圈。
   *
   * - `kind`: 后端认的目录名，见 types.ts 的 `CATEGORY_KIND`
   * - `name`: 具体条目名；`null` 表示打开该分类目录本身
   * - `parent`: 只有数据包子页要传，即存档目录名，落点 `saves/<parent>/datapacks`
   */
  async function openFolder(kind: string, name: string | null, parent: string | null = null) {
    await actLocal(() => openResourceFolder(instanceUuid.value, kind, name, parent));
  }

  return {
    busy,
    confirmBox,
    confirmBusy,
    renameAsk,
    askConfirm,
    askDelete,
    askRename,
    doRename,
    runConfirm,
    act,
    actLocal,
    openFolder,
  };
}
