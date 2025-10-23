# Muko

A command-line utility to manage hosts file entries and quickly toggle between development and production DNS settings.

## Name Origin

The name **muko** comes from **SFt** (mukM-gawa), meaning "the other side" in Japanese. This concept is famously featured in the Silent Hill video game series, where characters shift between the normal world and a dark, otherworldly dimension. Similarly, this tool lets you seamlessly switch between the "other side" of your development environment and the production reality.

## Motivation

As a developer, I often need to test applications locally by redirecting production domains to my local development environment. However, constantly editing the hosts file and commenting/uncommenting lines is tedious and error-prone. **Muko** solves this by providing a simple way to toggle domains between DEV mode (using custom IPs) and PROD mode (using real DNS) with a single command.

## Features

- Add domain entries to your hosts file with custom IPs
- Quickly switch domains between DEV and PROD modes
- View all managed domains in a formatted table
- See both dev IPs and resolved production IPs side-by-side
- Automatic DNS resolution to compare dev vs prod IPs
- Tag-based management to avoid interfering with other hosts file entries

## Installation

Install directly from the GitHub repository:

```bash
cargo install --git=https://github.com/shi-yan/muko.git
```

Note: This tool modifies `/etc/hosts`, so you'll need appropriate permissions (typically `sudo`) when running commands that modify the hosts file.

## Usage

### View All Managed Domains

Simply run `muko` without any arguments to see all domains managed by muko:

```bash
muko
```

This displays a table showing:
- **Mode**: DEV (active) or PROD (commented out)
- **Domain**: The domain name
- **Alias**: Optional alias (only shown if different from domain)
- **Dev IP**: The custom IP address used in DEV mode
- **Prod IP**: The actual resolved IP from DNS (shown only in PROD mode)

### Add a Domain

Add a new domain entry to your hosts file:

```bash
# Add with default IP (127.0.0.1)
sudo muko add example.com

# Add with custom IP
sudo muko add example.com --ip 192.168.1.100

# Add with alias
sudo muko add example.com --ip 127.0.0.1 --alias myapp
```

After adding a domain, it's automatically set to DEV mode (active).

### Switch to DEV Mode

Uncomment a domain entry to use the custom dev IP:

```bash
# Using domain name
sudo muko dev example.com

# Using alias
sudo muko dev myapp
```

In DEV mode, the domain will resolve to your custom IP address (e.g., 127.0.0.1).

### Switch to PROD Mode

Comment out a domain entry to use the real production IP:

```bash
# Using domain name
sudo muko prod example.com

# Using alias
sudo muko prod myapp
```

In PROD mode, the domain entry is commented out, so DNS resolution falls back to the real production IP address.

## How It Works

Muko manages hosts file entries by adding a special tag `#muko:` to each line it creates. This allows the tool to:

1. Identify which entries it manages (avoiding conflicts with other tools or manual entries)
2. Store additional metadata like aliases
3. Parse and modify only its own entries

Example hosts file entry managed by muko:

```
127.0.0.1 example.com #muko: myapp
```

When switched to PROD mode, this becomes:

```
#127.0.0.1 example.com #muko: myapp
```

## Examples

### Typical Workflow

```bash
# View current state
muko

# Add a production domain for local development
sudo muko add api.example.com --ip 127.0.0.1 --alias api

# Work on local development
# (api.example.com now points to 127.0.0.1)

# Need to test against production?
sudo muko prod api

# Back to development
sudo muko dev api
```

### Managing Multiple Domains

```bash
# Add multiple services
sudo muko add api.example.com --ip 127.0.0.1 --alias api
sudo muko add web.example.com --ip 127.0.0.1 --alias web
sudo muko add cdn.example.com --ip 127.0.0.1 --alias cdn

# View all at once
muko

# Switch all to prod when needed
sudo muko prod api
sudo muko prod web
sudo muko prod cdn
```

## Requirements

- Rust (for installation)
- Write access to `/etc/hosts` (typically requires `sudo`)

