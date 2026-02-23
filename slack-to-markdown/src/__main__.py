import argparse
import os
import sys
from datetime import datetime, timezone

from slack_sdk import WebClient

from .config import load_config
from .url_parser import parse_slack_url
from .slack_client import fetch_thread, download_files
from .markdown_writer import generate_markdown


def main():
    parser = argparse.ArgumentParser(
        description="Convert a Slack thread to a markdown file with attachments."
    )
    parser.add_argument("url", help="Slack thread URL")
    parser.add_argument(
        "-o", "--output",
        help="Output directory (default: current directory)",
        default=".",
    )
    args = parser.parse_args()

    # Parse URL
    try:
        ref = parse_slack_url(args.url)
    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

    # Load config
    config = load_config()
    client = WebClient(token=config.token)

    # Fetch thread
    print(f"Fetching thread from channel {ref.channel_id}...")
    thread = fetch_thread(client, ref.channel_id, ref.thread_ts)
    print(f"Found {len(thread.messages)} messages in #{thread.channel_name}")

    # Build output paths
    date_ts = float(thread.thread_ts)
    date_str = datetime.fromtimestamp(date_ts, tz=timezone.utc).strftime("%Y-%m-%d")
    base_name = f"{thread.channel_name}-{date_str}-thread"
    output_dir = os.path.abspath(args.output)
    md_path = os.path.join(output_dir, f"{base_name}.md")
    attachments_dir = os.path.join(output_dir, f"{base_name}-attachments")
    attachments_dirname = f"{base_name}-attachments"

    # Download files
    has_files = any(msg.get("files") for msg in thread.messages)
    file_map = {}
    if has_files:
        os.makedirs(attachments_dir, exist_ok=True)
        print("Downloading attachments...")
        file_map = download_files(thread.messages, attachments_dir, config.token)
        total = sum(len(v) for v in file_map.values())
        errors = sum(1 for v in file_map.values() for f in v if "error" in f)
        print(f"Downloaded {total - errors}/{total} files")

    # Generate markdown
    os.makedirs(output_dir, exist_ok=True)
    md_content = generate_markdown(thread, file_map, args.url, attachments_dirname)
    with open(md_path, "w") as f:
        f.write(md_content)

    print(f"\nSaved: {md_path}")
    if has_files:
        print(f"Attachments: {attachments_dir}")


if __name__ == "__main__":
    main()
