// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Returns a self-contained JavaScript runtime script for interactive Vell documents.
 * The script is a string that can be embedded in HTML output.
 */
export function getRuntimeScript(): string {
  return `(function() {
  'use strict';

  // -----------------------------------------------------------------------
  // Reactive variable store
  // -----------------------------------------------------------------------
  var store = {};
  var subscribers = {};
  var computedFns = {};

  function getVar(name) {
    return store[name];
  }

  function setVar(name, value) {
    if (store[name] === value) return;
    store[name] = value;
    notify(name, value);
    // Re-evaluate any computed variables that depend on this one
    for (var key in computedFns) {
      if (computedFns[key].deps.indexOf(name) !== -1) {
        var newVal = computedFns[key].fn();
        if (store[key] !== newVal) {
          store[key] = newVal;
          notify(key, newVal);
        }
      }
    }
  }

  function subscribe(name, fn) {
    if (!subscribers[name]) subscribers[name] = [];
    subscribers[name].push(fn);
    return function() {
      var idx = subscribers[name].indexOf(fn);
      if (idx !== -1) subscribers[name].splice(idx, 1);
    };
  }

  function notify(name, value) {
    var subs = subscribers[name];
    if (subs) {
      for (var i = 0; i < subs.length; i++) {
        subs[i](value);
      }
    }
  }

  // -----------------------------------------------------------------------
  // Computed variable: @var total = @{price} * @{quantity}
  // -----------------------------------------------------------------------
  function defineComputed(name, deps, fn) {
    computedFns[name] = { deps: deps, fn: fn };
    store[name] = fn();
    // Subscribe to all deps and re-evaluate on change
    for (var i = 0; i < deps.length; i++) {
      subscribe(deps[i], function() {
        var newVal = fn();
        if (store[name] !== newVal) {
          store[name] = newVal;
          notify(name, newVal);
        }
      });
    }
  }

  // -----------------------------------------------------------------------
  // DOM binding
  // -----------------------------------------------------------------------

  /** Update all elements with data-vell-var="${name}" */
  function updateVarDisplays(name, value) {
    var els = document.querySelectorAll('[data-vell-var="' + name + '"]');
    for (var i = 0; i < els.length; i++) {
      els[i].textContent = formatValue(value);
    }
  }

  function formatValue(v) {
    if (v === null || v === undefined) return '';
    if (Array.isArray(v)) return JSON.stringify(v);
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    return String(v);
  }

  /** Bind a slider input to a variable */
  function bindSlider(input, varName) {
    input.addEventListener('input', function() {
      setVar(varName, parseFloat(input.value));
    });
    // Update input when variable changes
    subscribe(varName, function(val) {
      var num = typeof val === 'number' ? val : parseFloat(val);
      if (!isNaN(num) && input.value !== String(num)) {
        input.value = String(num);
      }
    });
    // Initialize
    if (store[varName] !== undefined) {
      input.value = String(store[varName]);
    } else {
      setVar(varName, parseFloat(input.value));
    }
  }

  /** Bind a text input to a variable */
  function bindTextInput(input, varName) {
    input.addEventListener('input', function() {
      setVar(varName, input.value);
    });
    subscribe(varName, function(val) {
      var str = val == null ? '' : String(val);
      if (input.value !== str) input.value = str;
    });
    if (store[varName] !== undefined) {
      input.value = String(store[varName]);
    } else {
      setVar(varName, input.value);
    }
  }

  /** Bind a checkbox to a variable */
  function bindCheckbox(input, varName) {
    input.addEventListener('change', function() {
      setVar(varName, input.checked);
    });
    subscribe(varName, function(val) {
      input.checked = !!val;
    });
    if (store[varName] !== undefined) {
      input.checked = !!store[varName];
    } else {
      setVar(varName, input.checked);
    }
  }

  /** Bind a select/dropdown to a variable */
  function bindSelect(select, varName) {
    select.addEventListener('change', function() {
      setVar(varName, select.value);
    });
    subscribe(varName, function(val) {
      var str = val == null ? '' : String(val);
      if (select.value !== str) select.value = str;
    });
    if (store[varName] !== undefined) {
      select.value = String(store[varName]);
    } else {
      setVar(varName, select.value);
    }
  }

  // -----------------------------------------------------------------------
  // For/If block handling (runtime evaluation)
  // -----------------------------------------------------------------------

  /** Render a for-loop block by cloning the template for each item */
  function renderForBlock(container) {
    var variable = container.getAttribute('data-variable');
    var iterableName = container.getAttribute('data-iterable');
    var template = container.querySelector('[data-vell-template]');
    if (!template) return;
    var parent = template.parentNode;
    var items = store[iterableName];
    if (!Array.isArray(items)) return;

    // Store original template
    var originalHTML = template.innerHTML;

    function renderItems() {
      var items = store[iterableName];
      if (!Array.isArray(items)) return;
      // Remove all rendered items (keep template)
      while (parent.lastChild && parent.lastChild !== template) {
        parent.removeChild(parent.lastChild);
      }
      // Render each item
      for (var i = 0; i < items.length; i++) {
        var clone = template.cloneNode(true);
        clone.removeAttribute('data-vell-template');
        clone.innerHTML = originalHTML.replace(new RegExp('@\\\\{' + variable + '\\\\}', 'g'), formatValue(items[i]));
        parent.appendChild(clone);
      }
    }

    subscribe(iterableName, renderItems);
    renderItems();
  }

  /** Evaluate an if-block condition and show/hide content */
  function evalCondition(condition, vars) {
    // Simple expression evaluator for conditions like "@{count} > 5"
    var expr = condition;
    // Replace @{varName} with actual values
    for (var key in store) {
      expr = expr.replace(new RegExp('@\\\\{' + key + '\\\\}', 'g'), String(store[key]));
    }
    try {
      return !!eval(expr);
    } catch(e) {
      return false;
    }
  }

  function renderIfBlock(container) {
    var condition = container.getAttribute('data-condition');
    var consequent = container.querySelector('[data-vell-then]');
    var alternate = container.querySelector('[data-vell-else]');

    function updateVisibility() {
      var result = evalCondition(condition);
      if (consequent) consequent.style.display = result ? '' : 'none';
      if (alternate) alternate.style.display = result ? 'none' : '';
    }

    // Subscribe to all variables mentioned in condition
    var refs = condition.match(/@\\{[a-zA-Z_][a-zA-Z0-9_]*\\}/g) || [];
    for (var i = 0; i < refs.length; i++) {
      var varName = refs[i].slice(2, -1);
      subscribe(varName, updateVisibility);
    }
    updateVisibility();
  }

  // -----------------------------------------------------------------------
  // Initialization
  // -----------------------------------------------------------------------

  function init() {
    // 1. Load initial variables from data-vell-init attributes
    var initEls = document.querySelectorAll('[data-vell-init]');
    for (var i = 0; i < initEls.length; i++) {
      try {
        var data = JSON.parse(initEls[i].getAttribute('data-vell-init'));
        for (var key in data) {
          store[key] = data[key];
        }
      } catch(e) {}
    }

    // 2. Load data from data-vell-data attributes (Data directive)
    var dataEls = document.querySelectorAll('[data-vell-data]');
    for (var i = 0; i < dataEls.length; i++) {
      try {
        var data = JSON.parse(dataEls[i].getAttribute('data-vell-data'));
        for (var key in data) {
          store[key] = data[key];
        }
      } catch(e) {}
    }

    // 3. Set up computed variables from data-vell-computed
    var compEls = document.querySelectorAll('[data-vell-computed]');
    for (var i = 0; i < compEls.length; i++) {
      var el = compEls[i];
      var name = el.getAttribute('data-vell-computed');
      var expr = el.getAttribute('data-expr');
      if (name && expr) {
        // Find variable references in expression
        var refs = expr.match(/@\\{[a-zA-Z_][a-zA-Z0-9_]*\\}/g) || [];
        var deps = refs.map(function(r) { return r.slice(2, -1); });
        defineComputed(name, deps, new Function(deps.join(','), 'return ' + expr.replace(/@\\{[a-zA-Z_][a-zA-Z0-9_]*\\}/g, function(m) {
          return 'arguments[' + deps.indexOf(m.slice(2, -1)) + ']';
        })));
      }
    }

    // 4. Set up variable display elements
    var varEls = document.querySelectorAll('[data-vell-var]');
    for (var i = 0; i < varEls.length; i++) {
      var name = varEls[i].getAttribute('data-vell-var');
      if (name) {
        subscribe(name, (function(n) {
          return function(v) { updateVarDisplays(n, v); };
        })(name));
      }
    }

    // 5. Bind sliders
    var sliders = document.querySelectorAll('input[type="range"][data-bind]');
    for (var i = 0; i < sliders.length; i++) {
      bindSlider(sliders[i], sliders[i].getAttribute('data-bind'));
    }

    // 6. Bind text inputs
    var textInputs = document.querySelectorAll('input[type="text"][data-bind], input[type="number"][data-bind]');
    for (var i = 0; i < textInputs.length; i++) {
      bindTextInput(textInputs[i], textInputs[i].getAttribute('data-bind'));
    }

    // 7. Bind checkboxes
    var checkboxes = document.querySelectorAll('input[type="checkbox"][data-bind]');
    for (var i = 0; i < checkboxes.length; i++) {
      bindCheckbox(checkboxes[i], checkboxes[i].getAttribute('data-bind'));
    }

    // 8. Bind selects
    var selects = document.querySelectorAll('select[data-bind]');
    for (var i = 0; i < selects.length; i++) {
      bindSelect(selects[i], selects[i].getAttribute('data-bind'));
    }

    // 9. Set up for-loop blocks
    var forBlocks = document.querySelectorAll('.vell-for');
    for (var i = 0; i < forBlocks.length; i++) {
      renderForBlock(forBlocks[i]);
    }

    // 10. Set up if-blocks
    var ifBlocks = document.querySelectorAll('.vell-if');
    for (var i = 0; i < ifBlocks.length; i++) {
      renderIfBlock(ifBlocks[i]);
    }

    // 11. Initial update of all variable displays
    for (var name in store) {
      updateVarDisplays(name, store[name]);
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();`;
}
