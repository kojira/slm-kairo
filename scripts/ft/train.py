"""LoRAファインチューニング実行スクリプト。

mlx_lm.lora をsubprocessで呼び出してトレーニングを行う。
"""

import argparse
import logging
import subprocess
import sys
from pathlib import Path

logger = logging.getLogger(__name__)


def main(
    model: str,
    data_dir: str,
    output_dir: str,
    iters: int = 300,
    batch_size: int = 4,
    lora_layers: int = 16,
    lr: float = 1e-5,
) -> Path:
    """mlx_lm.loraを実行してLoRAアダプターを生成する。

    Returns:
        adapters出力ディレクトリのPath
    """
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    cmd = [
        sys.executable,
        "-m",
        "mlx_lm.lora",
        "--model",
        model,
        "--data",
        str(data_dir),
        "--train",
        "--adapter-path",
        str(output_path),
        "--iters",
        str(iters),
        "--batch-size",
        str(batch_size),
        "--num-layers",
        str(lora_layers),
        "--learning-rate",
        str(lr),
    ]

    logger.info(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=False)

    if result.returncode != 0:
        raise RuntimeError(f"mlx_lm.lora failed with exit code {result.returncode}")

    logger.info(f"Training complete. Adapters saved to {output_path}")
    return output_path


def cli() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run LoRA fine-tuning")
    parser.add_argument(
        "--model",
        default="mlx-community/TinySwallow-1.5B-Instruct-8bit",
        help="Base model name",
    )
    parser.add_argument("--data-dir", required=True, help="Data directory")
    parser.add_argument("--output-dir", required=True, help="Adapters output directory")
    parser.add_argument("--iters", type=int, default=300, help="Training iterations")
    parser.add_argument("--batch-size", type=int, default=4, help="Batch size")
    parser.add_argument("--lora-layers", type=int, default=16, help="Number of LoRA layers")
    parser.add_argument("--lr", type=float, default=1e-5, help="Learning rate")
    return parser.parse_args()


if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    )
    args = cli()
    try:
        main(
            model=args.model,
            data_dir=args.data_dir,
            output_dir=args.output_dir,
            iters=args.iters,
            batch_size=args.batch_size,
            lora_layers=args.lora_layers,
            lr=args.lr,
        )
    except Exception as e:
        logger.error(f"train failed: {e}")
        sys.exit(1)
