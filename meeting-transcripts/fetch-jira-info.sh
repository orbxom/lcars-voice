#!/bin/bash
# Fetch JIRA information for each transcript and append to markdown files
# Idempotent: removes existing JIRA section and re-fetches

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RECORDINGS_DIR="${1:-$SCRIPT_DIR/recordings}"

# Load config from .env if it exists
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
fi

# JIRA credentials (from .env or environment)
JIRA_URL="${JIRA_URL:-https://designcrowd.atlassian.net}"
JIRA_USER="${JIRA_USER:?JIRA_USER is required - set in .env or environment}"
JIRA_TOKEN="${JIRA_TOKEN:?JIRA_TOKEN is required - set in .env or environment}"

# Section divider for idempotency
JIRA_SECTION_START="<!-- JIRA_INFO_START -->"
JIRA_SECTION_END="<!-- JIRA_INFO_END -->"

fetch_issue() {
    local issue_key="$1"
    curl -s -u "$JIRA_USER:$JIRA_TOKEN" \
        -H "Content-Type: application/json" \
        "$JIRA_URL/rest/api/3/issue/$issue_key"
}

fetch_comments() {
    local issue_key="$1"
    curl -s -u "$JIRA_USER:$JIRA_TOKEN" \
        -H "Content-Type: application/json" \
        "$JIRA_URL/rest/api/3/issue/$issue_key/comment"
}

download_attachment() {
    local url="$1"
    local output_path="$2"
    curl -s -L -o "$output_path" \
        -u "$JIRA_USER:$JIRA_TOKEN" \
        "$url"
}

process_markdown_file() {
    local md_file="$1"
    local filename=$(basename "$md_file" .md)

    # Skip RecordingIndex.md
    if [[ "$filename" == "RecordingIndex" ]]; then
        return
    fi

    echo "Processing: $filename"

    # Extract JIRA IDs from filename (handles GT-1234-GT-5678 format)
    IFS='-' read -ra parts <<< "$filename"

    jira_ids=()
    i=0
    while [ $i -lt ${#parts[@]} ]; do
        if [[ "${parts[$i]}" =~ ^[A-Z]+$ ]] && [ $((i+1)) -lt ${#parts[@]} ] && [[ "${parts[$((i+1))]}" =~ ^[0-9]+$ ]]; then
            jira_ids+=("${parts[$i]}-${parts[$((i+1))]}")
            i=$((i+2))
        else
            i=$((i+1))
        fi
    done

    if [ ${#jira_ids[@]} -eq 0 ]; then
        echo "  WARNING: No JIRA IDs found in filename"
        return
    fi

    echo "  Found JIRA IDs: ${jira_ids[*]}"

    # Remove existing JIRA section if present
    if grep -q "$JIRA_SECTION_START" "$md_file"; then
        echo "  Removing existing JIRA section..."
        sed -i "/$JIRA_SECTION_START/,/$JIRA_SECTION_END/d" "$md_file"
    fi

    # Create attachments directory
    attachments_dir="$RECORDINGS_DIR/attachments/$filename"
    mkdir -p "$attachments_dir"

    # Start JIRA section
    {
        echo ""
        echo "$JIRA_SECTION_START"
        echo ""
        echo "---"
        echo ""
        echo "# JIRA Information"
        echo ""
    } >> "$md_file"

    for jira_id in "${jira_ids[@]}"; do
        echo "  Fetching $jira_id..."

        issue_json=$(fetch_issue "$jira_id")

        # Check for errors
        if echo "$issue_json" | grep -q '"errorMessages"'; then
            echo "  ERROR: Could not fetch $jira_id"
            echo "Error fetching $jira_id" >> "$md_file"
            echo "$issue_json" | head -c 200
            continue
        fi

        if [[ -z "$issue_json" || "$issue_json" == "null" ]]; then
            echo "  ERROR: Empty response for $jira_id"
            echo "Error: Empty response for $jira_id" >> "$md_file"
            continue
        fi

        comments_json=$(fetch_comments "$jira_id")

        # Download attachments (images)
        echo "  Checking attachments..."
        attachment_list=$(echo "$issue_json" | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    for att in data.get('fields', {}).get('attachment', []):
        print(att.get('filename', '') + '|' + att.get('content', ''))
except Exception as e:
    pass
" 2>/dev/null || echo "")

        if [[ -n "$attachment_list" ]]; then
            while IFS='|' read -r att_filename att_url; do
                if [[ -n "$att_filename" && -n "$att_url" ]]; then
                    echo "    Downloading: $att_filename"
                    download_attachment "$att_url" "$attachments_dir/$att_filename"
                fi
            done <<< "$attachment_list"
        fi

        # Format and append JIRA info using Python
        echo "  Writing JIRA info..."
        python3 - "$issue_json" "$comments_json" "$JIRA_URL" "$jira_id" "$filename" >> "$md_file" << 'PYTHON_SCRIPT'
import json
import sys

issue_json_str = sys.argv[1]
comments_json_str = sys.argv[2]
JIRA_URL = sys.argv[3]
issue_key = sys.argv[4]
filename = sys.argv[5]

try:
    issue = json.loads(issue_json_str)
    comments_data = json.loads(comments_json_str) if comments_json_str else {"comments": []}
except json.JSONDecodeError as e:
    print(f"Error parsing JSON for {issue_key}: {e}")
    sys.exit(0)

fields = issue.get('fields', {})

def extract_adf_text(node, depth=0):
    if depth > 10:
        return ''
    if node is None:
        return ''
    if isinstance(node, str):
        return node
    if isinstance(node, list):
        return ''.join(extract_adf_text(n, depth+1) for n in node)
    if isinstance(node, dict):
        node_type = node.get('type', '')
        text = ''
        if node_type == 'text':
            text = node.get('text', '')
        elif node_type == 'hardBreak':
            text = '\n'
        elif node_type == 'paragraph':
            text = extract_adf_text(node.get('content', []), depth+1) + '\n\n'
        elif node_type == 'listItem':
            text = '- ' + extract_adf_text(node.get('content', []), depth+1)
        elif node_type == 'bulletList' or node_type == 'orderedList':
            text = extract_adf_text(node.get('content', []), depth+1) + '\n'
        elif node_type == 'heading':
            level = node.get('attrs', {}).get('level', 3)
            text = '#' * level + ' ' + extract_adf_text(node.get('content', []), depth+1) + '\n\n'
        elif node_type == 'codeBlock':
            text = '```\n' + extract_adf_text(node.get('content', []), depth+1) + '\n```\n\n'
        elif 'content' in node:
            text = extract_adf_text(node['content'], depth+1)
        return text
    return ''

# Basic info
print(f"## {issue.get('key', 'Unknown')}")
print()
print(f"**Summary:** {fields.get('summary', 'N/A')}")
print()
print(f"**Status:** {fields.get('status', {}).get('name', 'N/A')}")
print()
print(f"**Issue Type:** {fields.get('issuetype', {}).get('name', 'N/A')}")
print()

# Priority
priority = fields.get('priority')
if priority:
    print(f"**Priority:** {priority.get('name', 'N/A')}")
    print()

# Assignee
assignee = fields.get('assignee')
if assignee:
    print(f"**Assignee:** {assignee.get('displayName', 'Unassigned')}")
else:
    print("**Assignee:** Unassigned")
print()

# Reporter
reporter = fields.get('reporter')
if reporter:
    print(f"**Reporter:** {reporter.get('displayName', 'N/A')}")
    print()

# Labels
labels = fields.get('labels', [])
if labels:
    print(f"**Labels:** {', '.join(labels)}")
    print()

# Components
components = fields.get('components', [])
if components:
    comp_names = [c.get('name', '') for c in components]
    print(f"**Components:** {', '.join(comp_names)}")
    print()

# Parent (for subtasks or issues in epics)
parent = fields.get('parent')
if parent:
    parent_key = parent.get('key', '')
    parent_summary = parent.get('fields', {}).get('summary', '')
    print(f"**Parent:** [{parent_key}]({JIRA_URL}/browse/{parent_key}) - {parent_summary}")
    print()

# Subtasks
subtasks = fields.get('subtasks', [])
if subtasks:
    print("### Subtasks")
    print()
    for st in subtasks:
        st_key = st.get('key', '')
        st_summary = st.get('fields', {}).get('summary', '')
        st_status = st.get('fields', {}).get('status', {}).get('name', '')
        print(f"- [{st_key}]({JIRA_URL}/browse/{st_key}): {st_summary} ({st_status})")
    print()

# Description
description = fields.get('description')
if description:
    print("### Description")
    print()
    desc_text = extract_adf_text(description)
    print(desc_text.strip())
    print()

# Attachments
attachments = fields.get('attachment', [])
if attachments:
    print("### Attachments")
    print()
    for att in attachments:
        att_filename = att.get('filename', '')
        mime = att.get('mimeType', '')
        if mime.startswith('image/'):
            print(f"![{att_filename}](attachments/{filename}/{att_filename})")
        else:
            print(f"- [{att_filename}](attachments/{filename}/{att_filename}) ({mime})")
    print()

# Comments
comments = comments_data.get('comments', [])
if comments:
    print("### Comments")
    print()
    for comment in comments:
        author = comment.get('author', {}).get('displayName', 'Unknown')
        created = comment.get('created', '')[:10]
        body = comment.get('body', {})
        body_text = extract_adf_text(body)
        print(f"**{author}** ({created}):")
        print()
        print(body_text.strip())
        print()
        print("---")
        print()

# Link to issue
print(f"[View in JIRA]({JIRA_URL}/browse/{issue_key})")
print()
PYTHON_SCRIPT

    done

    # End JIRA section
    echo "$JIRA_SECTION_END" >> "$md_file"

    echo "  Done!"
}

echo "Fetching JIRA information..."
echo "============================"

# Process all markdown files in recordings directory
for md_file in "$RECORDINGS_DIR"/*.md; do
    if [[ -f "$md_file" ]]; then
        process_markdown_file "$md_file"
    fi
done

echo ""
echo "============================"
echo "JIRA fetch complete!"
