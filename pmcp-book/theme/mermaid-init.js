;(function(){
  function ensureDivContainers() {
    var pres = document.querySelectorAll('pre.mermaid');
    pres.forEach(function(pre){
      var div = document.createElement('div');
      div.className = 'mermaid';
      // Use textContent to get decoded text (handles &gt; etc.)
      div.textContent = pre.textContent;
      pre.parentNode.replaceChild(div, pre);
    });
  }

  function currentTheme() {
    var root = document.documentElement;
    var isDark = root.classList.contains('ayu');
    return isDark ? 'dark' : 'neutral';
  }

  function initMermaid() {
    if (!window.mermaid) return;
    try {
      window.mermaid.initialize({ startOnLoad: false, theme: currentTheme() });
      window.mermaid.init(undefined, document.querySelectorAll('div.mermaid'));
    } catch (e) {
      console.error('Mermaid init failed', e);
    }
  }

  function onReady(fn){
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', fn, { once: true });
    } else { fn(); }
  }

  onReady(function(){
    ensureDivContainers();
    // Delay a tick in case mdBook injects content late
    setTimeout(initMermaid, 0);
  });

  // Re-render on theme toggle (mdBook toggles class on <html>)
  var mo = new MutationObserver(function(muts){
    for (var m of muts) {
      if (m.attributeName === 'class') {
        initMermaid();
        break;
      }
    }
  });
  mo.observe(document.documentElement, { attributes: true });
})();

