import { ref, shallowRef } from "vue";

type CloseChoice = "cancel" | "discard" | "save";

interface ConfigEditor {
  busy(): boolean;
  hasChanges(): boolean;
  save(): Promise<boolean>;
}

let editor: ConfigEditor | null = null;
export const configClosePrompt = shallowRef<((choice: CloseChoice) => void) | null>(null);
export const savingOnClose = ref(false);

export function registerConfigEditor(current: ConfigEditor): () => void {
  editor = current;
  return () => {
    if (editor !== current) return;
    editor = null;
    configClosePrompt.value?.("cancel");
  };
}

export async function allowConfigClose(): Promise<boolean> {
  const current = editor;
  if (!current) return true;
  if (configClosePrompt.value || current.busy()) return false;
  if (!current.hasChanges()) return true;

  try {
    const choice = await new Promise<CloseChoice>((resolve) => { configClosePrompt.value = resolve; });
    if (choice === "cancel" || editor !== current || current.busy()) return false;
    if (choice === "discard") return true;
    savingOnClose.value = true;
    return await current.save() && editor === current && !current.hasChanges();
  } finally {
    configClosePrompt.value = null;
    savingOnClose.value = false;
  }
}
