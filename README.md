# Ticket-Scanner

A Rust-based QR-code meal ticket scanner designed for a **Raspberry Pi 5** running Raspbian.  
It provides a full-screen, touch-friendly GUI that scans tickets, persists records in SQLite, detects duplicates, and shows a live rate estimate.

---

## Features

| # | Requirement | Implementation |
|---|---|---|
| 1 | Scan ticket → write to DB with timestamp | SQLite via `rusqlite`; timestamp stored as `YYYY-MM-DD HH:MM:SS` |
| 2 | Show total tickets sold + last 10 scanned | Live stat cards updated on every scan |
| 3 | Duplicate detection with alert | Red full-width banner held 8 seconds |
| 4 | Plates / hr from a rolling 10-minute window | `count_last_10_min × 6` recalculated every scan |
| 5 | USB QR-code scanner (HID keyboard) | egui keyboard-event capture; scanner sends digits + Enter |
| 6 | Touch-screen display | Large buttons (≥ 62 px), tap-friendly layout |
| 7 | On-screen number pad for manual entry | 3 × 4 grid + Submit / Clear, works with touch or mouse |
| 8 | Looks good | Dark egui theme, colour-coded cards, animated status banner |

---

## Hardware

* **Scanner** – [LENVII QR/Barcode USB scanner (B0B4VCSMHS)](https://www.amazon.com/dp/B0B4VCSMHS)  
  Acts as a USB HID keyboard; no special driver needed.
* **Display** – 7-inch touch display  
  Works with both X11 and Wayland.

---

## Building on Raspberry Pi 5

### 1. Install system dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libx11-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libxrandr-dev libxi-dev libxcursor-dev \
  libgl1-mesa-dev
```

### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 3. Build

```bash
git clone https://github.com/garrettlanderson/Ticket-Scanner.git
cd Ticket-Scanner
cargo build --release
```

The binary is at `target/release/ticket-scanner`.

### 4. Run

```bash
./target/release/ticket-scanner
```

The database file `tickets.db` is created in the current directory.  
Override the path with the `SCANNER_DB_PATH` environment variable:

```bash
SCANNER_DB_PATH=/home/pi/data/tickets.db ./target/release/ticket-scanner
```

---

## Running as a kiosk on boot (systemd)

Create `/etc/systemd/system/ticket-scanner.service`:

```ini
[Unit]
Description=Ticket Scanner
After=graphical.target

[Service]
User=pi
Environment=DISPLAY=:0
Environment=SCANNER_DB_PATH=/home/pi/tickets.db
ExecStart=/home/pi/Ticket-Scanner/target/release/ticket-scanner
Restart=on-failure

[Install]
WantedBy=graphical.target
```

Then enable it:

```bash
sudo systemctl daemon-reload
sudo systemctl enable ticket-scanner
sudo systemctl start ticket-scanner
```

---

## Usage

1. **USB scanner**: plug in and focus the app window.  
   When a QR code is scanned the number is submitted automatically (the scanner sends Enter).
2. **Manual entry**: tap the number-pad digits on screen, then tap **↵** or **Submit**.
3. **Clear**: tap **✕ Clear** or press Backspace to reset the input field.

### Ticket numbers

Tickets are expected to start at **1000**. Any number below 1000 is rejected with a warning banner.

---

## Database schema

```sql
CREATE TABLE scans (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ticket_number INTEGER NOT NULL UNIQUE,
    scanned_at    TEXT    NOT NULL   -- "YYYY-MM-DD HH:MM:SS"
);
```

You can query the database directly with `sqlite3 tickets.db`.

---

## Project structure

```
src/
  main.rs   – entry point, window configuration
  app.rs    – egui UI, input handling, plates/hr calculation
  db.rs     – SQLite database layer
Cargo.toml
```
