"""FTパイプライン統合スクリプト。

prepare → train → fuse → deploy を順に実行する。
"""

import argparse
import logging
import sys
from pathlib import Path

# プロジェクトルートをsys.pathに追加して、単独実行・パッケージ実行の両方に対応
_project_root = str(Path(__file__).resolve().parent.parent.parent)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

from scripts.ft import prepare, train, fuse, deploy

logger = logging.getLogger(__name__)


def main(
    input_path: str,
    base_model: str,
    work_dir: str,
    port: int = 8899,
    venv: str | None = None,
    split: float = 0.8,
    iters: int = 300,
    batch_size: int = 4,
    lora_layers: int = 16,
    lr: float = 1e-5,
) -> None:
    """FTパイプラインを実行する。"""
    work = Path(work_dir)
    data_dir = work / "data"
    adapters_dir = work / "adapters"
    merged_dir = work / "merged"

    steps = [
        (
            "prepare",
            lambda: prepare.main(
                input_path=input_path,
                output_dir=str(data_dir),
                split=split,
            ),
        ),
        (
            "train",
            lambda: train.main(
                model=base_model,
                data_dir=str(data_dir),
                output_dir=str(adapters_dir),
                iters=iters,
                batch_size=batch_size,
                lora_layers=lora_layers,
                lr=lr,
            ),
        ),
        (
            "fuse",
            lambda: fuse.main(
                model=base_model,
                adapters=str(adapters_dir),
                output_dir=str(merged_dir),
            ),
        ),
        (
            "deploy",
            lambda: deploy.main(
                model_path=str(merged_dir),
                port=port,
                venv=venv,
            ),
        ),
    ]

    for step_name, step_fn in steps:
        logger.info(f"=== Step: {step_name} ===")
        try:
            step_fn()
            logger.info(f"=== {step_name}: SUCCESS ===")
        except Exception as e:
            logger.error(f"=== {step_name}: FAILED === {e}")
            sys.exit(1)

    logger.info("Pipeline completed successfully")


def cli() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run full FT pipeline")
    parser.add_argument("--input", required=True, help="Input JSONL file")
    parser.add_argument(
        "--base-model",
        default="mlx-community/TinySwallow-1.5B-Instruct-8bit",
        help="Base model name",
    )
    parser.add_argument("--work-dir", required=True, help="Working directory")
    parser.add_argument("--port", type=int, default=8899, help="vllm-mlx port")
    parser.add_argument("--venv", default=None, help="Path to venv for vllm-mlx")
    parser.add_argument("--split", type=float, default=0.8, help="Train split ratio")
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
            input_path=args.input,
            base_model=args.base_model,
            work_dir=args.work_dir,
            port=args.port,
            venv=args.venv,
            split=args.split,
            iters=args.iters,
            batch_size=args.batch_size,
            lora_layers=args.lora_layers,
            lr=args.lr,
        )
    except SystemExit:
        raise
    except Exception as e:
        logger.error(f"pipeline failed: {e}")
        sys.exit(1)
