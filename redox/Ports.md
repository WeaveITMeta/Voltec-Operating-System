# V-OS - Port Configuration

## Development Ports

| Service | Default Port | Description |
|---------|--------------|-------------|
| QEMU SSH Forward | 2222 | SSH access to VM guest |
| QEMU HTTP Forward | 8080 | HTTP server in guest |
| QEMU Debug | 1234 | GDB remote debugging |
| VNC Display | 5900 | QEMU VNC display |

## QEMU Port Forwarding

```bash
# SSH access
qemu-system-x86_64 ... -netdev user,id=net0,hostfwd=tcp::2222-:22

# HTTP access
qemu-system-x86_64 ... -netdev user,id=net0,hostfwd=tcp::8080-:80

# Multiple forwards
qemu-system-x86_64 ... -netdev user,id=net0,hostfwd=tcp::2222-:22,hostfwd=tcp::8080-:80
```

## Guest OS Services

| Service | Port | Protocol |
|---------|------|----------|
| SSH | 22 | TCP |
| HTTP | 80 | TCP |
| HTTPS | 443 | TCP |

## Hypervisor Ports (Future)

| Service | Port | Description |
|---------|------|-------------|
| VM Management API | 9000 | REST API for VM control |
| Guest Agent | 9001 | Host-guest communication |
| VNC Proxy | 5900-5999 | Per-VM VNC displays |
