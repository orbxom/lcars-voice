# Slack-to-Markdown Design

**Date:** 2026-02-24
**Purpose:** Convert a Slack thread URL into a self-contained markdown file with downloaded images/attachments, for feeding into Claude Code.

## Architecture Overview

**`slack-to-markdown/`** - A Python CLI tool that takes a Slack thread URL, fetches all messages and files via the Slack Web API, and outputs a self-contained markdown file with downloaded images/attachments.

### Directory Layout

```
slack-to-markdown/
├── CLAUDE.md              # Architecture guide for Claude Code
├── .env                   # SLACK_USER_TOKEN
├── .env.example           # Template
├── requirements.txt       # slack_sdk, requests
├── setup.sh               # Dependency checker + venv setup
├── src/
│   ├── __main__.py        # CLI entry point (argparse)
│   ├── url_parser.py      # Parse Slack URLs → channel_id + thread_ts
│   ├── slack_client.py    # Fetch thread, resolve users, download files
│   ├── markdown_writer.py # Convert messages → markdown + save files
│   └── config.py          # Load .env, validate token
└── tests/
    ├── test_url_parser.py
    ├── test_slack_client.py
    └── test_markdown_writer.py
```

### Usage

```bash
# Basic — outputs to current directory
python -m src https://myworkspace.slack.com/archives/C12345/p1708789200000001

# With output dir
python -m src https://myworkspace.slack.com/archives/C12345/p1708789200000001 -o ~/slack-threads/
```

## Authentication

- **User OAuth Token (xoxp-)** stored in `.env` as `SLACK_USER_TOKEN`
- Matches the `.env` pattern used in `meeting-transcripts/`
- Gives access to everything the user can see (public, private, DMs)

### Required OAuth Scopes

- `channels:history` — read public channel messages
- `channels:read` — get channel name/info via conversations.info
- `groups:history` — read private channel messages
- `groups:read` — get private channel name/info
- `im:history` — read DM messages
- `mpim:history` — read group DM messages
- `users:read` — resolve user IDs to names (also used for bots.info)
- `files:read` — download files

## URL Parsing

Slack thread URLs come in this format:
```
https://workspace.slack.com/archives/C06ABCDEF/p1708789200123456
```

The `p` prefix timestamp needs conversion: strip `p`, insert a `.` before the last 6 digits → `1708789200.123456`. This gives us `channel_id=C06ABCDEF` and `thread_ts=1708789200.123456`.

## API Call Sequence

1. **`conversations.info`** — Fetch channel metadata (name, is_private, etc.) using the `channel` ID parsed from the URL. Needed for the output filename and markdown header.
2. **`conversations.replies`** — Fetch all messages in the thread using `channel` + `ts`. Set `limit=200` and use **cursor-based pagination** via `response_metadata.next_cursor` (default limit is 100, max is 999). Returns the parent message + all replies.
3. **`users.info`** — For each unique `user` ID in the messages, resolve to `display_name` (or `real_name` as fallback). Cache results to avoid redundant calls.
4. **`bots.info`** — For messages with `bot_id` (no `user` field), resolve bot name via the `bot` parameter. Fall back to `username` field from the message if `bots.info` fails.
5. **File downloads** — Messages contain a `files` array (`files?: File[]`). Each file has `url_private_download`. Download using the token as a Bearer Authorization header. Save to an attachments subdirectory alongside the markdown file.

Rate limiting is handled automatically by `slack_sdk`'s `WebClient`.

## Markdown Output Format

```markdown
# Slack Thread: #channel-name

**Date:** 2026-02-24
**Thread link:** https://workspace.slack.com/archives/C06ABCDEF/p1708789200123456
**Participants:** Alice, Bob, Charlie

---

## Alice — 2:00 PM

This is the parent message that started the thread.

Here's a screenshot of the bug:

![screenshot.png](attachments/screenshot.png)

---

## Bob — 2:05 PM

I see the issue. Here's my analysis:

- The config is wrong
- We need to update the deployment

> quoted text from somewhere

---

## Charlie — 2:10 PM

Fixed it. See the attached log:

[server-output.log](attachments/server-output.log)

---
```

### Slack mrkdwn → Markdown Conversion

Slack uses its own "mrkdwn" format which differs from standard markdown:

- `*bold*` → `**bold**`
- `_italic_` → `*italic*`
- `~strikethrough~` → `~~strikethrough~~`
- `<@U123>` → resolved display name (via users.info cache)
- `<#C123|channel>` → `#channel`
- `<https://example.com|link text>` → `[link text](https://example.com)`
- `<https://example.com>` → `https://example.com`
- Inline code and code blocks use the same backtick syntax — pass through as-is
- Block quotes (`>`) pass through naturally
- Lists pass through naturally

### General Formatting

- `## User — Time` headers make each message scannable
- Horizontal rules separate messages
- Images use standard markdown image syntax (Claude can read these)
- Non-image files get download links

### File Naming

- Output file: `{channel-name}-{date}-thread.md`
- Attachments directory: `{channel-name}-{date}-thread-attachments/`

## Scope

- **Thread replies only** — fetches parent message + all replies for a given thread URL
- Single messages with no replies are exported as-is
- All files (images, documents, binaries) are downloaded locally

## Error Handling

- **Missing/invalid token:** Fail fast with clear message pointing to `.env.example`
- **Permission errors:** Surface Slack API error with scope hint
- **Invalid URL:** Exit with expected format example
- **No replies:** Export the single message — still useful
- **Deleted users / bots:** For users, fall back to raw user ID. For bots, try `bots.info` with the `bot_id` field, then fall back to `username` field from the message, then raw bot_id
- **File download failure:** Log warning, write placeholder `[Failed to download: filename.ext]`, continue with remaining messages
- **Large files:** No size limit — user can ctrl+C if needed

## Dependencies

- `slack_sdk` — official Slack Python SDK (API calls, rate limiting)
- `requests` — file downloads with auth headers
- Python 3.10+ (match existing tools)
