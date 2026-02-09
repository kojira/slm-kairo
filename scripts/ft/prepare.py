"""FTデータの検証・分割スクリプト。

JSONL形式のconversationsデータを検証し、train/validに分割する。
"""

import argparse
import json
import logging
import random
import sys
from pathlib import Path

logger = logging.getLogger(__name__)

REQUIRED_ROLES = {"system", "user", "assistant"}


def validate_entry(entry: dict, line_num: int) -> list[str]:
    """1エントリを検証し、エラーメッセージのリストを返す。"""
    errors = []
    if "messages" not in entry:
        errors.append(f"Line {line_num}: 'messages' key missing")
        return errors

    roles_found = {msg.get("role") for msg in entry["messages"]}
    missing = REQUIRED_ROLES - roles_found
    if missing:
        errors.append(f"Line {line_num}: missing roles: {missing}")
    return errors


def load_and_validate(input_path: Path) -> list[dict]:
    """JSONLファイルを読み込み、全エントリを検証する。"""
    entries = []
    all_errors = []

    with open(input_path, encoding="utf-8") as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError as e:
                all_errors.append(f"Line {i}: invalid JSON: {e}")
                continue
            errors = validate_entry(entry, i)
            if errors:
                all_errors.extend(errors)
            else:
                entries.append(entry)

    if all_errors:
        for err in all_errors:
            logger.error(err)
        raise ValueError(f"Validation failed with {len(all_errors)} error(s)")

    logger.info(f"Validated {len(entries)} entries successfully")
    return entries


def split_data(
    entries: list[dict], train_ratio: float, seed: int = 42
) -> tuple[list[dict], list[dict]]:
    """データをtrain/validに分割する。"""
    shuffled = list(entries)
    random.seed(seed)
    random.shuffle(shuffled)
    split_idx = int(len(shuffled) * train_ratio)
    return shuffled[:split_idx], shuffled[split_idx:]


def write_jsonl(data: list[dict], path: Path) -> None:
    """データをJSONLファイルに書き出す。"""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        for entry in data:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    logger.info(f"Wrote {len(data)} entries to {path}")


def main(
    input_path: str,
    output_dir: str,
    split: float = 0.8,
) -> Path:
    """データを検証・分割してtrain.jsonl/valid.jsonlを出力する。

    Returns:
        output_dirのPath
    """
    input_path = Path(input_path)
    output_dir = Path(output_dir)

    logger.info(f"Loading data from {input_path}")
    entries = load_and_validate(input_path)

    train, valid = split_data(entries, split)
    logger.info(f"Split: train={len(train)}, valid={len(valid)}")

    write_jsonl(train, output_dir / "train.jsonl")
    write_jsonl(valid, output_dir / "valid.jsonl")

    return output_dir


def cli() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prepare FT data")
    parser.add_argument("--input", required=True, help="Input JSONL file")
    parser.add_argument("--output-dir", required=True, help="Output directory")
    parser.add_argument(
        "--split", type=float, default=0.8, help="Train split ratio (default: 0.8)"
    )
    return parser.parse_args()


if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    )
    args = cli()
    try:
        main(input_path=args.input, output_dir=args.output_dir, split=args.split)
    except Exception as e:
        logger.error(f"prepare failed: {e}")
        sys.exit(1)
