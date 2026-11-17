# Bastion HUD

A sci-fi terminal and system monitor for the Bastion distro, inspired by
eDEX-UI. It is written from scratch with Tauri 2 (Rust backend, web
frontend) and uses none of eDEX-UI's code.

Version 0.1 has:

- Terminal tabs running a real shell, with a one-click Kali tab
- Live panels: clock, system details, CPU (total and per core), memory and
  swap, network speed, disks, top processes
- Three colour themes (button in the top bar)

## Status

Versions up to 0.1.1 have been built and run on Windows 11 and on Bastion
(Linux) in a virtual machine.

## Build and run

### On Windows

1. Install [Rust](https://rustup.rs). When it asks, let it install the
   Microsoft C++ Build Tools.
2. Install [Node.js](https://nodejs.org) (LTS).
3. In this folder:

   ```powershell
   npm install
   npm run tauri dev
   ```

The first build downloads and compiles the Rust dependencies and takes
several minutes. Later runs take seconds.

### On Bastion (or any Fedora-based system)

The system image is read-only, so build inside a container:

```bash
distrobox create --name dev --image registry.fedoraproject.org/fedora:latest
distrobox enter dev
sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make nodejs npm
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
npm install
npm run tauri dev
```

Tauri's own list of requirements is at
<https://v2.tauri.app/start/prerequisites/>.

### Make an installer

```bash
npm run tauri build
```

The result lands in `src-tauri/target/release/bundle/`.

## Linux package (built by GitHub)

`.github/workflows/release.yml` builds an RPM on every push to `main` and
uploads it to a release tagged `latest`, always at the same address:

```
https://github.com/USER/bastion-hud/releases/download/latest/bastion-hud.x86_64.rpm
```

The Bastion image installs the app from that address.

## Shell profiles

The buttons in the top bar come from `src-tauri/src/profiles.rs`:

| Profile | Windows | Linux |
| --- | --- | --- |
| Shell | PowerShell | Your login shell (`$SHELL`) |
| Kali | `wsl -d kali-linux` (shown if WSL is installed) | `distrobox enter kali` (shown if distrobox is installed) |

To add a profile, add an entry in that file.

## Security design

eDEX-UI's known vulnerability was a local websocket that any web page
could connect to and send shell commands through. This app is built so
that class of bug cannot occur:

- **No network port.** The window and the backend talk over Tauri's private
  IPC channel. Nothing listens on localhost.
- **The page cannot choose what runs.** It asks for a profile by id. The
  program and arguments for each id are fixed in the Rust backend.
- **No remote content.** The content security policy only allows the app's
  own files. It loads nothing from the internet.
- **Minimal permissions.** `src-tauri/capabilities/default.json` grants the
  window Tauri's core defaults and no plugins (no file system, shell or
  HTTP plugin).

One setting is looser than the default: styles are allowed inline
(`style-src 'unsafe-inline'`, with Tauri's style nonce turned off), because
the terminal component creates its styles at runtime. Scripts stay locked
to the app's own files.

A terminal is a shell by design, so anyone who can use the window can run
commands as you. Run it from a standard account, not an administrator one.

## Layout of the code

| Path | Purpose |
| --- | --- |
| `index.html`, `src/styles.css` | Page structure, look and themes |
| `src/main.js` | Start-up, theme switching, keyboard shortcuts |
| `src/terminal.js` | Terminal tabs (xterm.js) |
| `src/panels.js` | System panels and graphs |
| `src-tauri/src/pty.rs` | Starts shells and streams their output |
| `src-tauri/src/stats.rs` | Collects system statistics |
| `src-tauri/src/profiles.rs` | The list of allowed shells |
| `src-tauri/tauri.conf.json` | Window, security policy, packaging |

## Not in 0.1

The file browser, network globe and on-screen keyboard from eDEX-UI are not
built yet.
