"""vllm-mlxデプロイスクリプト。

既存プロセスを停止し、新モデルでvllm-mlxを起動、ヘルスチェックを行う。
"""

import argparse
import logging
import subprocess
import sys
import time

logger = logging.getLogger(__name__)

DEFAULT_PORT = 8899
HEALTH_RETRIES = 15
HEALTH_INTERVAL = 2  # seconds


def kill_existing() -> None:
    """既存のvllm-mlxプロセスをkillする。"""
    logger.info("Stopping existing vllm-mlx processes...")
    subprocess.run(["pkill", "-f", "vllm_mlx"], check=False)
    time.sleep(1)


def start_server(model_path: str, port: int, venv: str | None = None) -> subprocess.Popen:
    """vllm-mlxをバックグラウンドで起動する。"""
    if venv:
        python = f"{venv}/bin/python"
    else:
        python = sys.executable

    cmd = [
        python,
        "-m",
        "vllm_mlx",
        "--model",
        model_path,
        "--port",
        str(port),
    ]

    logger.info(f"Starting vllm-mlx: {' '.join(cmd)}")
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    logger.info(f"vllm-mlx started (PID: {proc.pid})")
    return proc


def health_check(port: int) -> bool:
    """サーバーのヘルスチェックを行う。"""
    import urllib.request
    import urllib.error

    url = f"http://localhost:{port}/v1/models"

    for attempt in range(1, HEALTH_RETRIES + 1):
        try:
            req = urllib.request.Request(url)
            with urllib.request.urlopen(req, timeout=5) as resp:
                if resp.status == 200:
                    logger.info(f"Health check passed (attempt {attempt})")
                    return True
        except (urllib.error.URLError, OSError):
            logger.info(f"Health check attempt {attempt}/{HEALTH_RETRIES} - waiting...")
            time.sleep(HEALTH_INTERVAL)

    logger.error("Health check failed after all retries")
    return False


def main(
    model_path: str,
    port: int = DEFAULT_PORT,
    venv: str | None = None,
) -> bool:
    """vllm-mlxを再起動してヘルスチェックする。

    Returns:
        ヘルスチェック成功ならTrue
    """
    kill_existing()
    proc = start_server(model_path, port, venv)

    if not health_check(port):
        proc.kill()
        raise RuntimeError("vllm-mlx failed to start (health check failed)")

    logger.info(f"vllm-mlx is running on port {port} (PID: {proc.pid})")
    return True


def cli() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Deploy model with vllm-mlx")
    parser.add_argument("--model-path", required=True, help="Path to fused model")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help="Server port")
    parser.add_argument("--venv", default=None, help="Path to venv for vllm-mlx")
    return parser.parse_args()


if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    )
    args = cli()
    try:
        main(model_path=args.model_path, port=args.port, venv=args.venv)
    except Exception as e:
        logger.error(f"deploy failed: {e}")
        sys.exit(1)
