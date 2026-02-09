"""モデルフュージョンスクリプト。

mlx_lm.fuse でアダプターをマージし、tokenizer_config.json を元モデルから復元する。
"""

import argparse
import glob
import logging
import shutil
import subprocess
import sys
from pathlib import Path

logger = logging.getLogger(__name__)


def find_tokenizer_config(model_name: str) -> Path | None:
    """HuggingFaceキャッシュから元モデルのtokenizer_config.jsonを探す。"""
    cache_dir = Path.home() / ".cache" / "huggingface" / "hub"
    # model_name: "org/name" -> "models--org--name"
    dir_name = "models--" + model_name.replace("/", "--")
    pattern = str(cache_dir / dir_name / "snapshots" / "*" / "tokenizer_config.json")

    matches = glob.glob(pattern)
    if not matches:
        logger.warning(f"tokenizer_config.json not found in cache for {model_name}")
        return None

    # 最新のスナップショットを使う
    result = Path(sorted(matches)[-1])
    logger.info(f"Found tokenizer_config.json: {result}")
    return result


def main(
    model: str,
    adapters: str,
    output_dir: str,
) -> Path:
    """アダプターをベースモデルにマージし、tokenizerを修復する。

    Returns:
        マージ済みモデルのPath
    """
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    cmd = [
        sys.executable,
        "-m",
        "mlx_lm.fuse",
        "--model",
        model,
        "--adapter-path",
        str(adapters),
        "--save-path",
        str(output_path),
    ]

    logger.info(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=False)

    if result.returncode != 0:
        raise RuntimeError(f"mlx_lm.fuse failed with exit code {result.returncode}")

    logger.info("Fuse complete. Restoring tokenizer_config.json...")

    src = find_tokenizer_config(model)
    if src:
        dst = output_path / "tokenizer_config.json"
        shutil.copy2(src, dst)
        logger.info(f"Copied tokenizer_config.json to {dst}")
    else:
        logger.warning("Skipping tokenizer_config.json restoration (not found in cache)")

    return output_path


def cli() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Fuse LoRA adapters into base model")
    parser.add_argument(
        "--model",
        default="mlx-community/TinySwallow-1.5B-Instruct-8bit",
        help="Base model name",
    )
    parser.add_argument("--adapters", required=True, help="Adapters directory")
    parser.add_argument("--output-dir", required=True, help="Fused model output directory")
    return parser.parse_args()


if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    )
    args = cli()
    try:
        main(model=args.model, adapters=args.adapters, output_dir=args.output_dir)
    except Exception as e:
        logger.error(f"fuse failed: {e}")
        sys.exit(1)
