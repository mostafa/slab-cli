# slab-cli

CLI for [Slab](https://slab.com) knowledge base to read, search, sync, and push documentation from the terminal.

Built for integration with AI coding agents (Claude, Cursor, etc.) to automate documentation workflows.

## Features

- **Read & search** posts and topics via Slab's GraphQL API
- **Local vault**: pull your entire Slab workspace as markdown files with YAML frontmatter
- **Sync**: git-like `pull` / `status` / `diff` / `push` workflow
- **Delta ↔ Markdown**: automatic conversion between Slab's Quill Delta format and Markdown
- **Comments & reactions**: list, add, update, delete, resolve threads, react with emoji
- **Agent-friendly**: `--json` output, `--copy` to clipboard, env var config, stdin content pipes

## Install

```bash
cargo install --path crates/slab-cli
```

## Quick start

```bash
# 1. Authenticate
export SLAB_API_TOKEN="your-token-here"
slab auth login --team acme

# 2. Initialize local vault & pull all posts
slab vault init --team acme
slab pull --all

# 3. Browse locally
slab tree
slab search "onboarding"
slab status

# 4. Edit a post and push back
vim ~/.slab/acme/Topics/Engineering/my-rfc.md
slab diff
slab push --all
```

## Commands

| Command | Description |
|---------|-------------|
| `slab auth login\|status\|logout` | Manage API credentials |
| `slab vault init\|info` | Initialize / inspect local vault |
| `slab post:get <id>` | Fetch a post (markdown output) |
| `slab post:list [--topic=ID]` | List posts |
| `slab post:create <title>` | Create a new post |
| `slab post:update <id>` | Update a post |
| `slab topic:list` | List all topics |
| `slab topic:get <id>` | Get topic details |
| `slab topic:posts <id>` | List posts in a topic |
| `slab search <query>` | Search local vault (grep-style) |
| `slab search <query> --remote` | Search via Slab API |
| `slab ls` | Alias for `post:list` |
| `slab tree` | Show vault directory tree |
| `slab pull [--all\|--topic=\|--post=]` | Pull posts from Slab |
| `slab status` | Show modified/added/deleted files |
| `slab diff [file]` | Show local changes |
| `slab push [file\|--all]` | Push changes to Slab |
| `slab comment:list <post-id>` | List comment threads on a post |
| `slab comment:add <post-id> <text>` | Add a comment (new thread) |
| `slab comment:update <comment-id> <text>` | Edit a comment |
| `slab comment:delete <comment-id>` | Delete a comment |
| `slab comment:react <comment-id> <emoji>` | React to a comment |
| `slab comment:resolve <thread-id>` | Resolve a comment thread |
| `slab thread:delete <thread-id>` | Delete an entire thread |
| `slab links <file>` | Show outgoing links |
| `slab backlinks <file>` | Show incoming links |
| `slab open <id>` | Open post in browser |
| `slab completions <shell>` | Generate shell completions |

## Global options

| Flag | Description |
|------|-------------|
| `--team <TEAM>` | Slab team subdomain (`SLAB_TEAM` env) |
| `--token <TOKEN>` | API token (`SLAB_API_TOKEN` env) |
| `--vault <PATH>` | Vault path (`SLAB_VAULT` env) |
| `--format text\|json\|tsv\|md` | Output format |
| `--json` | Shorthand for `--format=json` |
| `--copy` | Copy output to clipboard |

## Vault layout

```
~/.slab/<team>/
  .slab/
    config.toml
    state.db
  Topics/
    Engineering/
      atlas-rfc.md
    Handbook/
      onboarding.md
  Inbox/                    # local drafts (not yet pushed)
```

Each markdown file has YAML frontmatter:

```yaml
---
slab_id: "abc123"
title: "Atlas RFC"
topics:
  - "Engineering"
version: 42
updated_at: "2025-01-15T10:30:00Z"
---
```

## Agent integration

### Claude Code / Cursor

Set environment variables and call commands directly:

```bash
export SLAB_API_TOKEN="..."
export SLAB_TEAM="acme"

# Agent reads a post
slab post:get abc123 --json

# Agent searches
slab search "deployment" --json

# Agent creates a post from stdin
echo "# New RFC\n\nContent here" | slab post:create "My RFC" --topic=eng --content-file=-
```

### Comments

```bash
# List comment threads on a post
slab comment:list abc123

# Add a comment to a post
slab comment:add abc123 "Looks good, minor typo in section 3"

# Reply to an existing thread
slab comment:add abc123 "Fixed, thanks!" --thread plco0ho0

# React, then resolve the thread
slab comment:react k4vjdkoz "👍"
slab comment:resolve plco0ho0
```

## Requirements

- Rust 1.88+
- Slab Business or Enterprise plan (API access)
- API token from Slab Team Settings → Developer

## License

MIT
