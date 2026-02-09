#!/usr/bin/env python3
"""Extract Discord #漫才 channel logs and create multi-turn FT data for FTv4."""

import json
import os
import random
import sys
import time
from datetime import datetime, timezone, timedelta
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.error import HTTPError

# === Config ===
CHANNEL_ID = "1468434240495423572"
KAIRO_BOT_ID = "1470442468473438288"
OTHER_BOTS = {
    "1470003846343295101": "ほわり",
    "1468658779917910191": "らぼみ",
    "1463503026936443009": "のすたろう",
}
WINDOW_MIN = 5
WINDOW_MAX = 10
JST = timezone(timedelta(hours=9))

SYSTEM_PROMPT_TEMPLATE = None  # loaded from config

# === Paths ===
TOKEN_PATH = Path("/Users/kojira/.openclaw/workspace/data/secrets/kairo_discord_token.txt")
CONFIG_PATH = Path("/Users/kojira/.openclaw/workspace/projects/slm-kairo/config/default.toml")
OUTPUT_MULTITURN = Path("/Volumes/1TB/dev/slm-kairo-ft/discord-multiturn.jsonl")
EXISTING_FT = Path("/Volumes/1TB/dev/slm-kairo-ft/ftv3-all.jsonl")
OUTPUT_MERGED = Path("/Volumes/1TB/dev/slm-kairo-ft/ftv4-all.jsonl")
OUTPUT_DIR = Path("/Volumes/1TB/dev/slm-kairo-ft/v4-data")


def load_token():
    return TOKEN_PATH.read_text().strip()


def load_system_prompt():
    """Extract system_prompt from TOML config (simple parser)."""
    content = CONFIG_PATH.read_text()
    in_prompt = False
    lines = []
    for line in content.split("\n"):
        if not in_prompt and "system_prompt" in line and "=" in line:
            # Get value after =
            val = line.split("=", 1)[1].strip()
            if val.startswith('"') and val.endswith('"'):
                return val[1:-1].replace("\\n", "\n")
            else:
                in_prompt = True
                lines.append(val.lstrip('"'))
        elif in_prompt:
            if line.rstrip().endswith('"'):
                lines.append(line.rstrip().rstrip('"'))
                return "\n".join(lines).replace("\\n", "\n")
            lines.append(line)
    raise ValueError("Could not parse system_prompt from config")


def fetch_messages(token, channel_id):
    """Fetch all messages from a Discord channel."""
    all_messages = []
    base_url = f"https://discord.com/api/v10/channels/{channel_id}/messages"
    headers = {"Authorization": f"Bot {token}"}
    before = None
    
    while True:
        url = f"{base_url}?limit=100"
        if before:
            url += f"&before={before}"
        
        req = Request(url, headers={**headers, "User-Agent": "DiscordBot (https://example.com, 1.0)"})
        try:
            with urlopen(req) as resp:
                messages = json.loads(resp.read())
        except HTTPError as e:
            if e.code == 429:
                retry_after = json.loads(e.read()).get("retry_after", 5)
                print(f"Rate limited, waiting {retry_after}s...")
                time.sleep(retry_after)
                continue
            raise
        
        if not messages:
            break
        
        all_messages.extend(messages)
        before = messages[-1]["id"]
        print(f"  Fetched {len(all_messages)} messages so far...")
        time.sleep(1)  # Rate limit respect
    
    # Sort by timestamp (oldest first)
    all_messages.sort(key=lambda m: m["id"])
    return all_messages


def format_message_for_user(msg):
    """Format a message as user role content."""
    author_id = msg["author"]["id"]
    author_name = msg["author"].get("global_name") or msg["author"]["username"]
    content = msg["content"]
    
    if author_id in OTHER_BOTS:
        bot_name = OTHER_BOTS[author_id]
        return f"[BOT] {bot_name}: {content}"
    else:
        return f"{author_name}: {content}"


def format_datetime(timestamp_str):
    """Format Discord timestamp to JST datetime string."""
    # Discord timestamps: 2024-01-15T12:00:00.000000+00:00
    dt = datetime.fromisoformat(timestamp_str)
    dt_jst = dt.astimezone(JST)
    return dt_jst.strftime("%Y年%m月%d日 %H:%M")


def create_samples(messages, system_prompt_template):
    """Create multi-turn FT samples from messages."""
    samples = []
    
    # Find all kairo bot message indices
    kairo_indices = [i for i, m in enumerate(messages) if m["author"]["id"] == KAIRO_BOT_ID]
    
    # Track which messages are "covered" by a kairo response
    covered_until = -1
    
    for idx, kairo_idx in enumerate(kairo_indices):
        msg = messages[kairo_idx]
        assistant_content = msg["content"]
        
        if not assistant_content.strip():
            continue
        
        # Window: take WINDOW_MIN to WINDOW_MAX messages before this kairo message
        window_start = max(0, kairo_idx - WINDOW_MAX)
        window_end = kairo_idx
        
        # Ensure at least WINDOW_MIN if available
        context_msgs = messages[window_start:window_end]
        
        # Skip if no context
        if not context_msgs:
            continue
        
        # Filter out empty messages and other kairo messages in context
        user_messages = []
        for cm in context_msgs:
            if not cm["content"].strip():
                continue
            if cm["author"]["id"] == KAIRO_BOT_ID:
                # Include previous kairo messages as assistant turns? 
                # No - keep them as context in user format
                user_messages.append({"role": "user", "content": format_message_for_user(cm)})
            else:
                user_messages.append({"role": "user", "content": format_message_for_user(cm)})
        
        if not user_messages:
            continue
        
        # Format datetime from the kairo message timestamp
        dt_str = format_datetime(msg["timestamp"])
        system_content = system_prompt_template.replace("{datetime}", dt_str)
        
        sample = {"messages": [
            {"role": "system", "content": system_content},
            *user_messages,
            {"role": "assistant", "content": assistant_content}
        ]}
        samples.append(sample)
        covered_until = kairo_idx
    
    # Generate NO_REPLY samples
    # Find gaps where kairo didn't respond
    # Group consecutive non-kairo messages between kairo responses
    prev_kairo_idx = -1
    for kairo_idx in kairo_indices + [len(messages)]:
        gap_start = prev_kairo_idx + 1
        gap_end = kairo_idx
        
        gap_msgs = [m for m in messages[gap_start:gap_end] 
                     if m["content"].strip() and m["author"]["id"] != KAIRO_BOT_ID]
        
        # If there are enough non-kairo messages in a gap, create NO_REPLY samples
        # Sample every WINDOW_MAX messages in the gap
        if len(gap_msgs) >= WINDOW_MIN:
            for i in range(0, len(gap_msgs) - WINDOW_MIN + 1, WINDOW_MAX):
                chunk = gap_msgs[i:i + WINDOW_MAX]
                if not chunk:
                    continue
                
                user_messages = []
                for cm in chunk:
                    user_messages.append({"role": "user", "content": format_message_for_user(cm)})
                
                dt_str = format_datetime(chunk[-1]["timestamp"])
                system_content = system_prompt_template.replace("{datetime}", dt_str)
                
                sample = {"messages": [
                    {"role": "system", "content": system_content},
                    *user_messages,
                    {"role": "assistant", "content": "NO_REPLY"}
                ]}
                samples.append(sample)
        
        prev_kairo_idx = kairo_idx
    
    return samples


def main():
    print("=== FTv4 Multi-turn Data Extraction ===")
    
    # Load config
    token = load_token()
    system_prompt = load_system_prompt()
    print(f"System prompt loaded ({len(system_prompt)} chars)")
    
    # Fetch messages
    print(f"\nFetching messages from channel {CHANNEL_ID}...")
    messages = fetch_messages(token, CHANNEL_ID)
    print(f"Total messages: {len(messages)}")
    
    kairo_count = sum(1 for m in messages if m["author"]["id"] == KAIRO_BOT_ID)
    print(f"Kairo messages: {kairo_count}")
    
    # Create samples
    print("\nCreating multi-turn samples...")
    samples = create_samples(messages, system_prompt)
    print(f"Generated {len(samples)} samples")
    
    reply_samples = sum(1 for s in samples if s["messages"][-1]["content"] != "NO_REPLY")
    noreply_samples = len(samples) - reply_samples
    print(f"  Reply: {reply_samples}, NO_REPLY: {noreply_samples}")
    
    # Write multi-turn data
    OUTPUT_MULTITURN.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_MULTITURN, "w") as f:
        for s in samples:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")
    print(f"\nWrote {len(samples)} samples to {OUTPUT_MULTITURN}")
    
    # Load existing FT data
    existing = []
    if EXISTING_FT.exists():
        with open(EXISTING_FT) as f:
            for line in f:
                line = line.strip()
                if line:
                    existing.append(json.loads(line))
        print(f"Loaded {len(existing)} existing samples from {EXISTING_FT}")
    
    # Merge and shuffle
    all_samples = existing + samples
    random.seed(42)
    random.shuffle(all_samples)
    print(f"Total merged: {len(all_samples)}")
    
    # Write merged
    with open(OUTPUT_MERGED, "w") as f:
        for s in all_samples:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")
    print(f"Wrote merged to {OUTPUT_MERGED}")
    
    # Train/valid split
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    split_idx = int(len(all_samples) * 0.8)
    train = all_samples[:split_idx]
    valid = all_samples[split_idx:]
    
    with open(OUTPUT_DIR / "train.jsonl", "w") as f:
        for s in train:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")
    
    with open(OUTPUT_DIR / "valid.jsonl", "w") as f:
        for s in valid:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")
    
    print(f"Train: {len(train)}, Valid: {len(valid)}")
    print("\n=== Done! ===")


if __name__ == "__main__":
    main()
