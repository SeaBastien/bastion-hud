import { invoke, Channel } from '@tauri-apps/api/core';

const HISTORY = 60;
const el = (id) => document.getElementById(id);

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function bytes(n) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function duration(seconds) {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return d > 0 ? `${d}d ${h}h ${m}m` : `${h}h ${m}m`;
}

function percent(part, whole) {
  return whole > 0 ? (part / whole) * 100 : 0;
}

/** Draws one or more line series on a canvas, scaled to the largest value. */
function drawGraph(canvas, seriesList, fixedMax) {
  const ratio = window.devicePixelRatio || 1;
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  if (!width || !height) return;
  if (canvas.width !== width * ratio || canvas.height !== height * ratio) {
    canvas.width = width * ratio;
    canvas.height = height * ratio;
  }
  const ctx = canvas.getContext('2d');
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.clearRect(0, 0, width, height);

  const max = fixedMax ?? Math.max(1, ...seriesList.flatMap((series) => series.data));
  for (const { data, color } of seriesList) {
    if (data.length < 2) continue;
    ctx.beginPath();
    data.forEach((value, i) => {
      const x = (i / (HISTORY - 1)) * width;
      const y = height - (Math.min(value, max) / max) * (height - 2) - 1;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }
}

function push(history, value) {
  history.push(value);
  if (history.length > HISTORY) history.shift();
}

function tickClock() {
  const now = new Date();
  el('clock').textContent = now.toLocaleTimeString([], { hour12: false });
  el('date').textContent = now.toLocaleDateString([], {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}

function renderCores(cpus) {
  const container = el('cores');
  if (container.children.length !== cpus.length) {
    container.replaceChildren(
      ...cpus.map(() => {
        const core = document.createElement('div');
        core.className = 'core';
        core.append(document.createElement('i'));
        return core;
      }),
    );
  }
  cpus.forEach((usage, i) => {
    container.children[i].firstChild.style.height = `${Math.min(usage, 100)}%`;
  });
}

function renderDisks(disks) {
  el('disks').replaceChildren(
    ...disks.map((disk) => {
      const used = disk.total - disk.available;
      const wrap = document.createElement('div');
      wrap.className = 'disk';
      const row = document.createElement('div');
      row.className = 'row';
      const name = document.createElement('span');
      name.textContent = disk.mount;
      name.title = `${disk.name} mounted at ${disk.mount}`;
      const size = document.createElement('span');
      size.className = 'dim';
      size.textContent = `${bytes(used)} / ${bytes(disk.total)}`;
      row.append(name, size);
      const bar = document.createElement('div');
      bar.className = 'bar thin';
      const fill = document.createElement('div');
      fill.style.width = `${percent(used, disk.total)}%`;
      bar.append(fill);
      wrap.append(row, bar);
      return wrap;
    }),
  );
}

function renderProcs(procs) {
  el('procs').tBodies[0].replaceChildren(
    ...procs.map((proc) => {
      const tr = document.createElement('tr');
      const cells = [
        [String(proc.pid), ''],
        [proc.name, ''],
        [`${proc.cpu.toFixed(1)}%`, 'num'],
        [bytes(proc.mem), 'num'],
      ];
      for (const [text, cls] of cells) {
        const td = document.createElement('td');
        td.textContent = text;
        if (cls) td.className = cls;
        tr.append(td);
      }
      return tr;
    }),
  );
}

export async function startPanels() {
  tickClock();
  setInterval(tickClock, 1000);

  try {
    const info = await invoke('system_info');
    el('si-host').textContent = info.host;
    el('si-user').textContent = info.user;
    el('si-os').textContent = info.os;
    el('si-kernel').textContent = info.kernel;
    el('si-cpu').textContent = `${info.cpuBrand} (${info.cores})`;
    el('si-cpu').title = info.cpuBrand;
  } catch {
    // panels stay at their placeholders
  }

  const cpuHistory = [];
  const rxHistory = [];
  const txHistory = [];

  const onEvent = new Channel();
  onEvent.onmessage = (s) => {
    el('uptime').textContent = duration(s.uptime);

    el('cpu-pct').textContent = `${s.cpuGlobal.toFixed(0)}%`;
    push(cpuHistory, s.cpuGlobal);
    drawGraph(el('cpu-graph'), [{ data: cpuHistory, color: cssVar('--accent') }], 100);
    renderCores(s.cpus);

    const memPct = percent(s.memUsed, s.memTotal);
    el('mem-pct').textContent = `${memPct.toFixed(0)}%`;
    el('mem-bar').style.width = `${memPct}%`;
    el('mem-text').textContent = `${bytes(s.memUsed)} / ${bytes(s.memTotal)}`;
    el('swap-bar').style.width = `${percent(s.swapUsed, s.swapTotal)}%`;
    el('swap-text').textContent = `swap ${bytes(s.swapUsed)} / ${bytes(s.swapTotal)}`;

    el('net-rx').textContent = `${bytes(s.netRx)}/s`;
    el('net-tx').textContent = `${bytes(s.netTx)}/s`;
    push(rxHistory, s.netRx);
    push(txHistory, s.netTx);
    drawGraph(el('net-graph'), [
      { data: rxHistory, color: cssVar('--accent') },
      { data: txHistory, color: cssVar('--accent-2') },
    ]);

    renderDisks(s.disks);
    renderProcs(s.procs);
  };

  await invoke('stats_subscribe', { onEvent }).catch(() => {});
}
