// 预览页里的元素检查：由编辑器注入，编辑器通过 postMessage 驱动。
//
// 元素树的展开、选中、样式读取与行内样式编辑都在预览页这一侧完成；
// 编辑器只负责显示与回传操作（两边跨源，不能直接读 iframe 的 DOM）。
export const INSPECTOR_SCRIPT = `(() => {
  if (window.__mmlInspector) return;
  window.__mmlInspector = 1;
  var PROTOCOL = 1;
  var enabled = false;
  var box = null;
  var label = null;
  var style = null;
  var hovered = null;
  var selected = null;

  var ROOT_SELECTOR = typeof window.__mmlInspectRoot === 'string' ? window.__mmlInspectRoot : '';

  // 检查范围可以限定在某个容器里：自定义主页面只有内容区可改，元素树不从 html 开始
  function root() {
    if (!ROOT_SELECTOR) return document.documentElement;
    return document.querySelector(ROOT_SELECTOR) || document.body;
  }

  var CSS =
    '.__mml-inspect-on, .__mml-inspect-on * { cursor: crosshair !important; }' +
    '.__mml-inspect-box { position: fixed; z-index: 2147483646; pointer-events: none; box-sizing: border-box;' +
    ' border: 1px solid #3ddc97; background: rgba(61, 220, 151, 0.14); }' +
    '.__mml-inspect-label { position: fixed; z-index: 2147483647; pointer-events: none; max-width: 70vw;' +
    ' overflow: hidden; padding: 2px 6px; border-radius: 4px; background: #0b1220; color: #d7e2f2;' +
    ' font: 10px/1.5 Consolas, monospace; white-space: nowrap; text-overflow: ellipsis; }';

  function send(data) {
    parent.postMessage(Object.assign({ __mmlInspector: PROTOCOL }, data), '*');
  }

  function ensureUi() {
    if (!style) {
      style = document.createElement('style');
      style.textContent = CSS;
      (document.head || document.documentElement).appendChild(style);
    }
    if (!box) {
      box = document.createElement('div');
      box.className = '__mml-inspect-box';
      box.style.display = 'none';
      document.body.appendChild(box);
    }
    if (!label) {
      label = document.createElement('div');
      label.className = '__mml-inspect-label';
      label.style.display = 'none';
      document.body.appendChild(label);
    }
  }

  function hide() {
    if (box) box.style.display = 'none';
    if (label) label.style.display = 'none';
  }

  // 高亮用的类挂在 <html> 上，树和面包屑里不能把它当成页面自己的类
  function classesOf(el) {
    return Array.prototype.filter.call(el.classList, function (name) {
      return name !== '__mml-inspect-on';
    });
  }

  function nameOf(el) {
    var name = el.tagName.toLowerCase();
    if (el.id) name += '#' + el.id;
    var classes = classesOf(el).slice(0, 3);
    if (classes.length) name += '.' + classes.join('.');
    return name;
  }

  function draw(el) {
    if (!el || !box) return;
    var rect = el.getBoundingClientRect();
    box.style.display = 'block';
    box.style.left = rect.left + 'px';
    box.style.top = rect.top + 'px';
    box.style.width = rect.width + 'px';
    box.style.height = rect.height + 'px';
    label.style.display = 'block';
    label.textContent = nameOf(el) + '  ' + Math.round(rect.width) + '×' + Math.round(rect.height);
    label.style.left = Math.max(0, rect.left) + 'px';
    label.style.top = Math.max(0, rect.top - 20) + 'px';
  }

  function pathOf(el) {
    var path = [];
    var top = root();
    var node = el;
    while (node && node !== top) {
      var parent = node.parentElement;
      if (!parent) return [];
      path.unshift(Array.prototype.indexOf.call(parent.children, node));
      node = parent;
    }
    return path;
  }

  function elementAt(path) {
    var node = root();
    for (var i = 0; i < path.length; i += 1) {
      node = node.children[path[i]];
      if (!node) return null;
    }
    return node;
  }

  function summary(el, path) {
    return {
      path: path,
      tag: el.tagName.toLowerCase(),
      id: el.id,
      classes: classesOf(el).slice(0, 4),
      childCount: el.children.length,
    };
  }

  // 整棵元素树一次发过去：主题页面的 DOM 不会大到需要懒加载，
  // 展开 / 收起纯在编辑器这一侧完成，不再依赖逐层请求
  var TREE_LIMIT = 800;

  function treeNode(el, path, budget) {
    var node = summary(el, path);
    node.children = [];
    for (var i = 0; i < el.children.length; i += 1) {
      var child = el.children[i];
      // 检查模式自己的高亮框与尺寸标签不算页面元素
      if (child.classList && (child.classList.contains('__mml-inspect-box') || child.classList.contains('__mml-inspect-label'))) continue;
      if (budget.count >= TREE_LIMIT) break;
      budget.count += 1;
      node.children.push(treeNode(child, path.concat([i]), budget));
    }
    return node;
  }

  function sendTree() {
    var nodes = [];
    var budget = { count: 0 };
    var top = root();
    // 默认根保持原样（html 的两子树）；限定范围时从容器本身开始，路径 [] 就是容器
    if (top === document.documentElement) {
      for (var i = 0; i < top.children.length; i += 1) {
        if (budget.count >= TREE_LIMIT) break;
        budget.count += 1;
        nodes.push(treeNode(top.children[i], [i], budget));
      }
    } else {
      nodes.push(treeNode(top, [], budget));
    }
    send({ type: 'tree', nodes: nodes });
  }

  function selectorOf(el) {
    // 只把规范的 id 当选择器；带空格等字符的 id 走结构路径，免得拼出非法选择器
    if (el.id && /^[A-Za-z_][A-Za-z0-9_-]*$/.test(el.id)) return '#' + el.id;
    var top = root();
    var parts = [];
    var node = el;
    while (node && node.nodeType === 1) {
      var classes = classesOf(node).filter(function (name) {
        return /^[A-Za-z_][A-Za-z0-9_-]*$/.test(name);
      }).slice(0, 2);
      var name = node.tagName.toLowerCase() + (classes.length ? '.' + classes.join('.') : '');
      var parent = node.parentElement;
      var same = parent ? Array.prototype.filter.call(parent.children, function (child) {
        return child.tagName === node.tagName;
      }) : [];
      // 类名已经能唯一确定这个兄弟时不再加 :nth-of-type，选择器短一点
      var unique = classes.length > 0 && same.filter(function (child) {
        return classes.every(function (name) { return child.classList.contains(name); });
      }).length === 1;
      var index = same.length > 1 && !unique ? ':nth-of-type(' + (same.indexOf(node) + 1) + ')' : '';
      parts.unshift(name + index);
      if (!parent || node === top) break;
      // 默认根是 html 时不把它写进选择器，与原来的 body > div 保持一致
      if (top === document.documentElement && parent === top) break;
      node = parent;
    }
    return parts.join(' > ');
  }

  function ancestorsOf(el) {
    var top = root();
    var list = [];
    var node = el;
    while (node && node.nodeType === 1) {
      list.unshift({
        tag: node.tagName.toLowerCase(),
        id: node.id,
        classes: classesOf(node).slice(0, 4),
        path: pathOf(node),
      });
      if (node === top) break;
      node = node.parentElement;
    }
    return list;
  }

  function declarationsOf(style) {
    var list = [];
    for (var i = 0; i < style.length && i < 80; i += 1) {
      var name = style[i];
      list.push({
        name: name,
        value: style.getPropertyValue(name),
        important: style.getPropertyPriority(name) === 'important',
      });
    }
    return list;
  }

  var STYLE_NAMES = [
    'display', 'position', 'width', 'height', 'margin', 'padding', 'border', 'border-radius',
    'color', 'background-color', 'font-size', 'font-weight', 'line-height', 'letter-spacing',
    'gap', 'flex-direction', 'justify-content', 'align-items', 'z-index', 'opacity',
  ];

  function stylesOf(el) {
    var computed = getComputedStyle(el);
    var list = [];
    for (var i = 0; i < STYLE_NAMES.length; i += 1) {
      var name = STYLE_NAMES[i];
      var value = computed.getPropertyValue(name);
      if (!value) continue;
      if (value === 'none' || value === 'normal' || value === 'auto' || value === '0px' || value === '1') continue;
      list.push({ name: name, value: value });
    }
    return list;
  }

  function variablesOf(el) {
    var computed = getComputedStyle(el);
    var list = [];
    for (var i = 0; i < computed.length; i += 1) {
      var name = computed[i];
      if (name.indexOf('--') !== 0) continue;
      var value = computed.getPropertyValue(name).trim();
      if (!value) continue;
      list.push({ name: name, value: value });
      if (list.length >= 80) break;
    }
    return list;
  }

  function walkRules(list, el, out) {
    for (var i = 0; i < list.length && out.length < 60; i += 1) {
      var rule = list[i];
      // 检查模式自己的高亮样式也算命中，别让它污染样式列表
      if (rule.selectorText && rule.selectorText.indexOf('__mml-inspect') >= 0) continue;
      if (rule.selectorText && rule.style) {
        var matched = false;
        try {
          matched = el.matches(rule.selectorText);
        } catch (error) {
          matched = false;
        }
        if (matched) out.push({ selector: rule.selectorText, declarations: declarationsOf(rule.style) });
      } else if (rule.cssRules && rule.cssRules.length) {
        var applies = true;
        if (rule.conditionText) {
          try {
            applies = matchMedia(rule.conditionText).matches;
          } catch (error) {
            applies = true;
          }
        }
        if (applies) walkRules(rule.cssRules, el, out);
      }
    }
  }

  function rulesOf(el) {
    var out = [];
    for (var s = 0; s < document.styleSheets.length && out.length < 60; s += 1) {
      var sheet = document.styleSheets[s];
      var list = null;
      try {
        list = sheet.cssRules;
      } catch (error) {
        list = null;
      }
      if (list) walkRules(list, el, out);
    }
    return out;
  }

  function nodeInfo(el) {
    var rect = el.getBoundingClientRect();
    return {
      path: pathOf(el),
      tag: el.tagName.toLowerCase(),
      id: el.id,
      classes: classesOf(el),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      selector: selectorOf(el),
      ancestors: ancestorsOf(el),
      inline: declarationsOf(el.style),
      rules: rulesOf(el),
      styles: stylesOf(el),
      variables: variablesOf(el),
    };
  }

  function select(el) {
    if (!el || !root().contains(el)) return;
    selected = el;
    draw(el);
    send({ type: 'pick', node: nodeInfo(el) });
  }

  function targetAt(x, y) {
    var el = document.elementFromPoint(x, y);
    if (!el || el === box || el === label || el === style) return null;
    // 容器外的启动器界面不归主题管，点它不选中任何元素
    if (!root().contains(el)) return null;
    return el;
  }

  function onMove(event) {
    var el = targetAt(event.clientX, event.clientY);
    if (!el) return;
    hovered = el;
    draw(el);
  }

  function onPress(event) {
    if (!enabled) return;
    var el = targetAt(event.clientX, event.clientY) || hovered;
    if (!el) return;
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
    select(el);
  }

  function onKey(event) {
    if (!enabled || event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    disable();
    send({ type: 'exit' });
  }

  function onViewportChange() {
    var el = hovered || selected;
    if (enabled && el) draw(el);
  }

  function enable() {
    if (enabled) return;
    enabled = true;
    ensureUi();
    root().classList.add('__mml-inspect-on');
    addEventListener('mousemove', onMove, true);
    addEventListener('mousedown', onPress, true);
    addEventListener('click', onPress, true);
    addEventListener('keydown', onKey, true);
    addEventListener('scroll', onViewportChange, true);
    addEventListener('resize', onViewportChange);
    hide();
  }

  function disable() {
    if (!enabled) return;
    enabled = false;
    hovered = null;
    selected = null;
    root().classList.remove('__mml-inspect-on');
    removeEventListener('mousemove', onMove, true);
    removeEventListener('mousedown', onPress, true);
    removeEventListener('click', onPress, true);
    removeEventListener('keydown', onKey, true);
    removeEventListener('scroll', onViewportChange, true);
    removeEventListener('resize', onViewportChange);
    hide();
  }

  addEventListener('message', function (event) {
    if (event.source !== parent) return;
    var data = event.data;
    if (!data || data.__mmlInspector !== PROTOCOL) return;
    if (data.type === 'enable') {
      enable();
    } else if (data.type === 'disable') {
      disable();
    } else if (data.type === 'tree') {
      sendTree();
    } else if (data.type === 'select') {
      var el = elementAt(data.path || []);
      if (el) select(el);
    } else if (data.type === 'hover') {
      var hover = data.path ? elementAt(data.path) : null;
      hovered = hover;
      if (hover) draw(hover);
      else if (selected) draw(selected);
      else hide();
    } else if (data.type === 'style') {
      var target = elementAt(data.path || []);
      if (target && data.name) {
        try {
          target.style.setProperty(String(data.name), String(data.value), data.important ? 'important' : '');
        } catch (error) {
          /* 非法值直接忽略，面板会保留输入 */
        }
        send({ type: 'inline', path: data.path, inline: declarationsOf(target.style) });
      }
    } else if (data.type === 'unstyle') {
      var removed = elementAt(data.path || []);
      if (removed && data.name) {
        removed.style.removeProperty(String(data.name));
        send({ type: 'inline', path: data.path, inline: declarationsOf(removed.style) });
      }
    }
  });

  // F12 / Ctrl+Shift+I 在预览里只切换检查模式，不打开工具窗口的开发者工具
  addEventListener('keydown', function (event) {
    var shortcut = (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'i';
    if (event.key !== 'F12' && !shortcut) return;
    event.preventDefault();
    event.stopPropagation();
    send({ type: 'toggle' });
  }, true);
})();`;
