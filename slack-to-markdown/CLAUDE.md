# CLAUDE.md

This file provides guidance to Claude Code when working with code in this project.

## Project Overview

CLI tool to convert a Slack thread URL into a self-contained markdown file with downloaded images/attachments. Designed for feeding Slack conversations into Claude Code.

## Running

```bash
# First-time setup
bash setup.sh

# Convert a Slack thread
.venv/bin/python -m src https://workspace.slack.com/archives/C12345/p1708789200000001

# With custom output directory
.venv/bin/python -m src <url> -o ~/slack-threads/

# Run tests
.venv/bin/python -m pytest tests/ -v
```

## Architecture

```
src/
├── __main__.py        CLI entry point (argparse)
├── url_parser.py      Parse Slack URLs → channel_id + thread_ts
├── slack_client.py    Fetch thread, resolve users/bots, download files
├── markdown_writer.py Slack mrkdwn → Markdown conversion + document generation
└── config.py          Load .env, validate SLACK_USER_TOKEN
```

**Data flow:** URL parsed → conversations.info (channel name) → conversations.replies (all messages, paginated) → users.info/bots.info (resolve names) → download files → generate markdown → write to disk.

## Output Format

- Markdown file: `{channel-name}-{date}-thread.md`
- Attachments directory: `{channel-name}-{date}-thread-attachments/`
- Images embedded as `![name](attachments/name)`, other files as `[name](attachments/name)`
- Each message has a `## User — Time` header with horizontal rule separators
- Slack mrkdwn converted to standard markdown (bold, italic, strikethrough, links, mentions)

## Slack App Setup

Create a Slack app at https://api.slack.com/apps with these **User Token Scopes**:

`channels:history`, `channels:read`, `groups:history`, `groups:read`, `im:history`, `mpim:history`, `users:read`, `files:read`

Install the app to your workspace, copy the `xoxp-...` User OAuth Token, and put it in `.env`.

## Key Conventions

- All Slack API calls go through `slack_sdk.WebClient` (handles rate limiting)
- File downloads use `requests` with Bearer token auth
- Tests mock all API calls — no real Slack access needed
- Config via `.env` file with `SLACK_USER_TOKEN`

## Known Limitations

- User mentions in message text (`<@U123>`) are only resolved if that user also posted in the thread. Mentions of non-participants render as raw user IDs.
- `%-I` strftime format (no zero-padding on hours) is Linux-only.

## Dependencies

Python packages: `slack_sdk`, `requests`, `python-dotenv`. See `requirements.txt`.
