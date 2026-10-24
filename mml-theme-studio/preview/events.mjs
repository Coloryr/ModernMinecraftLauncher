// 预览事件只在当前页面分发，不连接启动器或其他项目。
const listeners = new Map();
let nextId = 1;

export async function listen(event, handler, options = {}) {
  const id = nextId++;
  listeners.set(id, { event, handler, target: options.target });
  return () => listeners.delete(id);
}

export async function once(event, handler, options) {
  let remove;
  remove = await listen(event, (value) => { remove(); handler(value); }, options);
  return remove;
}

function dispatch(event, payload, target) {
  for (const [id, listener] of [...listeners]) {
    const label = typeof listener.target === 'string' ? listener.target : listener.target?.label;
    if (listener.event === event && (!target || !label || label === target)) listener.handler({ event, id, payload });
  }
}

export async function emit(event, payload) { dispatch(event, payload); }
export async function emitTo(target, event, payload) { dispatch(event, payload, typeof target === 'string' ? target : target.label); }
