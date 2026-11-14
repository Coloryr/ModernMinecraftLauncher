<script setup lang="ts">
// 账户占用确认：启动时发现当前账户正被其它实例使用，问继续还是换一个账户
//
// 常驻在 App 里，不随某个窗口卸载；「继续检测账户占用」关闭时才写配置，
// 取消启动不改变检查开关。
import { computed, ref, watch } from "vue";
import BaseModal from "./ui/BaseModal.vue";
import BaseButton from "./ui/BaseButton.vue";
import BaseSwitch from "./ui/BaseSwitch.vue";
import { t } from "../lib/i18n";
import { accountLockOpen, answerAccountLock } from "../lib/accountLock";
import { accounts, currentAccount, loadAccounts, typeLabelKey } from "../lib/accountStore";
import type { AccountStoreDto } from "../lib/bindings";

/** 选中的做法：继续用当前账户 / 换一个账户 */
const action = ref<"continue" | "switch">("continue");
/** 换账户时选中的账户 uuid */
const uuid = ref("");
/** 是否继续检测账户占用；每次打开重置为开 */
const keep = ref(true);

/** 当前账户之外的其它账户 */
const others = computed(() => accounts.value.filter((acc) => acc.uuid !== currentAccount.value?.uuid));
/** 有没有可换的账户；只有一个账户时第二项不可选 */
const canSwitch = computed(() => others.value.length > 0);

// 每次打开都重拉一次账户：另一个窗口刚加 / 删过账户时，列表不能停在旧快照
watch(accountLockOpen, (open) => {
  if (!open) return;
  keep.value = true;
  action.value = "continue";
  uuid.value = others.value[0]?.uuid ?? "";
  void loadAccounts();
});

// 列表晚到或被同步改掉时，保证下拉里选着一个仍然存在的账户
watch(others, (list) => {
  if (!accountLockOpen.value) return;
  if (!list.some((acc) => acc.uuid === uuid.value)) uuid.value = list[0]?.uuid ?? "";
});

/**
 * 账户在下拉里怎么显示
 *
 * - `acc`: 账户条目
 *
 * # 返回值
 *
 * 名字加账户类型，如 `Steve · 微软`
 */
function accountLabel(acc: AccountStoreDto): string {
  return `${acc.userName} · ${t(typeLabelKey(acc.authType))}`;
}

/**
 * 关闭弹窗并回答启动流程
 *
 * - `pick`: 继续用当前账户、换账户或取消
 */
function answer(pick: "continue" | "switch" | "cancel") {
  if (pick === "switch" && !uuid.value) return;
  answerAccountLock({
    action: pick,
    uuid: pick === "switch" ? uuid.value : "",
    keep: keep.value,
  });
}
</script>

<template>
  <BaseModal v-if="accountLockOpen" :title="t('accountLock.title')" :closable="false" @close="answer('cancel')">
    <p class="modal-text">{{ t("accountLock.desc") }}</p>
    <div class="lock-choices">
      <label class="lock-choice">
        <input v-model="action" type="radio" value="continue" class="lock-radio" />
        <span>{{ t("accountLock.continue") }}</span>
      </label>
      <div class="lock-choice-row">
        <label class="lock-choice" :class="{ 'is-disabled': !canSwitch }">
          <input v-model="action" type="radio" value="switch" class="lock-radio" :disabled="!canSwitch" />
          <span>{{ t("accountLock.switchAccount") }}</span>
        </label>
        <select v-model="uuid" class="field-select lock-select" :disabled="!canSwitch || action !== 'switch'">
          <option v-for="acc in others" :key="acc.uuid" :value="acc.uuid">{{ accountLabel(acc) }}</option>
        </select>
      </div>
    </div>
    <label class="lock-keep">
      <BaseSwitch v-model="keep" />
      <span>{{ t("accountLock.keep") }}</span>
    </label>
    <div class="modal-actions">
      <BaseButton @click="answer('cancel')">{{ t("accountLock.cancel") }}</BaseButton>
      <BaseButton variant="primary" :disabled="action === 'switch' && !uuid" @click="answer(action)">
        {{ t("accountLock.start") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 两个单选竖排；换账户那一项的下拉跟在文字后面占满剩余宽度 */
.lock-choices {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 14px 0;
}

.lock-choice {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text);
  cursor: pointer;
}

.lock-choice.is-disabled {
  color: var(--text-dim);
  cursor: default;
}

.lock-choice-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.lock-choice-row .lock-choice {
  flex-shrink: 0;
}

.lock-select {
  flex: 1;
  min-width: 0;
}

.lock-radio {
  margin: 0;
  accent-color: var(--accent);
  flex-shrink: 0;
}

.lock-keep {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 12px 0;
  color: var(--text);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
