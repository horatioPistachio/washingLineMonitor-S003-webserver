# washingLineMonitor-S003-webserver
Webserver to forward notifications from the line monitor to a phone

---

## Development Workflow: Making Changes and Redeploying

### Overview

The application runs inside a Docker container alongside PostgreSQL and Glances (via `docker-compose.yaml`). When you change source code, the goal is to rebuild and restart **only the app container** — leaving the database and monitoring containers untouched.

### The Rebuild Command

After making changes to any file in `src/`, run:

```bash
docker compose up --build --no-deps -d app
```

**What each flag does:**
- `--build` — forces Docker to rebuild the image for the `app` service before starting it
- `--no-deps` — skips restarting dependency services (`postgres`, `glances`). Without this flag, Docker Compose would also restart everything `app` depends on
- `-d` — detached mode; runs in the background so your terminal is free
- `app` — targets only the `app` service by name

### Why Rebuilds Are Fast (Docker Layer Caching)

The `Dockerfile` is deliberately structured to take advantage of Docker's **layer cache**:

```
# Step 1: copy ONLY Cargo.toml and Cargo.lock, then build dependencies
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release   ← this layer is CACHED after the first build

# Step 2: copy your actual source code
COPY src ./src
RUN cargo build --release   ← only this layer re-runs when src/ changes
```

Docker builds images in layers. Once a layer is built, it is cached on disk. A layer is only invalidated (rebuilt) if the files it depends on have changed. Since `Cargo.toml` and `Cargo.lock` change far less often than `src/`, the expensive step of downloading and compiling all your dependencies is almost always served from cache.

**In practice:**
- First build: slow (downloads and compiles all dependencies — potentially several minutes for a project this size)
- Subsequent builds after changing `src/`: fast (only recompiles your code, ~30–60 seconds)
- After changing `Cargo.toml` (adding a new crate): slow again (dependency layer is invalidated)

### Checking Logs After Redeployment

```bash
# Stream logs from the app container
docker compose logs -f app

# Or check all services at once
docker compose logs -f
```

### Full Stack Teardown and Rebuild

If you need to rebuild everything from scratch (e.g., after a schema change or dependency update):

```bash
docker compose down          # stop and remove all containers
docker compose up --build -d # rebuild all images and start everything
```

> **Note on database data:** `docker compose down` stops containers but does **not** delete the named `postgres_data` volume, so your database data survives. To also wipe the database, use `docker compose down -v` (the `-v` flag removes volumes too).

### Typical Iteration Loop

```
1. Edit src/*.rs in your editor
2. docker compose up --build --no-deps -d app
3. docker compose logs -f app   (watch for startup or panic messages)
4. Test your changes (curl, browser, ESP32 device)
5. Repeat
```

---

## API Endpoints

### General

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | Health check — returns `"Hello, world!"` |

### Devices

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `GET` | `/devices` | — | List all registered devices (returns array of `{ device_id }`) |
| `POST` | `/devices` | `{ "device_id": "...", "configuration": { ... } }` | Register a new device. Returns `201 Created` on success, `409 Conflict` if the device already exists |
| `GET` | `/devices/<device_id>` | — | Get a single device and its configuration. Returns `404` if not found |
| `PATCH` | `/devices/<device_id>` | `{ "device_id": "...", "configuration": { ... } }` | Update a device's configuration. Returns `200 OK` or `404` if not found |
| `DELETE` | `/devices/<device_id>` | — | Remove a device. Returns `204 No Content` or `404` if not found |

### Telemetry

| Method | Path | Body / Query Params | Description |
|--------|------|---------------------|-------------|
| `POST` | `/telemetry` | `{ "device_id": "...", "payload": { ... } }` | Submit a telemetry reading. Stores the record and spawns a background task to analyse the data and send an ntfy.sh notification if stable resistance is detected. Returns `201 Created` |
| `GET` | `/telemetry/<device_id>` | `?start_time=YYYY-MM-DDTHH:MM:SS&end_time=YYYY-MM-DDTHH:MM:SS` | Retrieve telemetry records for a device, ordered by timestamp descending. Both query parameters are optional; omitting them returns all records for the device |

---
