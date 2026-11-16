import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';
import { invoke, Channel } from '@tauri-apps/api/core';

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function xtermTheme() {
  return {
    background: 'rgba(0,0,0,0)',
    foreground: cssVar('--text'),
    cursor: cssVar('--accent'),
    cursorAccent: cssVar('--bg'),
    selectionBackground: cssVar('--line'),
  };
}

export class TerminalManager {
  constructor({ tabsEl, panesEl, onStatus }) {
    this.tabsEl = tabsEl;
    this.panesEl = panesEl;
    this.onStatus = onStatus;
    this.tabs = [];
    this.active = null;
    this.counter = 0;

    let timer = null;
    new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => this.fitActive(), 60);
    }).observe(panesEl);
  }

  async open(profile) {
    const tab = {
      key: ++this.counter,
      profile,
      ptyId: null,
      alive: true,
      queue: [],
    };

    tab.pane = document.createElement('div');
    tab.pane.className = 'term-pane';
    this.panesEl.append(tab.pane);

    tab.tabEl = document.createElement('div');
    tab.tabEl.className = 'tab';
    tab.tabEl.setAttribute('role', 'tab');
    const label = document.createElement('span');
    label.textContent = `${tab.key}: ${profile.label}`;
    const close = document.createElement('span');
    close.className = 'close';
    close.textContent = '×';
    close.title = 'Close tab';
    tab.tabEl.append(label, close);
    tab.tabEl.addEventListener('click', () => this.activate(tab));
    close.addEventListener('click', (event) => {
      event.stopPropagation();
      this.close(tab);
    });
    this.tabsEl.append(tab.tabEl);

    tab.term = new Terminal({
      allowTransparency: true,
      cursorBlink: true,
      fontFamily: cssVar('--mono'),
      fontSize: 13,
      scrollback: 5000,
      theme: xtermTheme(),
    });
    tab.fit = new FitAddon();
    tab.term.loadAddon(tab.fit);

    this.tabs.push(tab);
    this.activate(tab);
    tab.term.open(tab.pane);
    try {
      // GPU rendering when the webview supports it; the default renderer otherwise.
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      tab.term.loadAddon(webgl);
    } catch {
      // keep the default renderer
    }
    tab.fit.fit();

    // Keystrokes go to the backend. They wait in a queue until the shell exists.
    tab.term.onData((data) => {
      if (!tab.alive) return;
      if (tab.ptyId === null) tab.queue.push(data);
      else invoke('pty_write', { id: tab.ptyId, data }).catch(() => {});
    });
    tab.term.onResize(({ cols, rows }) => {
      if (tab.alive && tab.ptyId !== null) {
        invoke('pty_resize', { id: tab.ptyId, cols, rows }).catch(() => {});
      }
    });

    const onEvent = new Channel();
    onEvent.onmessage = (message) => {
      if (message.event === 'output') {
        tab.term.write(message.data.text);
      } else if (message.event === 'exit') {
        tab.alive = false;
        tab.tabEl.classList.add('dead');
        tab.term.write(`\r\n\x1b[2m[process exited with code ${message.data.code}]\x1b[0m\r\n`);
      }
    };

    try {
      tab.ptyId = await invoke('pty_spawn', {
        profile: profile.id,
        cols: tab.term.cols,
        rows: tab.term.rows,
        onEvent,
      });
      for (const data of tab.queue) {
        await invoke('pty_write', { id: tab.ptyId, data });
      }
      tab.queue = [];
      this.onStatus(`Opened ${profile.label}`);
    } catch (err) {
      tab.alive = false;
      tab.tabEl.classList.add('dead');
      tab.term.write(`\x1b[31mCould not start ${profile.label}: ${err}\x1b[0m\r\n`);
      this.onStatus(`Failed to open ${profile.label}`);
    }

    tab.term.focus();
    return tab;
  }

  activate(tab) {
    for (const other of this.tabs) {
      const isActive = other === tab;
      other.pane.classList.toggle('active', isActive);
      other.tabEl.classList.toggle('active', isActive);
    }
    this.active = tab;
    // Wait a frame so the pane has its size before measuring.
    requestAnimationFrame(() => {
      this.fitActive();
      tab.term.focus();
    });
  }

  fitActive() {
    if (!this.active) return;
    try {
      this.active.fit.fit();
    } catch {
      // pane not laid out yet
    }
  }

  async close(tab) {
    const index = this.tabs.indexOf(tab);
    if (index === -1) return;
    this.tabs.splice(index, 1);
    if (tab.ptyId !== null && tab.alive) {
      await invoke('pty_kill', { id: tab.ptyId }).catch(() => {});
    }
    tab.alive = false;
    tab.term.dispose();
    tab.pane.remove();
    tab.tabEl.remove();
    if (this.active === tab) {
      this.active = null;
      const next = this.tabs[Math.min(index, this.tabs.length - 1)];
      if (next) this.activate(next);
    }
  }

  closeActive() {
    if (this.active) this.close(this.active);
  }

  copySelection() {
    const text = this.active?.term.getSelection();
    if (text) navigator.clipboard?.writeText(text).catch(() => {});
  }

  applyTheme() {
    const theme = xtermTheme();
    for (const tab of this.tabs) tab.term.options.theme = theme;
  }
}
