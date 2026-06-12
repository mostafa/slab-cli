---
name: slab-cli
description: >-
  Interact with Slab knowledge base via the slab CLI. Read, search, create,
  update, and sync documentation posts, topics, and comments. Use when the user
  mentions Slab, knowledge base, documentation workflow, writing or reviewing
  docs in Slab, syncing posts, commenting, or managing a Slab vault.
---

# slab CLI

CLI for Slab knowledge base with a local markdown vault and git-like sync.

## Prerequisites

The `slab` binary must be on PATH. Auth requires `SLAB_API_TOKEN` and `SLAB_TEAM` env vars, or a prior `slab auth login`.

Check readiness before any operation:

```bash
slab auth status
```

If not authenticated, prompt the user for their team subdomain and API token.

## Command Reference

### Auth & Vault

```bash
slab auth login --team acme --token TOKEN
slab auth status
slab auth logout
slab vault init --team acme          # creates ~/.slab/acme/
slab vault info                      # show tracked post count
```

### Reading Posts

```bash
slab post:get POST_ID                # markdown output
slab post:get POST_ID --raw          # raw Quill Delta JSON
slab post:get POST_ID --json         # full post metadata as JSON
slab post:list                       # all posts
slab post:list --topic TOPIC_ID      # posts in a topic
slab ls                              # alias for post:list
```

### Topics

```bash
slab topic:list                      # all topics
slab topic:get TOPIC_ID              # topic details
slab topic:posts TOPIC_ID            # posts in a topic
```

### Searching

```bash
slab search "query"                  # grep local vault (fast)
slab search "query" --remote         # search via Slab API
slab search "query" --json           # structured output
slab search "query" --limit 5        # cap results
```

Local search returns grep-style `path:line: text` output.

### Sync Workflow (pull/status/diff/push)

```bash
slab pull --all                      # pull entire workspace
slab pull --topic TOPIC_ID           # pull one topic
slab pull --post POST_ID             # pull one post
slab status                          # show M/A/D/C per file
slab diff                            # unified diff of modified files
slab diff Topics/Eng/my-rfc.md      # diff a specific file
slab push --all                      # push all modified files
slab push Topics/Eng/my-rfc.md      # push one file
slab push --dry-run                  # preview without pushing
slab push --force                    # overwrite remote on conflict
```

### Writing

```bash
slab post:create "Title" --topic TOPIC_ID --content-file doc.md
echo "# New Post" | slab post:create "Title" --content-file -
slab post:update POST_ID --content-file updated.md
```

### Comments

```bash
slab comment:list POST_ID            # list all comment threads
slab comment:list POST_ID --json     # structured JSON output
slab comment:add POST_ID "Fix the typo in line 3"
slab comment:add POST_ID "Reply" --thread THREAD_ID  # reply to existing thread
slab comment:update COMMENT_ID "Updated text"
slab comment:delete COMMENT_ID
slab comment:react COMMENT_ID "👍"    # react to a comment
slab comment:resolve THREAD_ID       # resolve/close a thread
slab thread:delete THREAD_ID         # delete an entire thread
```

### Links

```bash
slab links Topics/Eng/my-rfc.md     # outgoing links
slab backlinks Topics/Eng/my-rfc.md # incoming links
```

### Utilities

```bash
slab tree                            # show vault directory tree
slab open POST_ID                    # open in browser
slab completions bash                # shell completions
slab api 'QUERY' [--variables JSON]  # raw GraphQL escape hatch (use - for stdin)
```

## Global Flags

| Flag | Effect |
|------|--------|
| `--json` | structured JSON output (always use for parsing) |
| `--format text\|json\|tsv\|md` | output format |
| `--copy` | copy output to clipboard |
| `--team TEAM` | override team (or `SLAB_TEAM` env) |
| `--token TOKEN` | override token (or `SLAB_API_TOKEN` env) |
| `--vault PATH` | override vault path (or `SLAB_VAULT` env) |

## Vault Layout

```
~/.slab/<team>/
  .slab/config.toml                  # auth + endpoint config
  .slab/state.db                     # sync state (sqlite)
  Topics/<TopicName>/<post-title>.md # pulled posts
  Inbox/                             # local drafts (no slab_id)
```

Each post file has YAML frontmatter:

```yaml
---
slab_id: "abc123"
title: "My RFC"
topics:
  - "Engineering"
version: 42
updated_at: "2025-01-15T10:30:00Z"
---
```

The `slab_id` field links the local file to the remote Slab post. Do not remove or alter it.

## Agent Workflows

### Read a specific post and summarize it

```bash
slab post:get POST_ID --json
```

### Find all docs about a topic, then update one

```bash
slab search "deployment" --json
# pick the relevant post ID from results
slab post:get CHOSEN_ID > /tmp/post.md
# edit /tmp/post.md
slab post:update CHOSEN_ID --content-file /tmp/post.md
```

### Full sync cycle (review and edit docs offline)

```bash
slab pull --all
slab search "onboarding"
# edit files directly in ~/.slab/<team>/Topics/...
slab status
slab diff
slab push --dry-run
slab push --all
```

### Create a new document from generated content

```bash
cat <<'EOF' | slab post:create "Deployment Runbook" --topic TOPIC_ID --content-file -
# Deployment Runbook

## Steps
1. ...
EOF
```

### Review comments on a post and respond

```bash
slab comment:list POST_ID --json
# parse threads, identify unresolved ones
slab comment:add POST_ID "Addressed in latest edit" --thread THREAD_ID
slab comment:resolve THREAD_ID
```

## Key Conventions

- Always use `--json` when parsing output programmatically.
- Always run `slab push --dry-run` before `slab push` to preview changes.
- Never use `--force` unless explicitly instructed by the user.
- Local search (`slab search`) requires a prior `slab pull`; use `--remote` for API search without a vault.
- Post IDs can be extracted from Slab URLs: `https://team.slab.com/posts/slug-POSTID` — the last hyphen-separated segment is the ID.
