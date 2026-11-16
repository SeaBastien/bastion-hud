import { invoke } from '@tauri-apps/api/core';
import { TerminalManager } from './terminal.js';
import { startPanels } from './panels.js';

const THEMES = ['tron', 'matrix', 'ember'];
const root = document.documentElement;

function setStatus(text) {
  document.getElementById('status-left').textContent = text;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function bootSequence() {
  const log = document.getElementById('boot-log');
  const lines = [
    'BASTION HUD v0.1.1',
    'linking interface ......... ok',
    'starting system monitors .. ok',
    'opening shell ............. ok',
  ];
  for (const line of lines) {
    log.textContent += line + '\n';
    await sleep(160);
  }
  await sleep(220);
  document.getElementById('boot').remove();
  document.getElementById('app').hidden = false;
}

function loadTheme() {
  let saved = null;
  try {
    saved = localStorage.getItem('bastion-theme');
  } catch {
    // storage unavailable: fall back to the default theme
  }
  if (THEMES.includes(saved)) root.dataset.theme = saved;
}

function nextTheme() {
  const index = THEMES.indexOf(root.dataset.theme);
  const theme = THEMES[(index + 1) % THEMES.length];
  root.dataset.theme = theme;
  try {
    localStorage.setItem('bastion-theme', theme);
  } catch {
    // ignore: the theme still applies for this session
  }
}

async function main() {
  loadTheme();
  await bootSequence();

  const terminals = new TerminalManager({
    tabsEl: document.getElementById('tabs'),
    panesEl: document.getElementById('terms'),
    onStatus: setStatus,
  });

  // The backend decides which shells exist. The page only picks one by id.
  let profiles = [];
  try {
    profiles = await invoke('list_profiles');
  } catch (err) {
    setStatus(`Could not load shell profiles: ${err}`);
  }

  const profilesEl = document.getElementById('profiles');
  for (const profile of profiles) {
    const button = document.createElement('button');
    button.className = 'btn';
    button.textContent = `+ ${profile.label}`;
    button.title = `Open a new ${profile.label} tab`;
    button.addEventListener('click', () => terminals.open(profile));
    profilesEl.append(button);
  }

  document.getElementById('theme-btn').addEventListener('click', () => {
    nextTheme();
    terminals.applyTheme();
  });

  window.addEventListener(
    'keydown',
    (event) => {
      if (!(event.ctrlKey && event.shiftKey)) return;
      if (event.code === 'KeyT' && profiles.length) {
        event.preventDefault();
        terminals.open(profiles[0]);
      } else if (event.code === 'KeyW') {
        event.preventDefault();
        terminals.closeActive();
      } else if (event.code === 'KeyC') {
        event.preventDefault();
        terminals.copySelection();
      }
    },
    true,
  );

  startPanels();
  if (profiles.length) await terminals.open(profiles[0]);
}

main();
