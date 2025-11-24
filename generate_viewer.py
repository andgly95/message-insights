#!/usr/bin/env python3
"""
Generate an iMessage viewer HTML file with embedded message data.
This script reads all .txt message files and creates a self-contained viewer.html file.
"""

import os
import json
from pathlib import Path

def main():
    # Get the directory where this script is located
    script_dir = Path(__file__).parent
    messages_dir = script_dir / 'messages'

    # Find all .txt files except orphaned.txt
    txt_files = [f for f in messages_dir.glob('*.txt') if f.name != 'orphaned.txt']

    print(f"Found {len(txt_files)} conversation files")

    # Read all message files into a dictionary
    message_data = {}
    for txt_file in txt_files:
        try:
            with open(txt_file, 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()
                message_data[txt_file.name] = content
                print(f"Loaded: {txt_file.name} ({len(content)} bytes)")
        except Exception as e:
            print(f"Error reading {txt_file.name}: {e}")

    # Read the viewer template
    viewer_template_path = script_dir / 'viewer_template.html'
    with open(viewer_template_path, 'r', encoding='utf-8') as f:
        viewer_html = f.read()

    # Replace the empty IMESSAGE_DATA with actual data
    # Find and replace the data injection point
    data_json = json.dumps(message_data, ensure_ascii=False, indent=2)

    # Replace the FIRST occurrence of the empty object with our data
    viewer_html = viewer_html.replace(
        'window.IMESSAGE_DATA = {};',
        f'window.IMESSAGE_DATA = {data_json};',
        1  # Only replace the first occurrence
    )

    # Write the complete viewer
    output_path = script_dir / 'viewer.html'
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(viewer_html)

    print(f"\n✓ Generated viewer with {len(message_data)} conversations")
    print(f"✓ Saved to: {output_path}")
    print(f"\nOpen viewer.html in your browser to view your messages!")

if __name__ == '__main__':
    main()
