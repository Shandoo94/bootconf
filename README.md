## 1. Purpose and Scope

**Purpose:** To provide a simple, lightweight and init-agnostic binary that applies declarative machine state (identity and access) to a transient root filesystem (`/etc`) on every boot. The configuration files are supposed to be on a separate partition which is mounted during boot. The intended use case is immutable operating systems based on tools such as `bootc` with `/var` mutable and `/etc` as transient.

**Scope:**
- Parsing structured TOML configuration files.
- Idempotently configuring the system hostname.
- Idempotently configuring the system timezone.
- Idempotently provisioning SSH host keys.
- Idempotently managing local user accounts, password hashes, and SSH authorized keys. 

**Out of Scope:**
- Mounting or unmounting the configuration partition (handled by an external wrapper script).
- Fetching configurations over the network or decrypting secrets (handled during the `dd` installation phase).
- Managing system services or network interfaces.

## 2. Execution Flow (Boot Sequence)

The tool is designed to run extremely early in the boot process (e.g., via an OpenRC `sysinit` script or a systemd unit ordered before `basic.target`).

1. **Pre-requisite:** The OS boot process mounts the `CONFIG` partition (e.g., to `/mnt/config`).
2. **Execution:** The wrapper script invokes the Rust binary: `bootconf apply --dir /mnt/config`.
3. **Processing:** The binary reads the TOML files, compares the desired state against the live transient `/etc`, and writes only the necessary changes.
4. **Cleanup:** The binary exits with code `0` (success) or `>0` (failure). The wrapper script unmounts `/mnt/config` and boot continues.

## 3. Command Line Interface (CLI) Design

The tool will use a subcommand architecture, built using the Rust `clap` crate. This allows modular execution and easy testing.

```text
bootconf [OPTIONS] <SUBCOMMAND>

SUBCOMMANDS:
  host     Apply only the host configuration
           Usage: my-provisioner host --file /mnt/config/host.yaml

  users    Apply only the users configuration
           Usage: my-provisioner users --file /mnt/config/users.yaml
```

## 4. Configuration Schemas (TOML)

### A. Host Configuration (`host.toml`)

Defines the machine's identity on the network.

```toml
# host.toml
hostname = "node-01.local"

[locale]
timezone = "America/New_York"

[ssh_host_keys.ed25519]
public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... root@node-01"
private = """
-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW QyNTUxOQAAACBEABy+F7s1oE9q0LOM0k4l6z4s5aJ/gD+8tWv3vXm4uQAAAJDgY+jG4GPo
...
-----END OPENSSH PRIVATE KEY----- """
```

### B. Users Configuration (`users.toml`)

Defines the local accounts, their authentication methods, and permissions.

```toml
# users.toml
[[users]]
name = "admin"
uid = 1000
groups = ["wheel", "docker"]
shell = "/bin/bash"
password_hash = "$6$rounds=65536$salt$hashedpassword..."
authorized_keys = [ "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... user@laptop" ]

[[users]]
name = "service_account"
uid = 1001
shell = "/sbin/nologin"
```
