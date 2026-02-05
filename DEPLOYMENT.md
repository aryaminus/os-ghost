# Production Deployment Guide

## Overview

OS-Ghost now supports multiple deployment modes:
- **Desktop Application** (Tauri) - Full GUI with companion
- **Headless Server** - HTTP API for remote access
- **CLI Client** - Command-line interface

## Binary Releases

### Desktop App
```bash
# macOS
./os-ghost.app/Contents/MacOS/os-ghost

# Windows
os-ghost.exe

# Linux
./os-ghost
```

### Server & CLI
```bash
# Start server
./os-ghost-server --port 7842

# Use CLI
./os-ghost-cli -s http://localhost:7843 status
```

## Production Server Configuration

### Environment Variables
```bash
# Required
export OSGHOST_API_KEY="your-secure-api-key-here"

# Optional
export OSGHOST_HOST="0.0.0.0"        # Bind to all interfaces (default: 127.0.0.1)
export OSGHOST_PORT="7842"           # Server port (default: 7842)
export OSGHOST_LOG_LEVEL="info"      # Logging level (default: info)
export OSGHOST_DATA_DIR="/var/lib/os-ghost"  # Data directory
```

### Systemd Service

Create `/etc/systemd/system/os-ghost-server.service`:

```ini
[Unit]
Description=OS-Ghost Headless Server
After=network.target

[Service]
Type=simple
User=os-ghost
Group=os-ghost
WorkingDirectory=/opt/os-ghost
ExecStart=/opt/os-ghost/bin/os-ghost-server --port 7842
Environment="OSGHOST_API_KEY=your-api-key"
Environment="RUST_LOG=info"
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable os-ghost-server
sudo systemctl start os-ghost-server
sudo systemctl status os-ghost-server
```

### Docker Deployment

```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cd src-tauri && cargo build --release --bin os-ghost-server

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=builder /app/src-tauri/target/release/os-ghost-server /usr/local/bin/
COPY --from=builder /app/src-tauri/target/release/os-ghost-cli /usr/local/bin/

EXPOSE 7842
ENV OSGHOST_PORT=7842
ENV OSGHOST_HOST=0.0.0.0

CMD ["os-ghost-server"]
```

Build and run:
```bash
docker build -t os-ghost-server .
docker run -p 7842:7842 -e OSGHOST_API_KEY=your-key os-ghost-server
```

## Security Considerations

### API Authentication
- Always use `OSGHOST_API_KEY` in production
- Generate keys with: `./os-ghost-server --generate-key`
- Rotate keys regularly
- Use HTTPS in production (put behind nginx/caddy)

### CORS Configuration
```bash
# Disable CORS in production (if using same-origin)
./os-ghost-server --no-cors

# Or restrict to specific origins
# (Configure via reverse proxy like nginx)
```

### Input Safety
- All input automation requires explicit user consent
- Dangerous key combinations are blocked (Cmd+Q, Alt+F4)
- Screen coordinates validated
- Rate limiting enforced
- Sensitive text patterns detected

## Monitoring

### Health Check Endpoint
```bash
curl http://localhost:7842/health
```

Response:
```json
{
  "status": "healthy",
  "version": "0.1.41",
  "connected": true,
  "active_agents": 0,
  "pending_actions": 0,
  "workflows_count": 0,
  "memory_entries": 0,
  "timestamp": "2025-02-05T10:00:00Z"
}
```

### Logs
```bash
# View logs
journalctl -u os-ghost-server -f

# Or with RUST_LOG
cd src-tauri
RUST_LOG=debug ./target/release/os-ghost-server
```

## Troubleshooting

### Server Won't Start
```bash
# Check port availability
lsof -i :7842

# Check permissions
ls -la /path/to/os-ghost-server

# Run with verbose logging
RUST_LOG=debug ./os-ghost-server --verbose
```

### CLI Can't Connect
```bash
# Verify server is running
curl http://localhost:7842/health

# Check correct port
./os-ghost-cli -s http://localhost:7842 status

# Verify API key
export OSGHOST_API_KEY="your-key"
```

### Build Issues
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release
```

## API Endpoints

### REST API
- `GET /` - Server info
- `GET /health` - Health check
- `GET /api/v1/status` - Server status
- `POST /api/v1/execute` - Execute task
- `GET /api/v1/workflows` - List workflows
- `POST /api/v1/record/start` - Start recording
- `POST /api/v1/record/stop` - Stop recording

### WebSocket
- `ws://host:port/ws` - Real-time events

## Updating

1. Stop current server
2. Download new release
3. Replace binary
4. Start server
5. Verify with health check

## Support

For issues:
1. Check logs: `journalctl -u os-ghost-server -n 100`
2. Run with debug: `RUST_LOG=debug ./os-ghost-server`
3. Check GitHub Issues
4. Create minimal reproduction case
