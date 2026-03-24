# Factorio Wake System
Small two-service system for running a Factorio server on demand.

It allows a machine to stay offline by default, be woken remotely, used for a session, and automatically suspended when idle.

---

## Components

### dashboard
Runs on an always-on machine.
- Web UI + API
- Sends Wake-on-LAN packets
- WebSocket updates for live status
- Stores session history (SQLite)

### watcher
Runs on the Factorio machine.
- Tails Factorio logs
- Suspends the machine when idle
- Tracks player join/leave events
- Reports sessions to dashboard

---

## Features

- Wake-on-LAN trigger
- Live machine + game status (TCP probes)
- Real-time updates via WebSockets
- Session tracking (join/leave events)
- Automatic suspend:
  - no players joined → suspend after `FIRST_JOIN_TIMEOUT_SECONDS`
  - players joined and left → suspend after `EMPTY_SERVER_TIMEOUT_SECONDS`

---

## Assumptions

- Linux (systemd)
- Wake-on-LAN already configured and working
- Factorio server logs in standard format
- Two-machine setup:
  - dashboard (always on)
  - Factorio machine (sleep/wake)
- Local network (LAN)

---

## Setup (high-level)
1. Enable Wake-on-LAN on the Factorio machine
2. Configure `.env` files for both services
3. Build:
`cargo build --release`
4. Deploy:
   - dashboard on always-on machine
   - run watcher on Factorio machine
5.	Register systemd services
6.	Allow watcher to suspend the machine (sudoers or root service)
7.	Ensure Factorio logs are accessible

## Development
- `cargo run -p dashboard`
- `cargo run -p watcher`

## Testing
`cargo test`

## Configuration
See `.env.example` in `services/dashboard` and `services/watcher`.

## Notes
This is a minimal, opinionated setup. Not intended as a generic product.

---

# Detailed Setup

This describes how the system is deployed in a typical two-machine setup.

---

## Factorio machine (watcher)
- Enable WoL in BIOS
- Enable it in OS: `sudo ethtool -s <interface> wol g`
- Persist it via systemd or network configuration (required, otherwise WoL may reset after reboot)
- `git clone <repo>`
- `cd <repo>`
- `cargo build --release -p watcher`
- Configure environment. Create: `services/watcher/.env` (see `.env.example`)
  - `LOG_PATH`: path to Factorio logs
  - `DASHBOARD_URL`: IP:PORT or hostname of dashboard (for internal API calls)
  - `INTERNAL_API_TOKEN`: shared secret with dashboard to allow internal API calls
  - `FIRST_JOIN_TIMEOUT_SECONDS`: seconds to wait for first player join before suspending
  - `EMPTY_SERVER_TIMEOUT_SECONDS`: seconds to wait for server after becoming empty before suspending
- Allow suspend
  - Enable suspend for runtime user without a password (Recommended)
    - `sudo visudo`
    - Add this line at the end of the file: `<user> ALL=(root) NOPASSWD: /usr/bin/systemctl suspend`
  - Run watcher service as root.
- Create systemd service (`sudo nano /etc/systemd/system/factorio-watcher.service`):
```
[Unit]
Description=Factorio Watcher
After=network.target

[Service]
Type=simple
User=<user>
Group=<user>
WorkingDirectory=/path/to/repo
ExecStart=/path/to/repo/target/release/watcher
Restart=on-failure
RestartSec=3
EnvironmentFile=/path/to/repo/services/watcher/.env

[Install]
WantedBy=multi-user.target
```

- `sudo systemctl daemon-reload`
- `sudo systemctl enable factorio-watcher`
- `sudo systemctl start factorio-watcher`

---

## Dashboard machine
- `git clone <repo>`
- `cd <repo>`
- `cargo build --release -p dashboard`
- Configure environment. Create: `services/dashboard/.env` (see `.env.example`)
    - `DB_PATH`: path to sqlite database
    - `INTERNAL_API_TOKEN`: shared secret with watcher to allow internal API calls
    - `BIND_ADDR`: Binding target for HTTP server
    - `TARGET_MAC`: MAC address of the machine to wake
    - `MACHINE_IP`: IP address of the Factorio machine
    - `MACHINE_CHECK_PORT`: TCP port to check if machine is up
    - `FACTORIO_CHECK_PORT`: TCP port to check if Factorio is running
- Create systemd service (`sudo nano /etc/systemd/system/factorio-dashboard.service`):
```
[Unit]
Description=Factorio Dashboard
After=network.target

[Service]
Type=simple
WorkingDirectory=/path/to/repo
ExecStart=/path/to/repo/target/release/dashboard
Restart=on-failure
RestartSec=3
EnvironmentFile=/path/to/repo/services/dashboard/.env

[Install]
WantedBy=multi-user.target
```

- `sudo systemctl daemon-reload`
- `sudo systemctl enable factorio-dashboard`
- `sudo systemctl start factorio-dashboard`

---

## Optional: NGINX
Expose dashboard behind nginx and add basic auth if needed. Not included here.

## Verification
1.	Open dashboard in browser
2.	Trigger wake
3.	Machine should power on
4.	Factorio becomes reachable
5.	Join/leave events appear
6.	Machine suspends after idle
