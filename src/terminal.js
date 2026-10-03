// Gömülü terminal sekmeleri: oturumu `claude --resume` ile SessionBoard içinde sürdürür.
// Sözde terminal (ConPTY) Rust tarafındadır (src-tauri/src/pty.rs); burada xterm.js + sekmeler +
// ağaç ile terminal arasındaki bölücü. app.js bunu `SessionTerminals.init(...)` ile bağlar.
window.SessionTerminals = (() => {
  'use strict';

  const { invoke, Channel } = window.__TAURI__.core;
  const MIN_WINDOW_WIDTH = 1180;
  const INSTALL_CMD = 'irm https://claude.ai/install.ps1 | iex';

  const tabsEl = document.getElementById('term-tabs');
  const bodyEl = document.getElementById('term-body');
  const splitterEl = document.getElementById('splitter');

  /** @type {Array<Tab>} */
  const tabs = [];
  let active = null;
  let hooks = { onChange() {}, confirm: async () => true, onSideWidth() {} };

  // Claude CLI koyu temaya göre renk seçer; terminal zemini bu yüzden her temada koyudur.
  const THEME = {
    background: '#1f1e1d',
    foreground: '#e8e6dc',
    cursor: '#d97757',
    cursorAccent: '#1f1e1d',
    selectionBackground: 'rgba(217, 119, 87, 0.35)',
    black: '#1f1e1d', red: '#e5604d', green: '#7fb069', yellow: '#e0b052',
    blue: '#6b9bd1', magenta: '#c58fd0', cyan: '#5fb3b3', white: '#d6d3c9',
    brightBlack: '#77756e', brightRed: '#f07a68', brightGreen: '#9ccc85', brightYellow: '#f0c977',
    brightBlue: '#8fb5e3', brightMagenta: '#d9a9e3', brightCyan: '#82cccc', brightWhite: '#f5f3ec',
  };

  function init(options) {
    hooks = { ...hooks, ...options };
    if (options.sideWidth) setSideWidth(options.sideWidth);
    initSplitter();
    new ResizeObserver(() => scheduleFit()).observe(bodyEl);
  }

  // ---------------------------------------------------------------- sekmeler

  /**
   * Oturumu terminalde açar. Aynı oturumun açık sekmesi varsa ona geçer (bittiyse yeniden başlatır).
   * `fork`: konuşmanın kopyasıyla yeni oturum (asıl oturum değişmez); her seferinde yeni sekme.
   */
  async function open(session, { fork = false } = {}) {
    let tab = fork ? null : tabs.find((t) => t.sessionId === session.id && !t.fork);
    if (tab) {
      activate(tab);
      if (tab.status === 'exited') await start(tab);
      return;
    }
    tab = createTab(session, fork);
    tabs.push(tab);
    setVisible(true);
    activate(tab);
    await start(tab);
  }

  function createTab(session, fork) {
    const el = document.createElement('div');
    el.className = 'term-pane';
    bodyEl.append(el);

    const term = new window.Terminal({
      fontFamily: '"Cascadia Mono", "Cascadia Code", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.15,
      cursorBlink: true,
      scrollback: 5000,
      theme: THEME,
      windowsPty: { backend: 'conpty' },
    });
    const fit = new window.FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(el);

    const tabEl = document.createElement('div');
    tabEl.className = 'term-tab';
    tabEl.setAttribute('role', 'tab');
    const dot = document.createElement('span');
    dot.className = 'term-dot';
    const label = document.createElement('span');
    label.className = 'term-label';
    label.textContent = (fork ? '⑂ ' : '') + (session.title || '(başlıksız)');
    const close = document.createElement('button');
    close.className = 'term-close';
    close.title = 'Kapat';
    close.innerHTML = '<svg viewBox="0 0 16 16"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>';
    tabEl.append(dot, label, close);
    tabsEl.append(tabEl);

    const tab = {
      sessionId: session.id,
      resumeId: session.cliSessionId,
      cwd: session.cwd,
      permissionMode: session.permissionMode,
      title: session.title || '(başlıksız)',
      fork,
      term,
      fit,
      el,
      tabEl,
      ptyId: null,
      status: 'starting',
      pendingInput: [],
    };
    tabEl.title = `${tab.title}\n${tab.cwd}${fork ? '\nKopya (fork): asıl oturum değişmez' : ''}`;

    tabEl.addEventListener('mousedown', (e) => {
      if (e.button === 1) e.preventDefault(); // orta tık: otomatik kaydırmayı engelle
    });
    tabEl.addEventListener('click', (e) => {
      if (e.target.closest('.term-close')) closeTab(tab);
      else activate(tab);
    });
    tabEl.addEventListener('auxclick', (e) => {
      if (e.button === 1) closeTab(tab);
    });

    term.onData((data) => {
      if (tab.ptyId != null) invoke('pty_write', { id: tab.ptyId, data }).catch(() => {});
      // ConPTY açılışta imleç konumunu sorar (ESC[6n) ve yanıt gelene kadar süreci bekletir; soru
      // pty_open yanıtından ÖNCE gelebilir. xterm'in yanıtı (ve erken tuşlar) kimlik gelene kadar tutulur.
      else if (tab.status === 'starting') tab.pendingInput.push(data);
    });
    term.onResize(({ cols, rows }) => {
      if (tab.ptyId != null) invoke('pty_resize', { id: tab.ptyId, cols, rows }).catch(() => {});
    });
    term.attachCustomKeyEventHandler((e) => handleKey(tab, e));
    el.addEventListener('contextmenu', (e) => {
      // Windows Terminal gibi: seçim varsa sağ tık kopyalar.
      e.preventDefault();
      if (term.hasSelection()) copySelection(tab);
    });
    return tab;
  }

  /** Kopyala/yapıştır kısayolları. Ctrl+C seçim yokken Claude'a gider (kesme). */
  function handleKey(tab, e) {
    if (e.type !== 'keydown') return true;
    const key = e.key.toLowerCase();
    if (e.ctrlKey && !e.altKey && key === 'c' && (e.shiftKey || tab.term.hasSelection())) {
      e.preventDefault();
      copySelection(tab);
      return false;
    }
    if (e.ctrlKey && !e.altKey && key === 'v') {
      // Tarayıcının yerleşik yapıştırma olayı xterm'e ulaşır (köşeli yapıştırma modu dahil);
      // xterm'in tuşu ayrıca ^V olarak göndermesi engellenir.
      return false;
    }
    return true;
  }

  function copySelection(tab) {
    const text = tab.term.getSelection();
    if (text) navigator.clipboard.writeText(text).catch(() => {});
    tab.term.clearSelection();
  }

  async function start(tab) {
    tab.status = 'starting';
    tab.ptyId = null;
    tab.pendingInput = [];
    updateTab(tab);
    fitTab(tab);
    if (!tab.resumeId) {
      tab.term.write('\x1b[31mBu oturumun konuşma kaydı (cliSessionId) yok; terminalde açılamaz.\x1b[0m\r\n');
      tab.status = 'exited';
      updateTab(tab);
      return;
    }
    tab.term.write(`\x1b[2m${tab.fork ? 'Kopya oturum başlatılıyor' : 'Oturum sürdürülüyor'}: ${tab.cwd}\x1b[0m\r\n`);

    const channel = new Channel();
    channel.onmessage = (ev) => {
      if (ev.kind === 'data') {
        tab.term.write(ev.data);
      } else if (ev.kind === 'exit') {
        tab.status = 'exited';
        tab.ptyId = null;
        const code = ev.code == null ? '' : ` · çıkış kodu ${ev.code}`;
        tab.term.write(`\r\n\x1b[2m[Claude kapandı${code}. Yeniden başlatmak için oturuma tekrar tıkla.]\x1b[0m\r\n`);
        updateTab(tab);
        hooks.onChange();
      }
    };

    try {
      const id = await invoke('pty_open', {
        resumeId: tab.resumeId,
        cwd: tab.cwd,
        permissionMode: tab.permissionMode || null,
        fork: tab.fork,
        cols: tab.term.cols,
        rows: tab.term.rows,
        onEvent: channel,
      });
      if (tab.status === 'starting') {
        tab.ptyId = id;
        tab.status = 'running';
        const pending = tab.pendingInput.join('');
        tab.pendingInput = [];
        if (pending) invoke('pty_write', { id, data: pending }).catch(() => {});
      }
    } catch (err) {
      tab.status = 'exited';
      writeOpenError(tab, String(err));
    }
    updateTab(tab);
    hooks.onChange();
    if (active === tab) tab.term.focus();
  }

  function writeOpenError(tab, err) {
    const t = tab.term;
    if (err === 'CLI_NOT_FOUND') {
      t.write('\x1b[33mClaude CLI bulunamadı.\x1b[0m\r\n\r\n');
      t.write('Terminalde çalışmak için bağımsız Claude Code CLI gerekir (masaüstü uygulamanın\r\n');
      t.write('kendi kopyası kullanılamaz). Windows Terminal ya da PowerShell\'de bir kez çalıştır:\r\n\r\n');
      t.write(`    \x1b[1m${INSTALL_CMD}\x1b[0m\r\n\r\n`);
      t.write('Ardından \x1b[1mclaude\x1b[0m yazıp giriş yap, \x1b[1m/exit\x1b[0m ile çık.\r\n');
      t.write('Sonra bu oturuma tekrar tıkla.\r\n');
    } else {
      t.write(`\x1b[31mBaşlatılamadı: ${err}\x1b[0m\r\n`);
    }
  }

  function updateTab(tab) {
    tab.tabEl.classList.toggle('active', tab === active);
    tab.tabEl.dataset.status = tab.status;
  }

  function activate(tab) {
    active = tab;
    for (const t of tabs) {
      t.el.classList.toggle('active', t === tab);
      updateTab(t);
    }
    if (tab) {
      tab.tabEl.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      requestAnimationFrame(() => {
        fitTab(tab);
        tab.term.focus();
      });
    }
    hooks.onChange();
  }

  async function closeTab(tab) {
    if (tab.status === 'running') {
      const ok = await hooks.confirm(
        `“${tab.title}” terminali kapatılsın mı?`,
        'Claude bu sekmede çalışıyor; kapatınca süreç sonlanır. Konuşma kaydı korunur, oturuma tekrar tıklayarak kaldığın yerden devam edebilirsin.',
        'Kapat',
      );
      if (!ok) return;
    }
    if (tab.ptyId != null) invoke('pty_close', { id: tab.ptyId }).catch(() => {});
    tab.ptyId = null;
    tab.term.dispose();
    tab.el.remove();
    tab.tabEl.remove();
    tabs.splice(tabs.indexOf(tab), 1);
    if (active === tab) activate(tabs[tabs.length - 1] || null);
    if (tabs.length === 0) setVisible(false);
    hooks.onChange();
  }

  // ---------------------------------------------------------------- yerleşim

  function setVisible(visible) {
    const was = document.body.classList.contains('has-terms');
    document.body.classList.toggle('has-terms', visible);
    if (visible && !was) invoke('ensure_window_width', { width: MIN_WINDOW_WIDTH }).catch(() => {});
  }

  let fitQueued = false;
  function scheduleFit() {
    if (fitQueued) return;
    fitQueued = true;
    requestAnimationFrame(() => {
      fitQueued = false;
      if (active) fitTab(active);
    });
  }

  function fitTab(tab) {
    if (!tab.el.classList.contains('active') || tab.el.clientWidth === 0) return;
    try {
      tab.fit.fit();
    } catch {
      /* görünmezken ölçülemez */
    }
  }

  function setSideWidth(px) {
    document.body.style.setProperty('--side-w', Math.round(px) + 'px');
  }

  function initSplitter() {
    splitterEl.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      splitterEl.setPointerCapture(e.pointerId);
      document.body.classList.add('resizing');
      let width = null;
      const onMove = (ev) => {
        width = Math.max(240, Math.min(ev.clientX, window.innerWidth - 360));
        setSideWidth(width);
      };
      const onUp = () => {
        splitterEl.removeEventListener('pointermove', onMove);
        document.body.classList.remove('resizing');
        if (width != null) hooks.onSideWidth(width);
        scheduleFit();
      };
      splitterEl.addEventListener('pointermove', onMove);
      splitterEl.addEventListener('pointerup', onUp, { once: true });
    });
  }

  // ---------------------------------------------------------------- sorgular

  function isOpen(sessionId) {
    return tabs.some((t) => t.sessionId === sessionId);
  }

  function isActive(sessionId) {
    return active != null && active.sessionId === sessionId;
  }

  function runningCount() {
    return tabs.filter((t) => t.status === 'running').length;
  }

  return { init, open, isOpen, isActive, runningCount };
})();
