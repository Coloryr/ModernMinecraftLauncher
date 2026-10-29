// 项目与文件的「已下载 / 下载中」角标判定
//
// 整合包窗口、资源窗口与共用的项目详情页都用这一份：三处判定口径必须一致，
// 各写一遍的话改一处忘一处，同一个文件在两个界面上一个带角标一个不带。
//
// 「已下载 / 已安装」取两个来源的「或」：
// - 实例侧快照，即列表 DTO 里的 `item.download` / `file.isDownload`，是取列表那一刻的状态；
// - 本次会话的任务表。
// 任务条目会被清掉（用户点「清除已完成」），但文件确实已经装好了 ——
// 所以见过的「完成」只往集合里加，不做任何复位。
//
// 「下载中」只看任务表：DTO 里那份同样是快照，用它角标会一直挂着不复位。
import { computed, ref, watch, type Ref } from "vue";
import type { FileListItemDto, ProjectItemDto } from "./bindings";

/** 任务条目的最小契约，整合包与资源两种任务 DTO 都满足 */
type TaskItem = {
  pid: string;
  fid: string;
  done: boolean;
  failed: boolean;
  cancelled?: boolean;
};

/** 任务快照的最小契约，`ModPackStatusDto` / `ResourceStatusDto` 都满足 */
interface TaskSnapshot {
  tasks: TaskItem[];
}

/**
 * 挂上角标判定，须在 setup 里调用
 *
 * - `status`: 本窗口的任务快照；传 null 表示还没有拉到任务快照
 *
 * # 返回值
 *
 * - `projectInstalled`: 项目是否已装，判据见文件头
 * - `projectRunning`: 项目是否有文件正在下载
 * - `fileInstalled`: 版本文件是否已装
 * - `fileRunning`: 版本文件是否正在下载
 */
export function useDownloadBadges<T extends TaskSnapshot>(status: Ref<T | null>) {
  /** 本次会话里见过的「完成」项目，只增不减 */
  const donePids = ref(new Set<string>());
  /** 同上，键是 `pid|fid` */
  const doneFileKeys = ref(new Set<string>());

  watch(
    status,
    (value) => {
      const tasks = value?.tasks ?? [];
      if (!tasks.some((task) => task.done)) return;
      const pids = new Set(donePids.value);
      const files = new Set(doneFileKeys.value);
      for (const task of tasks) {
        if (!task.done) continue;
        pids.add(task.pid);
        files.add(`${task.pid}|${task.fid}`);
      }
      // 只在真新增时换引用：进度事件很频繁，无谓的赋值会带着模板一起重渲染
      if (pids.size !== donePids.value.size) donePids.value = pids;
      if (files.size !== doneFileKeys.value.size) doneFileKeys.value = files;
    },
    { immediate: true },
  );

  /** 进行中任务派生的角标状态，由任务快照直接派生，不往列表项里写回 */
  const running = computed(() => {
    const tasks: TaskItem[] = status.value?.tasks ?? [];
    const isRunning = (task: TaskItem) => !task.done && !task.failed && !task.cancelled;
    return {
      projects: new Set(tasks.filter(isRunning).map((task) => task.pid)),
      files: new Set(tasks.filter(isRunning).map((task) => `${task.pid}|${task.fid}`)),
    };
  });

  function projectInstalled(item: ProjectItemDto): boolean {
    return item.download || donePids.value.has(item.source.pid);
  }

  function projectRunning(item: ProjectItemDto): boolean {
    return running.value.projects.has(item.source.pid);
  }

  function fileKey(file: FileListItemDto): string {
    return `${file.source.pid}|${file.source.fid}`;
  }

  function fileInstalled(file: FileListItemDto): boolean {
    return file.isDownload || doneFileKeys.value.has(fileKey(file));
  }

  function fileRunning(file: FileListItemDto): boolean {
    return running.value.files.has(fileKey(file));
  }

  return { projectInstalled, projectRunning, fileInstalled, fileRunning };
}
