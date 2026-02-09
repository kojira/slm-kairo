# SLM-Kairo ⚡

**Small Language Model + 回路 (Circuit)**

Rust製のローカルSLMエージェントフレームワーク。小さなモデルを賢く使う。

## 特徴

- **Best-of-N推論** — N個の候補を並列生成し、評価関数で最良を選択
- **ペルソナ評価関数** — ルールベースの人格準拠スコアリング
- **プラグインアーキテクチャ** — 推論・セッション・評価・ゲートウェイ等すべてプラグイン
- **Config駆動** — TOML設定で挙動を制御、コード変更不要
- **vllm-mlx連携** — Apple Silicon上のMLXモデルをOpenAI互換APIで利用

## アーキテクチャ

```
kairo-core           — プラグインシステム基盤
├── kairo-plugin-inference    — vllm-mlx推論（Best-of-N対応）
├── kairo-plugin-session      — チャンネルベースの会話管理
├── kairo-plugin-evaluator    — ペルソナ準拠スコアリング
├── kairo-plugin-gateway-discord — Discord Gateway（serenity）
├── kairo-plugin-memory-store — メモリストア（未実装）
├── kairo-plugin-memory-search — メモリ検索（未実装）
├── kairo-plugin-tools        — ツール呼び出し（未実装）
└── kairo-plugin-story        — ストーリー記憶（未実装）
```

## クイックスタート

### 1. vllm-mlxでモデル起動

```bash
pip install vllm-mlx
vllm-mlx serve mlx-community/TinySwallow-1.5B-Instruct-4bit --port 8899 --continuous-batching
```

### 2. 設定

`config/default.toml` を編集:

```toml
[plugins.inference]
api_url = "http://localhost:8899/v1"
model = "default"
max_tokens = 150
temperature = 0.7
repetition_penalty = 1.5

[plugins.session]
system_prompt = "あなたのキャラクター設定..."

[plugins.gateway-discord]
token_env = "KAIRO_DISCORD_TOKEN"
best_of_n = 4
```

### 3. 起動

```bash
KAIRO_DISCORD_TOKEN="your-token" cargo run
```

## ベンチマーク（Mac mini M4）

| モデル | 1並列 TPS | 4並列合計 TPS |
|--------|-----------|---------------|
| TinySwallow 1.5B-4bit | 87 | 201 |

Best-of-4が4並列スイートスポット（8並列でも合計スループットほぼ変わらず）。

## ファインチューニング

`mlx-lm` のLoRAでペルソナ特化FTが可能:

```bash
mlx_lm.lora --model mlx-community/TinySwallow-1.5B-Instruct-8bit \
  --data ./ft-data/ --train --iters 300 --adapter-path ./adapters
mlx_lm.fuse --model mlx-community/TinySwallow-1.5B-Instruct-8bit \
  --adapter-path ./adapters --save-path ./my-model
```

## 設計思想

- **小さいモデルを鍛えて使う** — 巨大モデルに頼らず、FT+Best-of-N+評価で品質を担保
- **全てプラグイン** — コア最小化、機能は全てプラグインで提供
- **Config駆動** — 再ビルド不要で挙動変更

## License

MIT

## Author

kojira
