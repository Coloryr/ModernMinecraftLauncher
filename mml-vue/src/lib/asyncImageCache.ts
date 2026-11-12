// AsyncImage 的跨实例缓存
//
// 方块网格切分类会整块重建，<script setup> 里的 Set 每个实例一份，
// 重建后认不出"这张图已经加载过"，就会再闪一次流光。
export const loadedAsyncImages = new Set<string>();
