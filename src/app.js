// SessionBoard — Claude Code oturumlarını workspace → grup → oturum ağacında düzenler.
//
// Oturum listesi Claude'un kayıtlarından SALT-OKUNUR gelir (src-tauri/src/sessions.rs).
// Gruplar, atamalar, sıralama ve tercihler yalnız bu uygulamanın state.json'ındadır
// (src-tauri/src/store.rs); Claude'un dosyalarına hiçbir şey yazılmaz.
(() => {
  'use strict';

  const invoke = window.__TAURI__.core.invoke;
  const REFRESH_MS = 20000;
  const SAVE_DELAY_MS = 250;
  const OPENING_TIMEOUT_MS = 10000;
  const LINK_PREFIX = 'claude://claude.ai/epitaxy/';

  /** @type {Array<{id:string,title:string,cwd:string,workspace:string,workspaceKey:string,branch:?string,archived:boolean,createdAt:number,lastActivityAt:number}>} */
  let sessions = [];
  let byId = new Map();
  let sessionsSig = '';
  let source = '';
  let skipped = 0;

  let state = normalizeState(null);
  // Yükleme başarısızsa KAYDETME: boş durumla mevcut dosyayı ezmeyelim.
  let stateLoaded = false;

  let query = '';
  let loading = false;
  let loadedOnce = false;
  let lastLoadedAt = null;
  let errorText = '';
  let errorTimer = null;
  // Grup dosyası okunamadıysa kalıcı uyarı (kayıt kapalı olduğu sürece görünür kalır).
  let stateError = '';
  let flashText = '';
  let flashTimer = null;
  let saveTimer = null;
  // "Claude'da aç" sürerken (Claude öne gelene kadar) dönen gösterge gösterilen oturum.
  let openingId = null;
  let openingTimer = null;

  const selected = new Set();
  let anchorId = null;
  let dragIds = null;
  let dragWs = null;
  let dropEl = null;
  let menuEl = null;
  let currentModel = [];
  // Filtre (arama/gizli/favori) uygulanmadan önceki tam klasör sırası; sıralama işlemleri bunun üzerinde yapılır.
  let allOrder = [];
  let hiddenCount = 0;
  let dragWsKey = null;
  let wsDropEl = null;

  // Satır içi düzenleme / menü / onay / sürükleme sürerken ağaç yeniden çizilmez;
  // fare basılıyken de çizilmez (mousedown ile click arasında DOM değişirse tık kaybolur).
  let interacting = 0;
  let pointerDown = false;
  let pendingRender = false;

  const treeEl = document.getElementById('tree');
  const searchEl = document.getElementById('search');
  const statusEl = document.getElementById('status');
  const statusRightEl = document.getElementById('status-right');
  const refreshBtn = document.getElementById('refresh');
  const menuBtn = document.getElementById('menu');
  const favToggleBtn = document.getElementById('fav-toggle');
  const terminals = window.SessionTerminals;

  const ICONS = {
    chevron: '<svg viewBox="0 0 16 16"><path d="M6 3.5L10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    folder: '<svg viewBox="0 0 16 16"><path d="M1.75 4.25c0-.69.56-1.25 1.25-1.25h3.1l1.5 1.5H13c.69 0 1.25.56 1.25 1.25v6c0 .69-.56 1.25-1.25 1.25H3c-.69 0-1.25-.56-1.25-1.25z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
    plus: '<svg viewBox="0 0 16 16"><path d="M8 3.25v9.5M3.25 8h9.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>',
    close: '<svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
    star: '<svg viewBox="0 0 16 16"><path d="M8 1.9l1.8 3.75 4.1.55-3 2.85.75 4.05L8 11.15 4.35 13.1l.75-4.05-3-2.85 4.1-.55z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
    eyeOff: '<svg viewBox="0 0 16 16"><path d="M2 8s2.2-4.25 6-4.25S14 8 14 8s-2.2 4.25-6 4.25S2 8 2 8z" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="8" r="1.9" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M2.5 13.5l11-11" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>',
    eye: '<svg viewBox="0 0 16 16"><path d="M2 8s2.2-4.25 6-4.25S14 8 14 8s-2.2 4.25-6 4.25S2 8 2 8z" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="8" r="1.9" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>',
    external: '<svg viewBox="0 0 16 16"><path d="M9.25 2.75h4v4M13.25 2.75L7.5 8.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/><path d="M11.75 9.25v3c0 .55-.45 1-1 1h-7c-.55 0-1-.45-1-1v-7c0-.55.45-1 1-1h3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>',
    terminal: '<svg viewBox="0 0 16 16"><rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M4.5 6.25L6.75 8 4.5 9.75M8 10h3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  };

  // ---------------------------------------------------------------- durum

  function normalizeState(raw) {
    const s = isObj(raw) ? raw : {};
    return {
      version: 1,
      workspaces: isObj(s.workspaces) ? s.workspaces : {},
      workspaceOrder: Array.isArray(s.workspaceOrder) ? s.workspaceOrder.filter((k) => typeof k === 'string') : [],
      aliases: isObj(s.aliases) ? s.aliases : {},
      assignments: isObj(s.assignments) ? s.assignments : {},
      prefs: {
        showArchived: false,
        showHidden: false,
        favoritesOnly: false,
        alwaysOnTop: false,
        tipDismissed: false,
        terminalWarned: false,
        sideWidth: 0,
        ...(isObj(s.prefs) ? s.prefs : {}),
      },
    };
  }

  function isObj(v) {
    return v !== null && typeof v === 'object' && !Array.isArray(v);
  }

  /** Değiştirmek için: kayıt yoksa oluşturur. */
  function wsState(key) {
    let w = state.workspaces[key];
    if (!isObj(w)) w = state.workspaces[key] = {};
    if (!Array.isArray(w.groups)) w.groups = [];
    return w;
  }

  /** Okumak için: kayıt oluşturmaz. */
  function peekWs(key) {
    const w = state.workspaces[key];
    if (!isObj(w)) return { groups: [] };
    return Array.isArray(w.groups) ? w : { ...w, groups: [] };
  }

  /** Birleştirilmiş (başka klasörün altına katılmış) workspace'i hedefine çözer. */
  function resolveKey(key) {
    let k = key;
    for (let i = 0; i < 10 && typeof state.aliases[k] === 'string'; i++) k = state.aliases[k];
    return k;
  }

  function scheduleSave() {
    if (!stateLoaded) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        await invoke('save_state', { state });
      } catch (err) {
        setError('Kaydedilemedi: ' + err);
      }
    }, SAVE_DELAY_MS);
  }

  /** Kullanıcı eylemi sonrası: kaydet + hemen çiz. */
  function commit() {
    scheduleSave();
    render();
  }

  // ---------------------------------------------------------------- model

  function buildModel() {
    const q = fold(query.trim());
    const map = new Map();
    for (const s of sessions) {
      if (s.archived && !state.prefs.showArchived) continue;
      const key = resolveKey(s.workspaceKey);
      let ws = map.get(key);
      if (!ws) {
        ws = { key, ownPath: '', anyPath: s.workspace, latest: 0, sessions: [] };
        map.set(key, ws);
      }
      if (s.workspaceKey === key && !ws.ownPath) ws.ownPath = s.workspace;
      ws.latest = Math.max(ws.latest, s.lastActivityAt);
      if (!q || matches(s, q)) ws.sessions.push(s);
    }

    let list = [...map.values()];
    for (const ws of list) {
      const st = peekWs(ws.key);
      ws.path = ws.ownPath || st.path || ws.anyPath;
      ws.name = st.name || baseName(ws.path);
      ws.hidden = !!st.hidden;
      ws.favorite = !!st.favorite;
      ws.collapsed = !!st.collapsed;
      ws.ungroupedCollapsed = !!st.ungroupedCollapsed;
      ws.merged = Object.keys(state.aliases).filter((src) => state.aliases[src] === ws.key);
      ws.groups = st.groups.map((g) => ({ id: g.id, name: g.name, collapsed: !!g.collapsed, sessions: [] }));
      const groupOf = new Map(ws.groups.map((g) => [g.id, g]));
      ws.ungrouped = [];
      for (const s of ws.sessions) {
        const g = groupOf.get(state.assignments[s.id]);
        (g ? g.sessions : ws.ungrouped).push(s);
      }
    }

    // Elle sıralanan klasörler önce (kayıtlı sırayla), kalanlar son etkinliğe göre.
    const order = state.workspaceOrder;
    list.sort((a, b) => {
      const ia = order.indexOf(a.key);
      const ib = order.indexOf(b.key);
      if (ia === -1 && ib === -1) return b.latest - a.latest;
      if (ia === -1) return 1;
      if (ib === -1) return -1;
      return ia - ib;
    });
    allOrder = list.map((ws) => ws.key);

    hiddenCount = list.filter((ws) => ws.hidden).length;
    if (!state.prefs.showHidden) list = list.filter((ws) => !ws.hidden);
    if (state.prefs.favoritesOnly) list = list.filter((ws) => ws.favorite);
    if (q) list = list.filter((ws) => ws.sessions.length > 0);
    return list;
  }

  function matches(s, q) {
    return fold(s.title).includes(q) || (s.branch ? fold(s.branch).includes(q) : false);
  }

  /** Arama için Türkçe katlama: büyük/küçük + aksan (ş→s, ı→i …) duyarsız. */
  function fold(str) {
    return (str || '')
      .toLocaleLowerCase('tr-TR')
      .normalize('NFD')
      .replace(/[̀-ͯ]/g, '')
      .replace(/ı/g, 'i');
  }

  function baseName(path) {
    const parts = (path || '').split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] || path || '(klasörsüz)';
  }

  // ---------------------------------------------------------------- çizim

  function render() {
    pendingRender = false;
    const scroll = treeEl.scrollTop;
    currentModel = buildModel();
    const searching = query.trim() !== '';
    const frag = document.createDocumentFragment();

    if (!searching && !state.prefs.tipDismissed && currentModel.length > 0 && !hasAnyGroup()) {
      frag.append(renderTip());
    }
    if (currentModel.length === 0) frag.append(renderEmpty(searching));
    for (const ws of currentModel) frag.append(renderWorkspace(ws, searching));

    treeEl.replaceChildren(frag);
    treeEl.scrollTop = scroll;
    favToggleBtn.classList.toggle('on', !!state.prefs.favoritesOnly);
    favToggleBtn.setAttribute('aria-pressed', String(!!state.prefs.favoritesOnly));
    favToggleBtn.title = state.prefs.favoritesOnly ? 'Tüm klasörleri göster' : 'Yalnız favori klasörler';
    renderStatus();
  }

  /** Otomatik yenilemeden gelen çizim: kullanıcı bir şey yaparken bekletilir. */
  function softRender() {
    if (interacting > 0 || pointerDown) pendingRender = true;
    else render();
  }

  function flushPending() {
    if (pendingRender && interacting === 0 && !pointerDown) render();
  }

  function release() {
    interacting = Math.max(0, interacting - 1);
    if (interacting === 0 && pendingRender) setTimeout(flushPending, 0);
  }

  function hasAnyGroup() {
    return Object.values(state.workspaces).some((w) => isObj(w) && Array.isArray(w.groups) && w.groups.length > 0);
  }

  function renderWorkspace(ws, searching) {
    const box = el('div', 'ws' + (ws.hidden ? ' is-hidden' : ''));
    const open = searching || !ws.collapsed;

    const row = el('div', 'row ws-row');
    row.dataset.kind = 'ws';
    row.dataset.ws = ws.key;
    row.setAttribute('role', 'treeitem');
    row.setAttribute('aria-expanded', String(open));
    const mergedPaths = ws.merged.map((k) => peekWs(k).path || k);
    row.title = [ws.path, ...mergedPaths.map((p) => '+ ' + p)].join('\n');
    row.draggable = !searching;
    const folder = svg('folder');
    folder.classList.add('folder');
    const fav = rowButton(ICONS.star, ws.favorite ? 'Favorilerden çıkar' : 'Favorilere ekle', 'fav');
    if (ws.favorite) fav.classList.add('on');
    const hide = rowButton(ws.hidden ? ICONS.eye : ICONS.eyeOff, ws.hidden ? 'Göster' : 'Gizle', 'hide');
    const add = rowButton(ICONS.plus, 'Grup ekle', 'add-group');
    row.append(chevron(open), folder, span('name', ws.name), span('count', String(ws.sessions.length)), fav, hide, add);
    box.append(row);
    if (!open) return box;

    const kids = el('div', 'children');
    if (ws.groups.length > 0) {
      for (const g of ws.groups) {
        if (searching && g.sessions.length === 0) continue;
        kids.append(renderGroup(ws, g, searching));
      }
      if (!searching || ws.ungrouped.length > 0) {
        kids.append(renderGroup(ws, { id: '', name: 'Grupsuz', collapsed: ws.ungroupedCollapsed, sessions: ws.ungrouped, pseudo: true }, searching));
      }
    } else {
      for (const s of ws.ungrouped) kids.append(renderSession(s, ws.key, 1));
      if (ws.ungrouped.length === 0) kids.append(span('note', 'Oturum yok', 'div'));
    }
    box.append(kids);
    return box;
  }

  function renderGroup(ws, g, searching) {
    const box = el('div', 'group' + (g.pseudo ? ' ungrouped' : ''));
    // Sürükle-bırak hedefi grubun tamamıdır (başlık + içindeki oturumlar).
    box.dataset.dropWs = ws.key;
    box.dataset.dropGroup = g.id;
    const open = searching || !g.collapsed;

    const row = el('div', 'row group-row');
    row.dataset.kind = g.pseudo ? 'ungrouped' : 'group';
    row.dataset.ws = ws.key;
    row.dataset.group = g.id;
    row.setAttribute('role', 'treeitem');
    row.setAttribute('aria-expanded', String(open));
    row.append(chevron(open), span('name', g.name), span('count', String(g.sessions.length)));
    box.append(row);

    if (open) {
      const kids = el('div', 'children');
      for (const s of g.sessions) kids.append(renderSession(s, ws.key, 2));
      if (g.sessions.length === 0 && !g.pseudo) kids.append(span('note', 'Oturumları buraya sürükle', 'div'));
      box.append(kids);
    }
    return box;
  }

  function renderSession(s, wsKey, depth) {
    const cls = ['row', 'session', 'depth-' + depth];
    if (selected.has(s.id)) cls.push('selected');
    if (s.archived) cls.push('archived');
    const inTerminal = terminals.isOpen(s.id);
    if (inTerminal && terminals.isActive(s.id)) cls.push('term-active');
    const row = el('div', cls.join(' '));
    row.dataset.kind = 'session';
    row.dataset.id = s.id;
    row.dataset.ws = wsKey;
    row.draggable = true;
    row.setAttribute('role', 'treeitem');
    row.title = [
      s.title || '(başlıksız)',
      s.branch ? 'Dal: ' + s.branch : '',
      s.cwd,
      'Son etkinlik: ' + fullDate(s.lastActivityAt),
      s.archived ? 'Arşivlenmiş' : '',
    ].filter(Boolean).join('\n');
    const time = span('time', ago(s.lastActivityAt));
    time.dataset.at = String(s.lastActivityAt);
    row.append(span('name', s.title || '(başlıksız)'));
    if (inTerminal) {
      const mark = svg('terminal');
      mark.classList.add('term-mark');
      row.append(mark);
    }
    // Açma yalnız bu düğmelerle (satırın üzerine gelince görünür); tek tık yalnız seçer.
    const openTerm = rowButton(ICONS.terminal, inTerminal ? 'Terminal sekmesine geç' : 'Terminalde aç', 'open-terminal');
    if (!s.cliSessionId) {
      openTerm.disabled = true;
      openTerm.title = 'Konuşma kaydı yok; terminalde açılamaz';
    }
    const openClaude = rowButton(ICONS.external, "Claude'da aç", 'open-claude');
    if (s.id === openingId) openClaude.classList.add('busy');
    row.append(openTerm, openClaude, time);
    return row;
  }

  function renderTip() {
    const box = el('div', 'tip');
    box.innerHTML =
      'Klasör satırındaki <b>+</b> ile grup ekle, oturumları gruba <b>sürükle</b>. ' +
      'Klasörleri sürükleyerek sırala, <b>☆</b> ile favorilere ekle. ' +
      '<b>Ctrl</b>/<b>Shift</b> ile çoklu seçim, <b>sağ tık</b> ile tüm işlemler. ' +
      'Oturumun üzerine gelince çıkan düğmelerle <b>terminalde</b> ya da <b>Claude\'da</b> aç.';
    const close = el('button', 'row-btn');
    close.innerHTML = ICONS.close;
    close.title = 'Kapat';
    close.addEventListener('click', () => {
      state.prefs.tipDismissed = true;
      commit();
    });
    box.append(close);
    return box;
  }

  function renderEmpty(searching) {
    const box = el('div', 'empty');
    if (searching) {
      box.textContent = `“${query.trim()}” için sonuç yok.`;
    } else if (state.prefs.favoritesOnly) {
      box.textContent = 'Henüz favori klasör yok.';
      box.append(span('', 'Klasör satırındaki ☆ ile ekle ya da üstteki yıldızla tüm klasörlere dön.', 'small'));
    } else if (!loadedOnce) {
      box.textContent = 'Oturumlar okunuyor…';
    } else {
      box.textContent = 'Gösterilecek oturum yok.';
      if (source) box.append(span('', source, 'small'));
    }
    return box;
  }

  function renderStatus() {
    const visible = currentModel.reduce((n, ws) => n + ws.sessions.length, 0);
    let left = `${visible} oturum · ${currentModel.length} klasör`;
    if (selected.size > 1) left += ` · ${selected.size} seçili`;
    if (state.prefs.favoritesOnly) left += ' · favoriler';
    if (hiddenCount > 0 && !state.prefs.showHidden) left += ` · ${hiddenCount} gizli`;
    if (skipped > 0) left += ` · ${skipped} kayıt okunamadı`;
    statusEl.textContent = left;
    statusEl.className = '';

    const err = errorText || stateError;
    if (err) {
      statusRightEl.textContent = err;
      statusRightEl.className = 'err';
      statusRightEl.title = err;
    } else if (flashText) {
      statusRightEl.textContent = flashText;
      statusRightEl.className = 'ok';
      statusRightEl.title = '';
    } else {
      statusRightEl.textContent = lastLoadedAt ? 'Güncellendi ' + clock(lastLoadedAt) : '';
      statusRightEl.className = '';
      statusRightEl.title = source;
    }
  }

  /** Göreli zamanları DOM'u değiştirmeden tazeler. */
  function refreshTimes() {
    for (const t of treeEl.querySelectorAll('.time[data-at]')) t.textContent = ago(Number(t.dataset.at));
  }

  // ---------------------------------------------------------------- yardımcılar (DOM)

  function el(tag, cls) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    return e;
  }

  function span(cls, textValue, tag = 'span') {
    const e = el(tag, cls);
    e.textContent = textValue;
    return e;
  }

  function svg(name) {
    const holder = document.createElement('span');
    holder.innerHTML = ICONS[name];
    return holder.firstElementChild;
  }

  function rowButton(iconSvg, title, action) {
    const b = el('button', 'row-btn');
    b.innerHTML = iconSvg;
    b.title = title;
    b.dataset.action = action;
    return b;
  }

  function chevron(open) {
    const c = el('span', 'chev' + (open ? ' open' : ''));
    c.innerHTML = ICONS.chevron;
    return c;
  }

  function ago(ms) {
    if (!ms) return '';
    const sec = (Date.now() - ms) / 1000;
    if (sec < 60) return 'şimdi';
    if (sec < 3600) return Math.floor(sec / 60) + ' dk';
    if (sec < 86400) return Math.floor(sec / 3600) + ' sa';
    if (sec < 86400 * 7) return Math.floor(sec / 86400) + ' g';
    return new Date(ms).toLocaleDateString('tr-TR', { day: '2-digit', month: '2-digit', year: '2-digit' });
  }

  function fullDate(ms) {
    return ms ? new Date(ms).toLocaleString('tr-TR', { dateStyle: 'medium', timeStyle: 'short' }) : '-';
  }

  function clock(d) {
    return d.toLocaleTimeString('tr-TR', { hour: '2-digit', minute: '2-digit' });
  }

  /** İşlem hataları birkaç saniye sonra kendiliğinden silinir; okuma hatası başarılı okumada silinir. */
  function setError(msg, transient = true) {
    errorText = msg;
    clearTimeout(errorTimer);
    if (transient) {
      errorTimer = setTimeout(() => {
        if (errorText === msg) {
          errorText = '';
          renderStatus();
        }
      }, 8000);
    }
    renderStatus();
  }

  function flash(msg, durationMs = 2500) {
    flashText = msg;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => {
      flashText = '';
      renderStatus();
    }, durationMs);
    renderStatus();
  }

  // ---------------------------------------------------------------- veri

  async function refresh() {
    if (loading) return;
    loading = true;
    refreshBtn.classList.add('spin');
    try {
      const res = await invoke('list_sessions');
      const next = res.sessions || [];
      source = res.source || '';
      skipped = res.skipped || 0;
      lastLoadedAt = new Date();
      loadedOnce = true;
      if (errorText.startsWith('Oturumlar okunamadı')) errorText = '';
      const sig = next.map((s) => [s.id, s.title, s.lastActivityAt, s.archived, s.workspaceKey, s.branch].join('|')).join('\n');
      if (sig !== sessionsSig) {
        sessionsSig = sig;
        sessions = next;
        byId = new Map(sessions.map((s) => [s.id, s]));
        for (const id of [...selected]) if (!byId.has(id)) selected.delete(id);
        softRender();
      } else {
        refreshTimes();
        renderStatus();
      }
    } catch (err) {
      loadedOnce = true;
      setError('Oturumlar okunamadı: ' + err, false);
      softRender();
    } finally {
      loading = false;
      refreshBtn.classList.remove('spin');
    }
  }

  /**
   * Claude'da açar. Claude'un oturuma geçmesi bir iki saniye sürer; bu sürede düğme dönen gösterge
   * olur. Bitişi Claude'un öne gelmesinden (bu pencerenin odağı kaybetmesi) anlarız; gelmezse
   * gösterge OPENING_TIMEOUT_MS sonra kendiliğinden kalkar.
   */
  async function openSession(id) {
    if (openingId === id) return; // çift tık: zaten açılıyor
    setOpening(id);
    flash("Claude'da açılıyor…", OPENING_TIMEOUT_MS);
    try {
      await invoke('open_session', { id });
    } catch (err) {
      setOpening(null);
      setError('Açılamadı: ' + err);
    }
  }

  function setOpening(id) {
    openingId = id;
    clearTimeout(openingTimer);
    if (id) openingTimer = setTimeout(() => setOpening(null), OPENING_TIMEOUT_MS);
    else if (flashText === "Claude'da açılıyor…") {
      flashText = '';
      clearTimeout(flashTimer);
    }
    // Ağacı yeniden çizmeden yalnız ilgili düğmeleri güncelle (tıklama/hover bozulmasın).
    for (const b of treeEl.querySelectorAll('[data-action="open-claude"]')) {
      b.classList.toggle('busy', b.closest('.row')?.dataset.id === openingId);
    }
    renderStatus();
  }

  /** Oturumu gömülü terminalde `claude --resume` ile sürdürür (fork: kopyasıyla yeni oturum). */
  async function openInTerminal(id, fork = false) {
    const s = byId.get(id);
    if (!s) return;
    if (!s.cliSessionId) {
      setError('Bu oturumun konuşma kaydı yok; terminalde açılamaz.');
      return;
    }
    if (!state.prefs.terminalWarned) {
      const ok = await confirmDialog(
        'Oturum terminalde sürdürülecek',
        'Aynı oturumu Claude penceresinde de açık tutma: ikisi aynı konuşmaya yazar ve terminalde yaptıkların ' +
          "Claude'da oturumu yeniden açana kadar görünmez. Artifact, tarayıcı paneli gibi masaüstüne özel " +
          'özellikler terminalde yoktur. Riskten kaçınmak için sağ tık → “Kopyasını terminalde aç”.',
        'Anladım, aç',
      );
      if (!ok) return;
      state.prefs.terminalWarned = true;
      scheduleSave();
    }
    await terminals.open(s, { fork });
    render();
  }

  async function copyLink(id) {
    try {
      await navigator.clipboard.writeText(LINK_PREFIX + id);
      flash('Bağlantı kopyalandı');
    } catch (err) {
      setError('Kopyalanamadı: ' + err);
    }
  }

  async function applyAlwaysOnTop() {
    try {
      await invoke('set_always_on_top', { on: !!state.prefs.alwaysOnTop });
    } catch (err) {
      setError('Pencere ayarı uygulanamadı: ' + err);
    }
  }

  // ---------------------------------------------------------------- işlemler

  /** Oturumları gruba atar (gid boşsa Grupsuz'a döner). Başka klasördeki oturumlar atlanır. */
  function applyAssign(ids, wsKey, gid) {
    for (const id of ids) {
      const s = byId.get(id);
      if (!s || resolveKey(s.workspaceKey) !== wsKey) continue;
      if (gid) state.assignments[id] = gid;
      else delete state.assignments[id];
    }
  }

  function moveSessions(ids, wsKey, gid) {
    applyAssign(ids, wsKey, gid);
    selected.clear();
    commit();
  }

  function addGroup(wsKey, moveIds) {
    const w = wsState(wsKey);
    const g = { id: crypto.randomUUID(), name: 'Yeni grup', collapsed: false };
    w.groups.push(g);
    w.collapsed = false;
    if (moveIds) {
      applyAssign(moveIds, wsKey, g.id);
      selected.clear();
    }
    commit();
    const nameEl = treeEl.querySelector(`.group-row[data-group="${g.id}"] .name`);
    nameEl?.scrollIntoView({ block: 'nearest' });
    inlineEdit(nameEl, g.name, (v) => {
      if (v) g.name = v;
      commit();
    });
  }

  function renameGroup(wsKey, gid) {
    const g = wsState(wsKey).groups.find((x) => x.id === gid);
    if (!g) return;
    const nameEl = treeEl.querySelector(`.group-row[data-group="${gid}"] .name`);
    inlineEdit(nameEl, g.name, (v) => {
      if (v) g.name = v;
      commit();
    });
  }

  function renameWorkspace(wsKey) {
    const ws = currentModel.find((w) => w.key === wsKey);
    if (!ws) return;
    const nameEl = [...treeEl.querySelectorAll('.ws-row')].find((r) => r.dataset.ws === wsKey)?.querySelector('.name');
    inlineEdit(nameEl, ws.name, (v) => {
      const w = wsState(wsKey);
      // Boş bırakılırsa klasör adına döner.
      if (v && v !== baseName(ws.path)) w.name = v;
      else delete w.name;
      commit();
    });
  }

  function moveGroup(wsKey, gid, dir) {
    const groups = wsState(wsKey).groups;
    const i = groups.findIndex((g) => g.id === gid);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= groups.length) return;
    [groups[i], groups[j]] = [groups[j], groups[i]];
    commit();
  }

  /** Yukarı/aşağı: görünen komşunun önüne/arkasına taşır. */
  function moveWorkspace(wsKey, dir) {
    const i = currentModel.findIndex((w) => w.key === wsKey);
    const neighbor = currentModel[i + dir];
    if (i < 0 || !neighbor) return;
    placeWorkspace(wsKey, neighbor.key, dir > 0);
  }

  /**
   * Klasörü hedefin önüne (after=false) ya da arkasına koyar. Tam sıra (gizli/favori dışı/arşivli
   * klasörler dahil) üzerinde çalışır; görünmeyen klasörlerin birbirine göre yeri değişmez.
   */
  function placeWorkspace(wsKey, targetKey, after) {
    if (wsKey === targetKey) return;
    const keys = [...allOrder, ...state.workspaceOrder.filter((k) => !allOrder.includes(k))].filter((k) => k !== wsKey);
    const t = keys.indexOf(targetKey);
    if (t < 0) return;
    keys.splice(after ? t + 1 : t, 0, wsKey);
    state.workspaceOrder = keys;
    commit();
  }

  function toggleFavorite(wsKey) {
    const w = wsState(wsKey);
    w.favorite = !w.favorite;
    if (!w.favorite) delete w.favorite;
    commit();
  }

  function toggleHidden(wsKey) {
    const w = wsState(wsKey);
    w.hidden = !w.hidden;
    if (!w.hidden) delete w.hidden;
    commit();
    if (w.hidden && !state.prefs.showHidden) flash('Klasör gizlendi · ⋯ → “Gizli klasörleri göster”');
  }

  async function deleteGroup(wsKey, gid) {
    const w = wsState(wsKey);
    const g = w.groups.find((x) => x.id === gid);
    if (!g) return;
    const count = Object.values(state.assignments).filter((v) => v === gid).length;
    const ok = await confirmDialog(
      `“${g.name}” grubu silinsin mi?`,
      count > 0 ? `İçindeki ${count} oturum Grupsuz'a döner. Oturumların kendisi silinmez.` : 'Grup boş.',
      'Sil',
      true,
    );
    if (!ok) return;
    w.groups = w.groups.filter((x) => x.id !== gid);
    for (const [sid, v] of Object.entries(state.assignments)) if (v === gid) delete state.assignments[sid];
    commit();
  }

  async function mergeWorkspace(srcKey, targetKey) {
    const src = currentModel.find((w) => w.key === srcKey);
    const target = currentModel.find((w) => w.key === targetKey);
    if (!src || !target) return;
    const ok = await confirmDialog(
      `“${src.name}” klasörü “${target.name}” altına katılsın mı?`,
      'Oturumları ve grupları hedef klasörde görünür. Hedefin sağ tık menüsünden “Ayır” ile geri alabilirsin.',
      'Kat',
    );
    if (!ok) return;
    const from = wsState(srcKey);
    const to = wsState(targetKey);
    from.path = from.path || src.path; // ayırırken adını gösterebilmek için
    to.path = to.path || target.path;
    to.groups.push(...from.groups);
    from.groups = [];
    state.aliases[srcKey] = targetKey;
    state.workspaceOrder = state.workspaceOrder.filter((k) => k !== srcKey);
    commit();
  }

  function unmergeWorkspace(srcKey) {
    delete state.aliases[srcKey];
    commit();
  }

  function setAllCollapsed(collapsed) {
    for (const ws of currentModel) {
      const w = wsState(ws.key);
      w.collapsed = collapsed;
      if (!collapsed) {
        w.ungroupedCollapsed = false;
        for (const g of w.groups) g.collapsed = false;
      }
    }
    commit();
  }

  function togglePref(name) {
    state.prefs[name] = !state.prefs[name];
    commit();
  }

  // ---------------------------------------------------------------- satır içi düzenleme

  function inlineEdit(nameEl, initial, onDone) {
    if (!nameEl) return;
    interacting++;
    const input = el('input', 'inline-edit');
    input.value = initial;
    input.maxLength = 80;
    input.spellcheck = false;
    nameEl.replaceWith(input);
    input.focus();
    input.select();
    let finished = false;
    const finish = (save) => {
      if (finished) return;
      finished = true;
      release();
      const v = input.value.trim();
      if (save && v !== initial) onDone(v);
      else render();
    };
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') finish(true);
      else if (e.key === 'Escape') finish(false);
    });
    input.addEventListener('blur', () => finish(true));
    input.addEventListener('click', (e) => e.stopPropagation());
    input.addEventListener('mousedown', (e) => e.stopPropagation());
  }

  // ---------------------------------------------------------------- menü

  function showMenu(x, y, items) {
    closeMenu();
    interacting++;
    menuEl = el('div', 'menu');
    menuEl.setAttribute('role', 'menu');
    for (const it of items) {
      if (it.sep) {
        menuEl.append(el('div', 'menu-sep'));
        continue;
      }
      if (it.header) {
        menuEl.append(span('menu-header', it.header, 'div'));
        continue;
      }
      const b = el('button', 'menu-item' + (it.danger ? ' danger' : '') + (it.checked ? ' on' : ''));
      b.textContent = it.label;
      b.title = it.label;
      b.disabled = !!it.disabled;
      b.setAttribute('role', it.checked !== undefined ? 'menuitemcheckbox' : 'menuitem');
      b.addEventListener('click', () => {
        closeMenu();
        it.action();
      });
      menuEl.append(b);
    }
    document.body.append(menuEl);
    const r = menuEl.getBoundingClientRect();
    const left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
    const top = y + r.height > window.innerHeight - 4 ? Math.max(4, y - r.height) : y;
    menuEl.style.left = left + 'px';
    menuEl.style.top = top + 'px';
  }

  function closeMenu() {
    if (!menuEl) return;
    menuEl.remove();
    menuEl = null;
    release();
  }

  function workspaceMenu(wsKey) {
    const ws = currentModel.find((w) => w.key === wsKey);
    if (!ws) return [];
    const searching = query.trim() !== '';
    const idx = currentModel.indexOf(ws);
    const items = [
      { label: 'Grup ekle', action: () => addGroup(wsKey) },
      { label: 'Yeniden adlandır', action: () => renameWorkspace(wsKey) },
      { sep: true },
      { label: ws.favorite ? 'Favorilerden çıkar' : 'Favorilere ekle', action: () => toggleFavorite(wsKey) },
      { label: ws.hidden ? 'Göster' : 'Gizle', action: () => toggleHidden(wsKey) },
      { sep: true },
      { label: 'Yukarı taşı', disabled: searching || idx <= 0, action: () => moveWorkspace(wsKey, -1) },
      { label: 'Aşağı taşı', disabled: searching || idx >= currentModel.length - 1, action: () => moveWorkspace(wsKey, 1) },
    ];
    if (ws.merged.length > 0) {
      items.push({ sep: true }, { header: 'Katılmış klasörler' });
      for (const src of ws.merged) {
        const st = peekWs(src);
        items.push({ label: 'Ayır: ' + (st.name || baseName(st.path || src)), action: () => unmergeWorkspace(src) });
      }
    }
    const others = currentModel.filter((w) => w.key !== wsKey);
    if (others.length > 0 && !searching) {
      items.push({ sep: true }, { header: 'Bu klasörü şunun altına kat' });
      for (const o of others) items.push({ label: o.name, action: () => mergeWorkspace(wsKey, o.key) });
    }
    return items;
  }

  function groupMenu(wsKey, gid) {
    const groups = peekWs(wsKey).groups;
    const i = groups.findIndex((g) => g.id === gid);
    return [
      { label: 'Yeniden adlandır', action: () => renameGroup(wsKey, gid) },
      { label: 'Yukarı taşı', disabled: i <= 0, action: () => moveGroup(wsKey, gid, -1) },
      { label: 'Aşağı taşı', disabled: i < 0 || i >= groups.length - 1, action: () => moveGroup(wsKey, gid, 1) },
      { sep: true },
      { label: 'Grubu sil', danger: true, action: () => deleteGroup(wsKey, gid) },
    ];
  }

  function sessionMenu(wsKey) {
    const ids = [...selected];
    const items = [];
    if (ids.length === 1) {
      const s = byId.get(ids[0]);
      const noTranscript = !s || !s.cliSessionId;
      items.push(
        { label: 'Terminalde aç', disabled: noTranscript, action: () => openInTerminal(ids[0]) },
        { label: 'Kopyasını terminalde aç', disabled: noTranscript, action: () => openInTerminal(ids[0], true) },
        { label: "Claude'da aç", action: () => openSession(ids[0]) },
        { label: 'Bağlantıyı kopyala', action: () => copyLink(ids[0]) },
        { sep: true },
      );
    }
    items.push({ header: ids.length > 1 ? `${ids.length} oturumu taşı` : 'Gruba taşı' });
    const current = ids.length === 1 ? state.assignments[ids[0]] || '' : null;
    for (const g of peekWs(wsKey).groups) {
      items.push({ label: g.name, disabled: current === g.id, action: () => moveSessions(ids, wsKey, g.id) });
    }
    items.push({ label: 'Grupsuz', disabled: current === '', action: () => moveSessions(ids, wsKey, '') });
    items.push({ label: 'Yeni gruba…', action: () => addGroup(wsKey, ids) });
    return items;
  }

  function viewMenuItems() {
    return [
      { label: 'Yalnız favori klasörler', checked: !!state.prefs.favoritesOnly, action: () => togglePref('favoritesOnly') },
      { label: 'Arşivlenmiş oturumları göster', checked: !!state.prefs.showArchived, action: () => togglePref('showArchived') },
      { label: 'Gizli klasörleri göster', checked: !!state.prefs.showHidden, action: () => togglePref('showHidden') },
      { label: 'Her zaman üstte', checked: !!state.prefs.alwaysOnTop, action: () => { togglePref('alwaysOnTop'); applyAlwaysOnTop(); } },
      { sep: true },
      { label: 'Tümünü daralt', action: () => setAllCollapsed(true) },
      { label: 'Tümünü genişlet', action: () => setAllCollapsed(false) },
      ...(state.prefs.tipDismissed ? [{ label: 'İpucunu yeniden göster', action: () => { state.prefs.tipDismissed = false; commit(); } }] : []),
    ];
  }

  // ---------------------------------------------------------------- onay penceresi

  function confirmDialog(title, body, okLabel, danger = false) {
    closeMenu();
    interacting++;
    return new Promise((resolve) => {
      const back = el('div', 'modal-back');
      const box = el('div', 'modal');
      box.setAttribute('role', 'dialog');
      box.setAttribute('aria-modal', 'true');
      box.append(span('modal-title', title, 'div'), span('modal-body', body, 'div'));
      const actions = el('div', 'modal-actions');
      const cancel = el('button', 'btn');
      cancel.textContent = 'Vazgeç';
      const ok = el('button', 'btn ' + (danger ? 'btn-danger' : 'btn-primary'));
      ok.textContent = okLabel;
      actions.append(cancel, ok);
      box.append(actions);
      back.append(box);
      document.body.append(back);
      ok.focus();

      const onKey = (e) => {
        if (e.key === 'Escape' || e.key === 'Enter') {
          e.preventDefault();
          e.stopPropagation();
          done(e.key === 'Enter');
        }
      };
      const done = (value) => {
        document.removeEventListener('keydown', onKey, true);
        back.remove();
        release();
        resolve(value);
      };
      document.addEventListener('keydown', onKey, true);
      cancel.addEventListener('click', () => done(false));
      ok.addEventListener('click', () => done(true));
      back.addEventListener('mousedown', (e) => {
        if (e.target === back) done(false);
      });
    });
  }

  // ---------------------------------------------------------------- seçim

  function visibleSessionIds() {
    return [...treeEl.querySelectorAll('.row.session')].map((r) => r.dataset.id);
  }

  function selectRange(fromId, toId) {
    const ids = visibleSessionIds();
    let i = ids.indexOf(fromId);
    let j = ids.indexOf(toId);
    if (i < 0 || j < 0) return;
    if (i > j) [i, j] = [j, i];
    for (let k = i; k <= j; k++) selected.add(ids[k]);
  }

  function onSessionClick(e, id) {
    if (e.ctrlKey || e.metaKey) {
      if (selected.has(id)) selected.delete(id);
      else selected.add(id);
      anchorId = id;
      render();
      return;
    }
    if (e.shiftKey && anchorId) {
      selectRange(anchorId, id);
      render();
      return;
    }
    // Tek tık yalnız seçer; açmak satırdaki düğmelerle ya da sağ tık menüsüyle yapılır.
    selected.clear();
    selected.add(id);
    anchorId = id;
    render();
  }

  // ---------------------------------------------------------------- olaylar: ağaç

  treeEl.addEventListener('click', (e) => {
    if (e.target.closest('input')) return;
    const row = e.target.closest('.row');
    if (!row) return;
    const action = e.target.closest('[data-action]')?.dataset.action;
    if (action) {
      e.stopPropagation();
      if (action === 'add-group') addGroup(row.dataset.ws);
      else if (action === 'fav') toggleFavorite(row.dataset.ws);
      else if (action === 'hide') toggleHidden(row.dataset.ws);
      else if (action === 'open-terminal') openInTerminal(row.dataset.id);
      else if (action === 'open-claude') openSession(row.dataset.id);
      return;
    }
    switch (row.dataset.kind) {
      case 'ws': {
        const w = wsState(row.dataset.ws);
        w.collapsed = !w.collapsed;
        commit();
        break;
      }
      case 'group': {
        const g = wsState(row.dataset.ws).groups.find((x) => x.id === row.dataset.group);
        if (g) {
          g.collapsed = !g.collapsed;
          commit();
        }
        break;
      }
      case 'ungrouped': {
        const w = wsState(row.dataset.ws);
        w.ungroupedCollapsed = !w.ungroupedCollapsed;
        commit();
        break;
      }
      case 'session':
        onSessionClick(e, row.dataset.id);
        break;
    }
  });

  treeEl.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (e.target.closest('input')) return;
    const row = e.target.closest('.row');
    if (!row) return;
    const { kind, ws } = row.dataset;
    if (kind === 'ws') {
      showMenu(e.clientX, e.clientY, workspaceMenu(ws));
    } else if (kind === 'group') {
      showMenu(e.clientX, e.clientY, groupMenu(ws, row.dataset.group));
    } else if (kind === 'session') {
      const id = row.dataset.id;
      if (!selected.has(id)) {
        selected.clear();
        selected.add(id);
        anchorId = id;
        render();
      }
      showMenu(e.clientX, e.clientY, sessionMenu(ws));
    }
  });

  // Sürükle-bırak: sürükleme sırasında DOM yeniden çizilmez (sürüklenen öğe değişirse
  // tarayıcı sürüklemeyi iptal eder); seçim sınıfları yerinde güncellenir.
  treeEl.addEventListener('dragstart', (e) => {
    const wsRow = e.target.closest('.ws-row');
    if (wsRow) {
      dragWsKey = wsRow.dataset.ws;
      interacting++;
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', wsRow.querySelector('.name')?.textContent || '');
      wsRow.parentElement.classList.add('dragging-ws');
      treeEl.classList.add('dragging');
      return;
    }
    const row = e.target.closest('.row.session');
    if (!row) return;
    const id = row.dataset.id;
    if (!selected.has(id)) {
      selected.clear();
      selected.add(id);
      anchorId = id;
    }
    dragIds = [...selected];
    dragWs = row.dataset.ws;
    interacting++;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', dragIds.map((x) => LINK_PREFIX + x).join('\n'));
    if (dragIds.length > 1) {
      const ghost = span('drag-ghost', `${dragIds.length} oturum`, 'div');
      document.body.append(ghost);
      e.dataTransfer.setDragImage(ghost, 12, 12);
      setTimeout(() => ghost.remove(), 0);
    }
    treeEl.classList.add('dragging');
    for (const r of treeEl.querySelectorAll('.row.session')) r.classList.toggle('selected', selected.has(r.dataset.id));
  });

  function dropTargetOf(e) {
    const box = e.target.closest('.group');
    if (!box || !dragIds || box.dataset.dropWs !== dragWs) return null;
    return box;
  }

  function setDropEl(box) {
    if (dropEl === box) return;
    dropEl?.classList.remove('drop');
    dropEl = box;
    box?.classList.add('drop');
  }

  function endDrag() {
    if (!dragIds) return;
    dragIds = null;
    dragWs = null;
    setDropEl(null);
    treeEl.classList.remove('dragging');
    release();
  }

  /** Klasör sürüklenirken: imleç hedef klasör satırının üst yarısındaysa önüne, değilse arkasına. */
  function wsDropOf(e) {
    const box = e.target.closest('.ws');
    if (!box || !dragWsKey) return null;
    const key = box.querySelector('.ws-row')?.dataset.ws;
    if (!key || key === dragWsKey) return null;
    const rowRect = box.querySelector('.ws-row').getBoundingClientRect();
    const after = e.clientY > rowRect.top + rowRect.height / 2;
    return { box, key, after };
  }

  function setWsDrop(target) {
    if (wsDropEl && (!target || wsDropEl !== target.box)) wsDropEl.classList.remove('drop-before', 'drop-after');
    wsDropEl = target ? target.box : null;
    if (target) {
      target.box.classList.toggle('drop-before', !target.after);
      target.box.classList.toggle('drop-after', target.after);
    }
  }

  function endWsDrag() {
    if (!dragWsKey) return;
    dragWsKey = null;
    setWsDrop(null);
    for (const b of treeEl.querySelectorAll('.dragging-ws')) b.classList.remove('dragging-ws');
    treeEl.classList.remove('dragging');
    release();
  }

  treeEl.addEventListener('dragover', (e) => {
    if (dragWsKey) {
      const target = wsDropOf(e);
      setWsDrop(target);
      if (target) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
      }
      return;
    }
    const box = dropTargetOf(e);
    setDropEl(box);
    if (box) {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
    }
  });

  treeEl.addEventListener('dragleave', (e) => {
    if (!treeEl.contains(e.relatedTarget)) {
      setDropEl(null);
      setWsDrop(null);
    }
  });

  treeEl.addEventListener('drop', (e) => {
    if (dragWsKey) {
      const target = wsDropOf(e);
      const moving = dragWsKey;
      endWsDrag();
      if (target) {
        e.preventDefault();
        placeWorkspace(moving, target.key, target.after);
      }
      return;
    }
    const box = dropTargetOf(e);
    if (!box) return;
    e.preventDefault();
    const ids = dragIds;
    const wsKey = box.dataset.dropWs;
    const gid = box.dataset.dropGroup;
    endDrag();
    moveSessions(ids, wsKey, gid);
  });

  treeEl.addEventListener('dragend', () => {
    endDrag();
    endWsDrag();
  });
  treeEl.addEventListener('scroll', closeMenu);

  // ---------------------------------------------------------------- olaylar: genel

  searchEl.addEventListener('input', () => {
    query = searchEl.value;
    render();
  });

  refreshBtn.addEventListener('click', () => refresh());
  favToggleBtn.addEventListener('click', () => togglePref('favoritesOnly'));

  menuBtn.addEventListener('click', () => {
    const r = menuBtn.getBoundingClientRect();
    showMenu(r.right, r.bottom + 4, viewMenuItems());
  });

  document.addEventListener('mousedown', (e) => {
    pointerDown = true;
    if (menuEl && !menuEl.contains(e.target)) closeMenu();
  }, true);

  document.addEventListener('mouseup', () => {
    pointerDown = false;
    // click olayı mouseup'tan hemen sonra aynı görevde gelir; bekleyen çizim ondan sonra.
    if (pendingRender) setTimeout(flushPending, 0);
  }, true);

  document.addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input') && !e.target.closest('#term-area')) e.preventDefault();
  });

  document.addEventListener('keydown', (e) => {
    if (e.target instanceof Element && e.target.closest('#term-area')) return;
    const key = e.key.toLowerCase();
    if (e.key === 'Escape') {
      if (menuEl) closeMenu();
      else if (query) {
        searchEl.value = '';
        query = '';
        render();
      } else if (selected.size > 0) {
        selected.clear();
        render();
      }
    } else if (e.ctrlKey && key === 'f') {
      e.preventDefault();
      searchEl.focus();
      searchEl.select();
    } else if (e.key === 'F5' || (e.ctrlKey && key === 'r')) {
      e.preventDefault();
      refresh();
    }
  });

  window.addEventListener('blur', () => {
    closeMenu();
    // Claude öne geldi → açılış bitti.
    if (openingId) setOpening(null);
  });
  window.addEventListener('resize', closeMenu);
  window.addEventListener('focus', () => refresh());
  setInterval(() => {
    if (document.visibilityState === 'visible') refresh();
  }, REFRESH_MS);

  // ---------------------------------------------------------------- başlat

  async function init() {
    try {
      state = normalizeState(await invoke('load_state'));
      stateLoaded = true;
    } catch (err) {
      stateError = 'Grup dosyası okunamadı, değişiklikler KAYDEDİLMEYECEK: ' + err;
    }
    terminals.init({
      onChange: softRender,
      confirm: confirmDialog,
      sideWidth: state.prefs.sideWidth || 0,
      onSideWidth: (w) => {
        state.prefs.sideWidth = Math.round(w);
        scheduleSave();
      },
    });
    applyAlwaysOnTop();
    render();
    await refresh();
  }

  init();
})();
