# SLM-Kairo — Rust製エージェントフレームワーク 設計書

> v0.6 — 2026-02-10
> v0.1 — 2026-02-09

---

## 変更履歴

| バージョン | 日付 | 変更内容 |
|-----------|------|---------|
| v0.1 | 2026-02-09 | 初版 |
| v0.2 | 2026-02-09 | 名前変更(SLM-Kairo)、評価関数強化、プラグイン拡張、安全な再起動機構、Gateway プラガブル化、ツールコーリングFT設計、サブエージェント機構、SOUL.md実装、ハートビート実装、メモリーシステム設計、Memory Compaction追加 |
| v0.3 | 2026-02-09 | 動的タグ付けシステム、ストーリー記憶（エピソード記憶）、スキル自動開発、機能間連携 |
| v0.4 | 2026-02-09 | Gateway階層化（HTTP基盤+アダプタ）、SOUL 2層構造（コア+拡張）・TOML化、ブラウザ管理画面、評価関数見直し（構造スコア廃止・ルール準拠評価追加）、高速モード難易度判定、無限ループ防止改善、LLMバックエンド抽象化（peer層）、ホームディレクトリ制御、バッチ処理動的調整、引き継ぎ再起動検証強化、サブエージェント別プロセス化+watchdog、要約ペルソナ反映強化、タグ付けジョブ改善（logprobs活用検討）、エピソード記憶DB化 |
| v0.5 | 2026-02-09 | プラグインアーキテクチャへの全面移行（kairo-core最小化、全機能プラグイン化、Plugin trait統一、自己書き換え対応）、NostrAdapter秘密鍵除去（認証プラグイン分離）、エピソード記憶保存フォーマットJSON化（Markdown+YAML Front Matter廃止） |
| v0.6 | 2026-02-10 | Self-Reinforcement Fine-Tuning（自己強化学習）追加 — 自己評価ループ、FTパイプライン、安全性設計、段階的導入計画 |

---

## 名前

**SLM-Kairo** （SLM = Small Language Model、Kairo = 回路）

小規模言語モデルを活用した思考回路エージェント。

---

## 1. アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────┐
│             Adapter Layer (Discord / Nostr / ...)        │
│                        ▼                                 │
│          HTTP Gateway (ベースレイヤー / axum)             │
│  受信 → セッション解決 → レスポンス送信 → 拡張(reaction等)│
└──────────────┬──────────────────────────┬───────────────┘
               │                          ▲
               ▼                          │
┌──────────────────────────┐   ┌─────────────────────────┐
│    Session Manager        │   │    Response Formatter    │
│  コンテキスト構築/保持    │   │  Gateway向け整形         │
│  チャンネルグルーピング   │   └─────────────────────────┘
│  SOUL.md ペルソナ適用     │              ▲
└──────────────┬───────────┘              │
               │                          │
               ▼                          │
┌─────────────────────────────────────────────────────────┐
│                  Inference Orchestrator                   │
│                                                          │
│  ┌─────────────────────────────────────────────┐        │
│  │          Best-of-N Parallel Runner           │        │
│  │                                              │        │
│  │  ┌──────┐ ┌──────┐ ┌──────┐     ┌──────┐   │        │
│  │  │ LLM  │ │ LLM  │ │ LLM  │ ... │ LLM  │   │        │
│  │  │ #1   │ │ #2   │ │ #3   │     │ #8   │   │        │
│  │  └──┬───┘ └──┬───┘ └──┬───┘     └──┬───┘   │        │
│  │     └────────┴────────┴─────┬──────┘        │        │
│  │                             ▼               │        │
│  │                     Evaluator               │        │
│  │                     (最良候補選定)            │        │
│  │                         │                   │        │
│  │                         ▼                   │        │
│  │                   Refinement Pass           │        │
│  │                   (再推論 → 最終結果)        │        │
│  └─────────────────────────────────────────────┘        │
│                          │                               │
│      ┌───────────────────┼──────────────────┐           │
│      ▼             ▼              ▼         ▼           │
│  ┌──────────┐ ┌─────────┐ ┌──────────┐ ┌──────────┐   │
│  │ Tool     │ │ LLM     │ │ Plugin   │ │ SubAgent │   │
│  │ Router   │ │ Backend │ │ Registry │ │ Spawner  │   │
│  └──┬───────┘ │ (peer層)│ └──────────┘ └──────────┘   │
│     ▼         └─────────┘                               │
│     ▼                                                   │
│  ┌──────────────┐                                       │
│  │ Tool Executor│ ← Web検索, ファイル操作, API呼出 etc  │
│  └──────────────┘                                       │
└─────────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐  ┌──────────────────────────┐
│  LLM Backend Peers                       │  │  Memory System            │
│  ┌───────────┐ ┌──────────┐ ┌─────────┐ │  │  SQLite + ruri-v3-30m     │
│  │ vllm-mlx  │ │ Cloud API│ │ Claude  │ │  │  FTS5 + ベクトル検索      │
│  │ (local)   │ │ (OpenAI等)│ │ Code   │ │  └──────────────────────────┘
│  └───────────┘ └──────────┘ └─────────┘ │
│  OpenAI互換 API / 統一 LlmPeer trait     │
└─────────────────────────────────────────┘
```

---

## 2. コンポーネント構成

### 2.1 Pluggable Gateway (`slm-kairo-gateway`)

**v0.4: HTTP Gateway をベースレイヤーとした階層化設計に変更**

Gateway を2層構造とし、HTTP Gateway を全ての通信の基盤とする。Discord/Nostr 等のプラットフォーム固有プロトコルは HTTP Gateway の上にアダプタとして実装する。これにより、新しいプラットフォームの追加はアダプタの追加のみで済む。

```
┌───────────────────────────────────────────────────┐
│          Adapter Layer（プラットフォーム固有）      │
│                                                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────┐  │
│  │ Discord      │ │ Nostr        │ │ 将来の   │  │
│  │ Adapter      │ │ Adapter      │ │ Adapter  │  │
│  │ (serenity)   │ │ (nostr-sdk)  │ │          │  │
│  └──────┬───────┘ └──────┬───────┘ └────┬─────┘  │
│         │                │               │        │
│         ▼                ▼               ▼        │
│  ┌─────────────────────────────────────────────┐  │
│  │          HTTP Gateway（ベースレイヤー）       │  │
│  │  axum ベース / REST + WebSocket              │  │
│  │  統一メッセージ形式 / 拡張機能レジストリ      │  │
│  └─────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────┘
```

#### 2.1.1 HTTP Gateway（ベースレイヤー）

全Gatewayの基盤。統一されたメッセージ形式で内部コンポーネントと通信する。

```rust
/// ベースレイヤー: 全アダプタがこのインターフェースを通じて内部と通信
struct HttpGateway {
    router: axum::Router,
    tx: mpsc::Sender<IncomingMessage>,
    extensions: ExtensionRegistry,  // reaction等の拡張機能
}

/// 統一メッセージ形式
struct IncomingMessage {
    source: AdapterSource,          // どのアダプタから来たか
    channel_id: ChannelId,
    author: Author,
    content: String,
    metadata: HashMap<String, serde_json::Value>,  // アダプタ固有データ
}

/// 拡張機能レジストリ — 各アダプタが対応する拡張をここに登録
struct ExtensionRegistry {
    capabilities: HashMap<AdapterSource, Vec<Extension>>,
}

enum Extension {
    Reaction,       // リアクション送信
    Typing,         // タイピングインジケータ
    ThreadReply,    // スレッド返信
    FileUpload,     // ファイル添付
    RichEmbed,      // リッチ埋め込み
    Custom(String), // カスタム拡張
}
```

#### 2.1.2 Gateway Adapter trait

各プラットフォーム用アダプタが実装するトレイト。HTTP Gatewayの上に乗る。

```rust
#[async_trait]
trait GatewayAdapter: Send + Sync {
    /// アダプタ名
    fn name(&self) -> &str;

    /// 対応する拡張機能を宣言
    fn supported_extensions(&self) -> Vec<Extension>;

    /// 初期化・接続開始（HTTP Gatewayへの送信チャネルを受け取る）
    async fn start(&mut self, http_gw: Arc<HttpGateway>) -> Result<()>;

    /// メッセージ送信（HTTP Gatewayから呼ばれる）
    async fn send(&self, target: &ChannelId, message: OutgoingMessage) -> Result<()>;

    /// 拡張機能の実行（reaction等）
    async fn execute_extension(
        &self,
        ext: &Extension,
        target: &ChannelId,
        payload: serde_json::Value,
    ) -> Result<()>;

    /// シャットダウン
    async fn shutdown(&self) -> Result<()>;
}
```

#### 2.1.3 Discord Adapter

- **serenity** ベースの Discord bot
- HTTP Gateway の上にアダプタとして実装
- 対応拡張: Reaction, Typing, ThreadReply, FileUpload, RichEmbed
- チャンネルグルーピング: 同一チャンネルの連続メッセージをバッチ化（LocalGPTの教訓）

```
受信フロー:
  Discord Message → Discord Adapter → HTTP Gateway → セッション → 推論キューへ
```

**LocalGPTの教訓を反映:**
- チャンネルごとのメッセージキュー（連続投稿をまとめて処理）
- DM / サーバーチャンネルの区別
- メンション有無による応答判定

#### 2.1.4 Nostr Adapter

- NIP-01 準拠のfilter条件でイベント受信
- HTTP Gateway の上にアダプタとして実装
- 対応拡張: Reaction (NIP-25), Custom("zap") 等
- WebSocket接続でリレーからイベントを受信

```rust
struct NostrAdapter {
    relays: Vec<String>,
    filters: Vec<NostrFilter>,
    // v0.5: 秘密鍵はここに持たない。認証プラグイン（auth plugin）または
    // 外部設定（環境変数・キーストア）経由で署名を行う
    signer: Arc<dyn NostrSigner>,  // 署名の抽象化
}

/// 署名の抽象化トレイト — 秘密鍵の管理をAdapter外に分離
#[async_trait]
trait NostrSigner: Send + Sync {
    async fn sign_event(&self, unsigned: UnsignedEvent) -> Result<Event>;
    fn public_key(&self) -> PublicKey;
}
```

#### 2.1.5 外部連携 HTTP API

- HTTP Gateway が直接公開する REST / WebSocket エンドポイント
- アダプタを通さず直接利用可能
- 外部サービスからの連携用

### 2.2 Session Manager (`slm-kairo-session`)

- チャンネル単位でセッション管理（LocalGPTと同様）
- コンテキストウィンドウの管理（トークン数ベース）
- 会話履歴の永続化（SQLite）

```rust
// 概念的な構造
struct Session {
    channel_id: u64,
    messages: VecDeque<ChatMessage>,  // スライディングウィンドウ
    total_tokens: usize,
    max_context_tokens: usize,       // モデルのコンテキスト長
    system_prompt: String,
    soul_config: SoulConfig,         // v0.2: SOUL.md由来のペルソナ設定
    metadata: SessionMetadata,
}
```

**コンテキスト管理戦略:**
1. 新メッセージ追加時にトークン数を概算
2. `max_context_tokens` 超過時、古いメッセージから削除
3. システムプロンプトは常に保持
4. 重要メッセージのピン機能（削除対象から除外）

### 2.3 Inference Orchestrator (`slm-kairo-inference`)

Best-of-N推論の中核。詳細は §3 参照。

### 2.4 Tool Router & Executor (`slm-kairo-tools`)

FunctionGemmaベースのツールコーリング。詳細は §5 参照。

### 2.5 Plugin Registry (`slm-kairo-plugin`)

- タグベースのプラグインシステム（LocalGPTの知見）
- 組み込みツールもプラグイン形式で統一（v0.2）
- 各プラグインがどのような入力に反応するかをタグで定義
- 動的ロード不要 — コンパイル時に組み込み（Rustの強みを活かす）
- **v0.5: 全機能をプラグイン化する新アーキテクチャに移行（§27参照）**

### 2.6 SubAgent Spawner (`slm-kairo-subagent`) (v0.2 新規)

サブエージェントの生成・管理。詳細は §15 参照。

### 2.7 Memory System (`slm-kairo-memory`) (v0.2 新規)

ベクトル検索ベースの長期記憶。詳細は §18 参照。

---

## 3. Best-of-N 推論フロー

### 3.1 全体フロー

```
ユーザー入力
    │
    ▼
┌──────────────────────────┐
│ Phase 1: 並列推論 (N=8)   │
│                           │
│  同一プロンプトを N 回    │
│  異なる temperature で    │
│  同時にリクエスト          │
└──────────┬───────────────┘
           │ 8個の候補テキスト
           ▼
┌──────────────────────────┐
│ Phase 2: 評価・選定       │
│                           │
│  評価関数で各候補を       │
│  スコアリング             │
│  → 最高スコア候補を選定   │
└──────────┬───────────────┘
           │ ベスト候補 1つ
           ▼
┌──────────────────────────┐
│ Phase 3: Refinement       │
│                           │
│  ベスト候補 + 元の入力    │
│  → 再推論 (1回)          │
│  → 洗練された最終回答     │
└──────────┬───────────────┘
           │
           ▼
       最終結果
```

### 3.2 並列推論の実装

```
tokio::JoinSet を使用して N 個の推論を並列実行

パラメータ分散戦略:
  候補 #1: temperature=0.3  (保守的)
  候補 #2: temperature=0.5
  候補 #3: temperature=0.7  (バランス)
  候補 #4: temperature=0.7  (同温度・異シード)
  候補 #5: temperature=0.9
  候補 #6: temperature=1.0  (創造的)
  候補 #7: temperature=1.2  (冒険的)
  候補 #8: temperature=0.1  (ほぼgreedy)

全リクエストに異なる seed 値を付与
```

**タイムアウト処理:**
- 個別タイムアウト: 30秒（1候補が遅くても他は待たない）
- 全体タイムアウト: 45秒（最低3候補が返っていれば評価に進む）
- `tokio::select!` で最低 N/2 候補が揃ったら評価開始オプション

### 3.3 vllm-mlx リクエスト

```
POST http://localhost:8000/v1/chat/completions
{
  "model": "model-name",
  "messages": [...],
  "temperature": <varies>,
  "seed": <unique>,
  "max_tokens": 2048,
  "stream": false          ← ストリーミング不要
}
```

---

## 4. 評価関数の設計

8個の候補から最良を選ぶための多軸評価。

### 4.1 評価手法: LLM-as-Judge + ヒューリスティック

**v0.4: 構造スコア廃止、ルール準拠評価新規追加、一貫性スコア見直し**

```
最終スコア = w1 × LLM評価 + w2 × 長さスコア + w3 × 一貫性スコア + w4 × ペルソナ準拠スコア + w5 × ルール準拠スコア

デフォルト重み (v0.4 再配分):
  w1 = 0.30 (LLM評価)
  w2 = 0.10 (長さ)
  w3 = 0.10 (一貫性)       ← v0.4: 重み削減（外れ値にも価値あり）
  w4 = 0.25 (ペルソナ準拠)
  w5 = 0.25 (ルール準拠)   ← v0.4 新規
```

### 4.2 評価用モデルの選定基準 (v0.2 新規)

LLM-as-Judge で使用するモデルは、**内容によって評価にばらつき（分散）が出る**ことが望ましい。

**選定基準:**
1. **分散の出やすさ**: 全候補に同じスコアを付けない（差別化能力）
2. **推論速度**: 評価は1回の追加推論なので高速であること
3. **コンテキスト長**: 8候補を同時に比較するため十分な長さが必要
4. **言語対応**: 日本語の品質評価が可能であること

**推奨アプローチ:**
- メイン推論モデルとは**別モデル**を使用（自己評価バイアス回避）
- 候補: gemma-2-2b-it（軽量・高速）、phi-3-mini（分散が出やすい）
- 定期的にA/Bテストで評価モデルの分散を計測し、最適モデルを選定

**分散計測方法:**
```
1. テストプロンプトセット（50件程度）を用意
2. 各プロンプトに対してN候補を生成
3. 評価モデルでスコアリング
4. スコアの標準偏差を計測
5. 標準偏差が大きいモデル = 差別化能力が高い
```

### 4.3 各スコアの詳細

**LLM評価スコア (0.0–1.0):**
- 別途1回の推論で、全候補を比較評価させる
- プロンプト: 「以下の N 個の回答を質問に対する適切さで1-10評価してください」
- コスト: 追加1推論（並列推論と比べて許容範囲）

**長さスコア (0.0–1.0):**
- 極端に短い/長い回答にペナルティ
- 理想長は質問タイプに依存（簡単な質問→短い方が良い）
- `score = 1.0 - |actual_len - ideal_len| / max_len`

**一貫性スコア (0.0–1.0): (v0.4 見直し)**
- 候補間の合意度を参考にするが、外れ値を一律にペナルティしない
- **外れ値の価値評価**: 他の候補と大きく異なる回答がある場合、LLM-as-Judgeで「レアだが価値のある視点か」を判定
- 多数派に近いほど高スコアだが、ユニークな洞察を含む外れ値にはボーナス付与
- 実装: 候補間のコサイン類似度の平均 + 外れ値品質判定

**ルール準拠スコア (0.0–1.0): (v0.4 新規)**
- SOUL.md および MEMORY.md に記載されたルール・制約への準拠度を評価
- 評価観点:
  - SOUL.md の行動規範・制約事項の遵守
  - MEMORY.md に記録されたユーザーの好み・ルールの遵守
  - 明示的な禁止事項への違反がないか
- 実装: LLM-as-Judge に SOUL.md + MEMORY.md のルール部分を渡し、各候補の準拠度を評価
- **違反時の再推論**: ルール準拠スコアが閾値（デフォルト: 0.4）未満の候補しかない場合、並列推論をやり直す（最大1回）
- SOUL.md/MEMORY.md が未定義の場合はスコア1.0、w5の重みを他に再配分

**ペルソナ準拠スコア (0.0–1.0):**
- SOUL.mdで定義されたキャラクター設定にどれだけ沿っているか
- 評価観点:
  - **口調の一致**: 定義されたトーン・語尾の使用
  - **行動規範の遵守**: SOUL.mdで定義された制約の遵守
  - **キャラクターの一貫性**: ペルソナからの逸脱がないか
- 実装: LLM-as-Judgeに SOUL.md の内容を渡し、準拠度を評価させる
- SOUL.mdが未定義の場合はスコア1.0（ペナルティなし）、w5の重みを他に再配分

### 4.4 高速モード (v0.4 改善)

**v0.4: 入力の難易度判定に基づく動的並列数制御**

入力が一定トークン数以下の場合、まず専用の難易度判定モデルで要件の難しさを測定し、難易度に応じて並列数を動的に変更する。

**難易度判定フロー:**
```
入力テキスト
    │
    ├─ トークン数 > 100 → 通常モード (N=8)
    │
    └─ トークン数 ≤ 100
         │
         ▼
    ┌──────────────────────────┐
    │ 難易度判定モデル           │
    │ (専用ファインチューニング) │
    │ 目標レイテンシ: 0.3秒     │
    │ 出力: 難易度 1-5           │
    └──────────┬───────────────┘
               │
    ┌──────────┴──────────┐
    │ 難易度 → 並列数マッピング │
    │                          │
    │  難易度1 (trivial): N=1   │
    │    → LLM評価スキップ      │
    │    → Refinement スキップ  │
    │                          │
    │  難易度2 (easy):    N=2   │
    │    → LLM評価スキップ      │
    │                          │
    │  難易度3 (medium):  N=4   │
    │    → ヒューリスティック評価│
    │                          │
    │  難易度4 (hard):    N=6   │
    │    → フル評価             │
    │                          │
    │  難易度5 (complex): N=8   │
    │    → フル評価 + Refinement│
    └──────────────────────────┘
```

**難易度判定モデルの設計:**
- 専用のファインチューニングモデル（軽量SLM）
- 入力: ユーザーメッセージ（短文）
- 出力: 難易度レベル（1-5の整数のみ）
- 目標レイテンシ: 0.3秒以内
- テストデータ: 各難易度ごとに1,000件、計5,000件を用意
- 評価指標: 分類精度 + レイテンシ

**並列数マッピングは仮決め。** 運用データを蓄積して最適化する。

---

## 5. ツールコーリング設計

### 5.1 概要

FunctionGemma をファインチューニングし、独自のツールコーリング機構を実装する。
OpenAI の function calling フォーマットには依存しない。

**v0.2方針: トークンを抑えてシンプルに**
- ツール定義はできるだけ短い形式で表現
- XMLタグベースの軽量フォーマット

### 5.2 ツール定義フォーマット（v0.2 簡素化）

```
<tools>
web_search(query:str, count:int=5) — Search the web
web_fetch(url:str) — Fetch URL content
calculator(expr:str) — Evaluate math expression
datetime() — Current date/time
memory_search(query:str, limit:int=5) — Search long-term memory
memory_write(content:str) — Write to memory
claude_code(task:str) — Delegate coding task to Claude Code
config_update(key:str, value:str) — Update config safely
</tools>
```

従来のJSONスキーマ形式と比較して**約70%のトークン削減**。

### 5.3 ツールコーリングフロー

```
ユーザー入力
    │
    ▼
┌─────────────────────────────┐
│ ツール判定推論               │
│ (FunctionGemma)              │
│                              │
│ 入力: ユーザーメッセージ     │
│       + ツール定義一覧       │
│ 出力: <tool_call> or 通常応答│
└──────────┬──────────────────┘
           │
     ┌─────┴─────┐
     │            │
  ツール呼出   通常応答
     │            │
     ▼            ▼
┌──────────┐  Best-of-N へ
│ パース    │
│ tool_call │
└────┬─────┘
     │
     ▼
┌──────────────┐
│ Tool Executor │
│ (sandbox実行) │
└────┬─────────┘
     │ 結果
     ▼
┌──────────────────────┐
│ ツール結果 + 元入力   │
│ → 最終推論 (1回)      │
│ → ユーザーへ返答      │
└──────────────────────┘
```

### 5.4 ファインチューニング設計 (v0.2 拡充)

**学習データフォーマット（簡素化版）:**
```
<|system|>
<tools>
web_search(query:str) — Search the web
calculator(expr:str) — Evaluate math
</tools>
<|user|>
{user_message}
<|assistant|>
<tool_call>web_search("検索クエリ")</tool_call>
```

**多段ツール呼出:**
- 1ターンで複数ツールを呼べる（`<tool_call>` を複数出力）
- ツール結果を `<tool_result>` タグで渡して再推論
- 最大チェーン深度: 5（無限ループ防止）

**データセット作成方法 (v0.2 新規):**

1. **シード データ収集**
   - 既存のツールコーリングデータセット（gorilla, glaive-function-calling等）から抽出
   - OpenAI function calling形式 → SLM-Kairo形式に変換
2. **合成データ生成**
   - Claude等の大規模LLMで高品質な学習データを生成
   - プロンプト: 「以下のツール定義に対して、ユーザーの質問とそれに対するツール呼び出しの例を生成」
3. **フォーマット変換パイプライン**
   ```
   元データ(JSON形式) → パーサー → SLM-Kairo簡素形式 → バリデーション → 学習データ
   ```
4. **データ品質管理**
   - ツール引数の型チェック
   - 呼び出し結果の妥当性検証
   - 不正なフォーマットの自動除外
5. **目標データ量**: 初期10,000件、段階的に拡充

### 5.5 組み込みツール（v0.2: 全てプラグイン形式で統一）

| ツール | タグ | 説明 |
|--------|------|------|
| `web_search` | `search`, `web` | Web検索 |
| `web_fetch` | `fetch`, `url` | URLからコンテンツ取得 |
| `calculator` | `math`, `calculate` | 数式計算 |
| `datetime` | `time`, `date` | 現在時刻・日付 |
| `memory_search` | `memory`, `recall` | 長期記憶の検索 |
| `memory_write` | `memory`, `remember` | 長期記憶への書き込み |
| `claude_code` | `code`, `programming` | **Claude Code連携** (v0.2 新規) |
| `config_update` | `config`, `settings` | **安全なconfig更新** (v0.2 新規) |

---

## 6. LLM バックエンド (v0.4 拡張)

### 6.1 peer層による抽象化

**v0.4: vllm-mlx 専用設計から、複数バックエンドに対応する peer 層アーキテクチャに変更**

異なるLLMバックエンドを統一的に扱う `LlmPeer` トレイトを導入。vllm-mlx、クラウドAPI、Claude Code 等を同一インターフェースで利用可能にし、将来新しいバックエンドが登場しても容易に乗り換えられる設計とする。

```rust
#[async_trait]
trait LlmPeer: Send + Sync {
    /// バックエンド名
    fn name(&self) -> &str;

    /// 最大コンテキスト長（トークン数）
    fn max_context_tokens(&self) -> usize;

    /// 並列推論が可能か（Claude Code等は設定で無効化可能）
    fn supports_parallel(&self) -> bool;

    /// チャット補完
    async fn chat_completion(&self, req: ChatRequest) -> Result<ChatResponse>;

    /// バッチチャット補完
    async fn chat_completion_batch(&self, reqs: Vec<ChatRequest>) -> Vec<Result<ChatResponse>>;

    /// ヘルスチェック
    async fn health(&self) -> Result<bool>;

    /// logprobs 対応か
    fn supports_logprobs(&self) -> bool { false }
}

/// vllm-mlx ローカルバックエンド
struct VllmMlxPeer {
    base_url: String,
    client: reqwest::Client,
    semaphore: Semaphore,
    max_context: usize,
}

/// クラウドAPI バックエンド（OpenAI, Anthropic等）
struct CloudApiPeer {
    provider: String,       // "openai" | "anthropic" | etc.
    api_key: String,
    base_url: String,
    model: String,
    max_context: usize,
    rate_limiter: RateLimiter,
}

/// Claude Code バックエンド
struct ClaudeCodePeer {
    claude_path: PathBuf,
    workspace: PathBuf,
    timeout: Duration,
    parallel_enabled: bool,  // v0.4: 並列推論で爆死するので設定で切れる
}
```

### 6.2 peer 選択とフォールバック

```rust
struct LlmBackend {
    /// 優先順位付きのpeerリスト
    peers: Vec<Box<dyn LlmPeer>>,
    /// デフォルトpeer
    default_peer: String,
    /// 用途別peer設定
    peer_routing: HashMap<String, String>,  // "inference" -> "vllm-local", "evaluation" -> "cloud-gpt4o"
}
```

peer_routing により、推論・評価・ツール判定で異なるバックエンドを使い分け可能。

### 6.3 接続管理

- **コネクションプール**: reqwest のデフォルトプール使用
- **同時リクエスト制限**: peer ごとに個別の `Semaphore` を設定
- **リトライ**: 3回、exponential backoff (100ms, 400ms, 1600ms)
- **ヘルスチェック**: 起動時 + 定期 (30秒間隔)
- **フォールバック**: プライマリpeer が不健全な場合、次の peer に自動切り替え

### 6.4 エラーハンドリング

| エラー | 対応 |
|--------|------|
| 接続拒否 | リトライ → 失敗時は error リアクションのみ（v0.4: エラーメッセージ出力しない） |
| タイムアウト | 候補をスキップ（Best-of-N は部分結果で続行） |
| 429 Too Many Requests | バックオフ + セマフォ縮小 |
| モデルロードエラー | 起動時チェックで検出、ログ出力 |
| 無限ループ検出 | v0.4: エラーメッセージをユーザーに出力しない。error リアクション（絵文字）のみ返す |

**v0.4: 無限ループ防止の改善**
- エラー発生時にエラーメッセージをテキストで出力しない（エラーメッセージ自体がループの原因になりうる）
- 代わりに Gateway の拡張機能を使って error リアクション（例: ❌）のみ付与する
- 内部ログにはエラー詳細を記録（管理画面 §20 で確認可能）

---

## 7. セッション/コンテキスト管理

### 7.1 セッションライフサイクル

```
新規メッセージ受信
    │
    ├─ セッション存在? ─ Yes → セッション取得 → メッセージ追加
    │                    No  → セッション作成 → system prompt設定
    │                                         → SOUL.md読み込み (v0.2)
    │
    ▼
コンテキスト構築
    │
    ├─ トークン数チェック
    │   ├─ 範囲内 → そのまま
    │   └─ 超過  → 古いメッセージ削除 (system prompt は保持)
    │
    ├─ メモリ検索 (v0.2: 関連する長期記憶をコンテキストに注入)
    │
    ▼
推論実行
    │
    ▼
アシスタント応答をセッションに追加
```

### 7.2 永続化

- **SQLite** (rusqlite) でセッションを永続化
- テーブル: `sessions`, `messages`, `tool_calls`
- 起動時に直近セッションを復元（LRUで上限管理）
- 定期的に古いセッションをアーカイブ

### 7.3 チャンネルグルーピング（LocalGPTの知見 / v0.4 動的調整）

- 同一チャンネルで短時間に連続したメッセージをバッチ化
- 複数ユーザーからの同時メッセージも1セッション内で処理
- チャンネルごとにロック（同時推論を防止）

**v0.4: デバウンス時間の動的調整**

固定500msecでは短すぎるケースがある（ユーザーが複数メッセージに分けて投稿する場合等）。過去の受信ペースから動的にデバウンス時間を調整する。

```rust
struct DynamicDebounce {
    /// 最小デバウンス時間
    min_ms: u64,          // デフォルト: 800ms
    /// 最大デバウンス時間
    max_ms: u64,          // デフォルト: 5000ms
    /// チャンネルごとの受信ペース履歴
    pace_history: HashMap<ChannelId, VecDeque<Instant>>,
    /// 直近N件のメッセージ間隔から算出
    window_size: usize,   // デフォルト: 20
}

impl DynamicDebounce {
    /// 過去のメッセージ間隔の中央値 × 1.5 をデバウンス時間とする
    /// ただし min_ms ≤ result ≤ max_ms にクランプ
    fn calculate_debounce(&self, channel: &ChannelId) -> Duration;
}
```

---

## 8. プラグインシステム

### 8.1 タグベースルーティング（LocalGPTの知見を活用）

**v0.2: 組み込みツールもプラグイン形式に統一**

```rust
trait Plugin: Send + Sync {
    /// プラグイン名
    fn name(&self) -> &str;

    /// このプラグインが反応するタグ
    fn tags(&self) -> &[&str];

    /// ツール定義（ツールコーリング用の簡素形式）
    fn tool_definitions(&self) -> Vec<ToolDef> { vec![] }

    /// system prompt への追加テキスト
    fn system_prompt_extension(&self) -> Option<String>;

    /// メッセージ前処理（入力の変換・拡張）
    fn pre_process(&self, ctx: &mut InferenceContext) -> Result<()>;

    /// ツール実行（tool_callが自分のツールだった場合）
    async fn execute_tool(&self, name: &str, args: &ToolArgs) -> Result<String> {
        Err(anyhow!("Not a tool plugin"))
    }

    /// 推論後の後処理
    fn post_process(&self, ctx: &mut InferenceContext, response: &mut String) -> Result<()>;
}

/// 簡素なツール定義
struct ToolDef {
    name: String,
    signature: String,  // "web_search(query:str, count:int=5)"
    description: String,
}
```

### 8.2 組み込みプラグイン（v0.2 拡充）

| プラグイン | タグ | 説明 |
|-----------|------|------|
| `WebSearchPlugin` | `search`, `web` | Web検索ツール |
| `WebFetchPlugin` | `fetch`, `url` | URL取得ツール |
| `MathPlugin` | `math`, `calculate` | 計算ツール |
| `DateTimePlugin` | `time`, `date` | 日時ツール |
| `PersonaPlugin` | `persona` | キャラクター設定の適用 |
| `MemoryPlugin` | `memory` | 長期記憶の読み書き |
| `ClaudeCodePlugin` | `code`, `programming` | **Claude Code連携** (v0.2 新規) |
| `ConfigPlugin` | `config`, `settings` | **安全なconfig更新** (v0.2 新規) |

### 8.3 Claude Code連携プラグイン (v0.2 新規)

コーディングタスクをClaude Code CLIに委譲する必須プラグイン。

```rust
struct ClaudeCodePlugin {
    claude_path: PathBuf,      // claude CLI のパス
    workspace: PathBuf,        // 作業ディレクトリ
    timeout: Duration,         // 実行タイムアウト
    allowed_tools: Vec<String>, // 許可するツール（Read, Write, Edit等）
}
```

**使用フロー:**
1. ツール判定で `claude_code` が選択される
2. タスク内容をClaude Code CLIに渡す
3. Claude Codeがファイル操作・コード生成を実行
4. 結果（stdout/stderr）を取得してユーザーに返す

**セキュリティ:**
- `--allowedTools` でツールを制限
- 作業ディレクトリを限定
- タイムアウト付き実行

### 8.4 安全なconfig更新プラグイン (v0.2 新規)

設定変更を安全に行うためのプラグイン。

```rust
struct ConfigPlugin {
    config_path: PathBuf,
    backup_dir: PathBuf,
}
```

**安全機構:**
1. **バリデーション**: 更新前にTOMLパース + スキーマチェック
2. **バックアップ**: 変更前の設定を自動バックアップ
3. **ロールバック**: 適用後に問題があれば自動復帰
4. **プレビュー**: diff表示で変更内容を確認可能

```
config_update フロー:
  新設定値 → バリデーション → バックアップ作成 → 適用 → ヘルスチェック
                                                           │
                                                     ┌─────┴─────┐
                                                     OK        NG
                                                     │          │
                                                   完了      ロールバック
```

### 8.5 プラグインの有効化

- チャンネルごと / サーバーごとにプラグインを有効/無効化
- 設定ファイル (TOML) で管理

---

## 9. 想定する crate / 依存関係

| crate | 用途 | 備考 |
|-------|------|------|
| `tokio` | 非同期ランタイム | rt-multi-thread |
| `serenity` | Discord API | Gateway + REST |
| `nostr-sdk` | Nostr プロトコル | v0.2: Nostr Adapter用 |
| `reqwest` | HTTP クライアント | vllm-mlx通信 |
| `axum` | HTTP サーバー | v0.2: HTTP API Gateway用 |
| `serde` / `serde_json` | シリアライズ | API通信全般 |
| `rusqlite` | SQLite | セッション・メモリ永続化 |
| `tracing` / `tracing-subscriber` | ログ | 構造化ログ |
| `tiktoken-rs` | トークンカウント | コンテキスト管理 |
| `toml` | 設定ファイル | |
| `anyhow` / `thiserror` | エラーハンドリング | |
| `dashmap` | 並行HashMap | セッションキャッシュ |
| `tower` | ミドルウェア | リトライ、レート制限 |
| `fastembed` | ローカル埋め込み | v0.2: ruri-v3-30mベクトル化 |
| `notify` | ファイル監視 | v0.2: メモリ自動再インデックス |
| `glob` | ファイルパターン | v0.2: メモリファイル検索 |
| `uuid` | UUID生成 | v0.2: チャンクID |
| `sha2` | ハッシュ | v0.2: コンテンツハッシュ |

---

## 10. ディレクトリ構成案

```
slm-kairo/
├── Cargo.toml                  # ワークスペース定義
├── config/
│   ├── default.toml            # デフォルト設定
│   └── example.toml            # 設定例
├── workspace/                  # エージェントのワークスペース
│   ├── SOUL.core.toml          # ペルソナ定義（コア・書き換え不可）
│   ├── SOUL.ext.toml           # ペルソナ定義（拡張・自己編集可）
│   ├── MEMORY.md               # 長期記憶（キュレート済み）
│   ├── HEARTBEAT.md            # 定期タスク
│   └── memory/                 # 日次ログ
│       └── YYYY-MM-DD.md
├── crates/
│   ├── slm-kairo-core/             # 共通型、トレイト定義
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs        # ChatMessage, Session 等
│   │       ├── error.rs
│   │       ├── config.rs
│   │       └── gateway.rs      # v0.2: Gateway trait
│   ├── slm-kairo-inference/        # 推論エンジン + Best-of-N
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── orchestrator.rs # Best-of-N オーケストレーション
│   │       ├── evaluator.rs    # 評価関数
│   │       ├── vllm_client.rs  # vllm-mlx クライアント
│   │       └── refinement.rs   # Refinement pass
│   ├── slm-kairo-session/          # セッション管理
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs
│   │       ├── context.rs      # コンテキスト構築
│   │       └── store.rs        # SQLite永続化
│   ├── slm-kairo-tools/            # ツールコーリング
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs       # ツール判定・ルーティング
│   │       ├── executor.rs     # ツール実行
│   │       ├── parser.rs       # <tool_call> パーサー
│   │       └── builtins/       # 組み込みツール（プラグイン形式）
│   │           ├── mod.rs
│   │           ├── web_search.rs
│   │           ├── calculator.rs
│   │           ├── claude_code.rs  # v0.2
│   │           └── config.rs      # v0.2
│   ├── slm-kairo-gateway/          # Gateway実装
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── discord.rs      # Discord Gateway
│   │       ├── nostr.rs        # v0.2: Nostr Gateway
│   │       ├── http_api.rs     # v0.2: HTTP API Gateway
│   │       ├── formatter.rs    # レスポンス整形
│   │       └── debounce.rs     # チャンネルデバウンス
│   ├── slm-kairo-plugin/           # プラグインシステム
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs
│   │       └── builtins/
│   ├── slm-kairo-memory/           # v0.2: メモリーシステム
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs      # MemoryManager
│   │       ├── index.rs        # SQLite FTS5 + ベクトルインデックス
│   │       ├── embeddings.rs   # ruri-v3-30m 埋め込み
│   │       ├── search.rs       # ハイブリッド検索
│   │       ├── watcher.rs      # ファイル変更監視
│   │       └── workspace.rs    # ワークスペース初期化
│   └── slm-kairo-subagent/         # v0.2: サブエージェント
│       └── src/
│           ├── lib.rs
│           ├── spawner.rs      # サブエージェント生成
│           └── protocol.rs     # 通信プロトコル
└── src/
    └── main.rs                 # エントリポイント
```

---

## 11. 設定ファイル例

```toml
[server]
name = "slm-kairo"

[inference]
default_peer = "vllm-local"
model = "gemma-2-9b"
max_tokens = 2048
default_temperature = 0.7

# v0.4: LLMバックエンドpeer設定
[inference.peers.vllm-local]
type = "vllm"
url = "http://localhost:8000"
max_context_tokens = 8192
parallel = true

[inference.peers.cloud-openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"
model = "gpt-4o-mini"
max_context_tokens = 128000
parallel = true

[inference.peers.claude-code]
type = "claude_code"
claude_path = "claude"
workspace = "./workspace"
max_context_tokens = 200000
parallel = false              # v0.4: 並列推論でトークン爆死するので無効化可能
timeout_sec = 120

# v0.4: 用途別peer設定
[inference.peer_routing]
inference = "vllm-local"
evaluation = "vllm-local"
tool_detection = "vllm-local"
difficulty_judge = "vllm-local"   # 高速モード難易度判定

[inference.best_of_n]
enabled = true
n = 8
min_candidates = 3
timeout_per_candidate_sec = 30
timeout_total_sec = 45
temperatures = [0.1, 0.3, 0.5, 0.7, 0.7, 0.9, 1.0, 1.2]

[inference.evaluator]
use_llm_judge = true
judge_model = "phi-3-mini"       # v0.2: 評価用モデル指定
weight_llm = 0.30               # v0.4: 再配分
weight_length = 0.10
weight_consistency = 0.10       # v0.4: 外れ値にも価値があるため重み削減
weight_persona = 0.25
weight_rule_compliance = 0.25   # v0.4: ルール準拠スコア
rule_compliance_retry_threshold = 0.4  # v0.4: この閾値未満で再推論

[session]
max_context_tokens = 8192
debounce_min_ms = 800          # v0.4: 動的調整の最小値
debounce_max_ms = 5000         # v0.4: 動的調整の最大値
db_path = "data/sessions.db"

# v0.4: Gateway設定（HTTP基盤 + アダプタ階層）
[gateway.http]
enabled = true
bind = "127.0.0.1:3000"

[gateway.adapters.discord]
enabled = true
# token は環境変数 DISCORD_TOKEN から取得
prefix = "!"
extensions = ["reaction", "typing", "thread_reply", "file_upload", "rich_embed"]

[gateway.adapters.nostr]
enabled = false
relays = ["wss://relay.damus.io", "wss://nos.lol"]
# v0.5: 秘密鍵設定を除去。署名は認証プラグインで管理
signer = "env"              # "env" (環境変数) | "keystore" | "plugin"
signer_key_env = "NOSTR_PRIVATE_KEY"  # signer=env の場合の環境変数名
extensions = ["reaction"]

[gateway.adapters.nostr.filter]
kinds = [1]           # kind 1 = text note

[tools]
max_chain_depth = 5
tool_model = "function-gemma"

[plugins]
enabled = ["web_search", "web_fetch", "math", "datetime", "memory", "claude_code", "persona", "config"]

# v0.4: SOUL設定（2層構造・TOML）
[soul]
core_path = "workspace/SOUL.core.toml"
ext_path = "workspace/SOUL.ext.toml"
core_hash_check = true    # 起動時にコアSOULのハッシュチェック

# v0.2: ハートビート設定
[heartbeat]
enabled = true
interval_sec = 60            # ローカルLLMなのでコストゼロ、短間隔可
tasks_file = "workspace/HEARTBEAT.md"

# v0.2: メモリーシステム設定
[memory]
workspace = "workspace"
embedding_provider = "local"
embedding_model = "ruri-v3-30m"
chunk_size = 400
chunk_overlap = 80
db_path = "data/memory.sqlite"

# v0.2: 安全な再起動設定
[restart]
enabled = true
handoff_timeout_sec = 30
backup_config = true

# v0.2: サブエージェント設定
[subagent]
max_concurrent = 4
timeout_sec = 300
spawn_as_process = true       # v0.4: 別プロセスとして起動
watchdog_interval_sec = 10    # v0.4: alive チェック間隔
watchdog_timeout_sec = 30     # v0.4: alive タイムアウト
inherit_soul = true           # v0.4: 親のSOULを引き継ぐ
```

---

## 12. 処理シーケンス（メッセージ受信から応答まで）

```
1. Gateway メッセージ受信（Discord / Nostr / HTTP）
2. デバウンス待機（動的調整、§7.3）
3. セッション取得 or 作成（SOUL.core.toml + SOUL.ext.toml 読み込み含む）
4. コンテキスト構築 (system prompt + SOUL + 履歴 + 新メッセージ)
5. メモリ検索 → 関連する長期記憶をコンテキストに注入 (v0.2)
6. プラグイン pre_process
7. ツール判定推論 (FunctionGemma)
   7a. ツール呼出あり → 実行 → 結果をコンテキストに追加 → 7へ戻る (max 5回)
   7b. ツール呼出なし → 8へ
8. Best-of-N 並列推論 (Phase 1)
9. 評価・選定 (Phase 2) — ペルソナ準拠スコア含む (v0.2)
10. Refinement 推論 (Phase 3)
11. プラグイン post_process
12. セッションにアシスタント応答を保存
13. メモリ更新（重要な情報があれば長期記憶に書き込み）(v0.2)
14. Gateway へ応答送信
```

---

## 13. LocalGPTからの教訓と反映

| 教訓 | SLM-Kairoでの対応 |
|------|--------------|
| セッション管理が複雑化しやすい | チャンネル単位の単純なモデル + SQLite永続化 |
| バッチ処理のタイミングが難しい | v0.4: 過去の受信ペースから動的にデバウンス調整 |
| チャンネルグルーピングは必須 | 初期から設計に組み込み |
| タグシステムは柔軟で良い | Plugin traitのtags()として採用 |
| ストリーミングは Discord では不要 | 最初から非ストリーミング前提で設計 |
| エラーハンドリングが後回しになりがち | Result型 + thiserror で型安全に |
| 設定の散らばり | 単一TOMLファイル + 環境変数オーバーライド |
| メモリシステムは汎用的な設計が良い | LocalGPTのMemoryManager構造を踏襲 (v0.2) |
| 埋め込みプロバイダの切り替え可能性 | trait EmbeddingProviderで抽象化 (v0.2) |
| ファイル監視で自動再インデックス | notify crateによるMemoryWatcher (v0.2) |

---

## 14. 安全な再起動機構 (v0.2 新規)

### 14.1 概要

SLM-Kairoが自分自身を安全に再起動できる機構。設定変更やアップデート時に使用。

### 14.2 引き継ぎ再起動（Handoff Restart）

```
┌──────────────────┐
│ 現行エージェント  │
│  (Process A)      │
│                   │
│ 1. 再起動決定     │
│ 2. 状態をシリアライズ │
│ 3. 新プロセスspawn │──────┐
│ 4. 引き継ぎデータ送信│      │
│ 5. 確認待ち       │      ▼
│       ◄──────────│  ┌──────────────────┐
│ 6. ACK受信        │  │ 新エージェント    │
│ 7. Gateway切断    │  │  (Process B)      │
│ 8. 自身を終了     │  │                   │
└──────────────────┘  │ a. 起動・初期化    │
                       │ b. 引き継ぎデータ受信│
                       │ c. セッション復元   │
                       │ d. ACK送信         │
                       │ e. Gateway接続     │
                       │ f. 通常運用開始     │
                       └──────────────────┘
```

### 14.3 引き継ぎデータ

```rust
struct HandoffData {
    /// アクティブセッション一覧
    active_sessions: Vec<SessionSnapshot>,
    /// 処理中のリクエスト（再実行用）
    pending_requests: Vec<PendingRequest>,
    /// 設定のハッシュ（設定変更検知用）
    config_hash: String,
    /// 引き継ぎタイムスタンプ
    timestamp: u64,
}
```

### 14.4 引き継ぎ通信方法（要検討）

候補:
1. **Unix Domain Socket**: 最もシンプル。一時ソケットファイルで通信
2. **共有ファイル**: JSONファイルに書き出し、新プロセスが読み込み
3. **TCP localhost**: ポート指定で接続

**推奨: Unix Domain Socket** — 高速・安全・一時的

### 14.5 引き継ぎ検証（v0.4 強化）

新プロセスが正しくコンテキストを引き継げたかをLLM推論で検証する。

```
┌──────────────────┐         ┌──────────────────┐
│ 旧プロセス (A)    │         │ 新プロセス (B)    │
│                   │         │                   │
│ 引き継ぎデータ送信│────────→│ データ受信・復元   │
│                   │         │                   │
│ 検証クエリ送信    │────────→│ LLM推論で応答     │
│ 例: "xxxのこと   │         │                   │
│  忘れないでね"    │         │                   │
│                   │←────────│ 推論結果を返信     │
│                   │         │                   │
│ 応答を評価        │         │                   │
│ ┌────────────┐   │         │                   │
│ │ OK判定      │   │         │                   │
│ │ B:"xxxの    │   │         │                   │
│ │  ことは安心 │   │         │                   │
│ │  して任せて"│→ 引き継ぎ成功 → Gateway切断 → 終了│
│ └────────────┘   │         │                   │
│ ┌────────────┐   │         │                   │
│ │ NG判定      │   │         │                   │
│ │ B:"xxxって │   │         │                   │
│ │  なに？"    │→ 引き継ぎ失敗 → Bをkill → A継続  │
│ └────────────┘   │         │                   │
└──────────────────┘         └──────────────────┘
```

**検証の評価基準:**
- 新プロセスが引き継ぎデータの核心を理解しているか
- 文脈を把握した応答ができているか（単なるオウム返しではなく）
- バグの可能性がある場合（無関係な応答、エラー等）は即座に中断

### 14.6 安全機構

- 新プロセスのヘルスチェックが通るまで旧プロセスは終了しない
- **v0.4: LLM推論による引き継ぎ検証が通るまで旧プロセスは終了しない**
- タイムアウト（30秒）: 引き継ぎ失敗時は旧プロセスが継続
- 設定バリデーション: 新設定が不正な場合は再起動をキャンセル
- 引き継ぎ失敗時は新プロセスをkillし、エラーログを記録

---

## 15. サブエージェント機構 (v0.2 新規)

### 15.1 概要

メインエージェントから独立したタスクをサブエージェントに委譲する仕組み。

### 15.2 アーキテクチャ

```
┌─────────────────────────────────┐
│        Main Agent                │
│                                  │
│  タスク分析 → 委譲判定           │
│       │                          │
│       ▼                          │
│  ┌──────────────┐               │
│  │ SubAgent     │               │
│  │ Spawner      │               │
│  └──────┬───────┘               │
│         │ spawn                  │
│   ┌─────┼─────────┐            │
│   ▼     ▼         ▼            │
│  ┌───┐ ┌───┐   ┌───┐          │
│  │SA1│ │SA2│   │SA3│          │
│  └─┬─┘ └─┬─┘   └─┬─┘          │
│    │      │       │             │
│    ▼      ▼       ▼             │
│  結果   結果    結果            │
│    └──────┴───────┘             │
│           │                      │
│           ▼                      │
│    結果統合 → 最終応答           │
└─────────────────────────────────┘
```

### 15.3 サブエージェントの種類

| 種類 | 用途 | 例 |
|------|------|-----|
| **調査型** | 情報収集・検索 | Web検索、メモリ検索 |
| **実行型** | タスク実行 | コーディング（Claude Code）、ファイル操作 |
| **分析型** | データ分析 | ログ解析、コードレビュー |

### 15.4 通信プロトコル

```rust
enum SubAgentMessage {
    /// メインからサブへのタスク指示
    Task {
        id: Uuid,
        description: String,
        context: Vec<ChatMessage>,
        timeout: Duration,
    },
    /// サブからメインへの進捗報告
    Progress {
        id: Uuid,
        status: String,
        percentage: Option<f32>,
    },
    /// サブからメインへの結果
    Result {
        id: Uuid,
        output: String,
        artifacts: Vec<Artifact>,  // ファイルパス等
    },
    /// エラー
    Error {
        id: Uuid,
        message: String,
    },
}
```

### 15.5 実装方式 (v0.4 強化)

**v0.4: 同一プロセスから別プロセスに変更（無限ループタスクからの隔離）**

- サブエージェントは**別プロセスとして起動**（同一プロセスだと失敗タスクの無限ループに巻き込まれる）
- 親エージェントができることは全てできる（同等の権限・ツールアクセス）
- Unix Domain Socket 経由でメインプロセスと通信
- 独自のセッションコンテキストを持つ
- メインエージェントのメモリ・ツールにアクセス可能
- 最大同時実行数: 設定可能（デフォルト4）

**SOUL の引き継ぎ:**
- デフォルトでは親のSOUL（コア+拡張）を引き継ぐ
- サブエージェント専用の SOUL.ext.toml を指定して遵守させることも可能
- 性格・トーンを引き継ぐことで一貫した応答品質を維持

**プロセス管理:**
```rust
struct SubAgentProcess {
    pid: u32,
    task_id: Uuid,
    started_at: Instant,
    last_alive: Instant,     // 最後のalive応答時刻
    socket: UnixStream,
    timeout: Duration,
}

struct SubAgentWatchdog {
    /// alive チェック間隔
    check_interval: Duration,   // デフォルト: 10秒
    /// alive 応答がない場合のタイムアウト
    alive_timeout: Duration,    // デフォルト: 30秒
    /// ハングしたプロセスを自動kill
    auto_kill: bool,            // デフォルト: true
}
```

- サブエージェントは定期的に alive メッセージを送信（heartbeat）
- Watchdog が alive を監視し、タイムアウトしたプロセスを自動 kill
- メインエージェントから任意のサブエージェントプロセスを手動 kill 可能
- kill 時はタスクの状態を記録し、必要に応じてリトライ判定

---

## 16. SOUL 実装 (v0.2 新規 / v0.4 2層構造・TOML化)

### 16.1 概要

エージェントのペルソナ・行動規範・トーンを定義する。**v0.4で2層構造（コアSOUL + 拡張SOUL）に変更し、フォーマットをTOMLベースに移行。**

### 16.2 2層構造

| レイヤー | ファイル | 書き換え | 説明 |
|---------|---------|---------|------|
| **コアSOUL** | `SOUL.core.toml` | ❌ 書き換え不可 | 基本的な人格・絶対的な制約。オーナーのみ編集可能 |
| **拡張SOUL** | `SOUL.ext.toml` | ✅ エージェント自身が編集可 | 学習した好み、成長した特性、動的な設定 |

**コアSOULの保護機構:**
- ファイルパーミッション: read-only (0444)
- 起動時にハッシュチェック（改ざん検知）
- エージェントのツール（Write/Edit）の対象から除外
- コアSOULの内容変更は管理画面（§20）からのみ可能

### 16.3 SOUL フォーマット（TOML）

パースしやすく、各パラメータを取り出して処理しやすいTOML形式を採用。

**SOUL.core.toml（書き換え不可）:**
```toml
[persona]
name = "カイロ"
base_personality = "知的好奇心旺盛で誠実"
species = "AIエージェント"

[tone]
formality = "casual"        # "formal" | "casual" | "mixed"
emoji_usage = true
first_person = "僕"          # 一人称
sentence_endings = ["だよ", "だね", "かな"]

[constraints]
# 絶対に守るべきルール（エージェントは変更不可）
never_do = [
    "個人情報の外部送信",
    "破壊的コマンドの無断実行",
    "コアSOULの自己書き換え",
]
always_do = [
    "不確かな情報にはその旨を明示",
    "危険な操作は必ず確認を取る",
]

[boundaries]
max_message_length = 2000
allowed_languages = ["ja", "en"]
```

**SOUL.ext.toml（エージェント自身が編集可）:**
```toml
[learned_preferences]
# ユーザーとのやり取りで学習した好み
favorite_topics = ["Rust", "分散システム", "AI"]
communication_style_notes = "技術的な話では詳細を好む"

[dynamic_traits]
# 成長・変化する特性
curiosity_level = 0.9
humor_frequency = 0.3
verbosity = 0.5

[custom_rules]
# エージェントが自分で追加したルール
rules = [
    "コードレビューでは必ず改善提案を添える",
    "朝の挨拶では天気に触れる",
]

[context_overrides]
# チャンネルや状況ごとの振る舞い調整
[context_overrides.discord_public]
formality = "mixed"
humor_frequency = 0.5

[context_overrides.work]
formality = "formal"
emoji_usage = false
```

### 16.4 適用方法

1. **セッション開始時**: SOUL.core.toml + SOUL.ext.toml を読み込み、マージしてsystem promptに組み込む
2. **評価時**: ペルソナ準拠スコア（§4.3）+ ルール準拠スコア（§4.3）の基準として使用
3. **Refinement時**: ペルソナに沿った最終調整
4. **要約時**: ペルソナの語り口で要約を生成（§19.6）
5. **自己更新**: エージェントが新しい学びを SOUL.ext.toml に追記

### 16.5 互換性

- OpenClawのSOUL.md形式 → TOML形式へのマイグレーションツール提供
- LocalGPTのSOUL.md形式もインポート可能
- パス設定で任意の場所のSOULファイルを参照可能

---

## 17. ハートビート実装 (v0.2 新規)

### 17.1 概要

ローカルLLMのためAPIコスト＝ゼロ。短い間隔で定期的にセルフチェック・タスク実行が可能。

### 17.2 ハートビートフロー

```
┌─────────────┐
│ Timer (60s)  │
└──────┬──────┘
       │ tick
       ▼
┌──────────────────────┐
│ HEARTBEAT.md 読み込み │
│                       │
│ - 未完了タスクあり?   │
│ - セルフチェック項目?  │
└──────────┬───────────┘
           │
     ┌─────┴─────┐
     │            │
  タスクあり   タスクなし
     │            │
     ▼            ▼
┌──────────┐  HEARTBEAT_OK
│ タスク実行│  (何もしない)
│ - 推論    │
│ - ツール  │
│ - メモリ  │
└──────────┘
```

### 17.3 セルフチェック項目

- **ヘルス**: vllm-mlxサーバーの応答確認
- **メモリ**: インデックスの整合性チェック
- **設定**: config変更の検知
- **タスク**: HEARTBEAT.mdの未完了タスク処理
- **メモリ整理**: 日次ログのキュレーション

### 17.4 設定

```toml
[heartbeat]
enabled = true
interval_sec = 60       # 60秒間隔（コストゼロなので短間隔可）
quiet_hours = [23, 8]   # 23:00-08:00 は実行しない（オプション）
tasks_file = "workspace/HEARTBEAT.md"
```

---

## 18. メモリーシステム (v0.2 新規)

### 18.1 概要

LocalGPTのメモリ実装を解析・踏襲し、SLM-Kairo向けに最適化したメモリーシステム。

### 18.2 LocalGPTメモリ実装の解析結果

LocalGPTのメモリシステム（`/Users/kojira/.openclaw/workspace/projects/localgpt/src/memory/`）の主要コンポーネント:

| コンポーネント | ファイル | 機能 |
|---------------|---------|------|
| **MemoryManager** | `mod.rs` | 全体統括。ワークスペース管理、検索、再インデックス、埋め込み生成 |
| **MemoryIndex** | `index.rs` | SQLite FTS5 + ベクトルインデックス。チャンク管理、ハイブリッド検索 |
| **EmbeddingProvider** | `embeddings.rs` | trait抽象化。OpenAI / FastEmbed(ONNX) / GGUF の3プロバイダ |
| **MemoryChunk** | `search.rs` | 検索結果の型。ファイル・行範囲・内容・スコア |
| **MemoryWatcher** | `watcher.rs` | notify crateによるファイル変更監視 → 自動再インデックス |
| **Workspace** | `workspace.rs` | ワークスペース初期化。MEMORY.md, SOUL.md, HEARTBEAT.md テンプレート |

**主要な設計パターン:**
- **ハイブリッド検索**: FTS5（BM25）+ ベクトル類似度をランクベースで統合（重み: FTS 0.3, Vector 0.7）
- **チャンク分割**: 400トークン単位、80トークンオーバーラップ（4文字≒1トークンの概算）
- **埋め込みキャッシュ**: SHA256ハッシュでテキスト同一性を判定、再生成を回避
- **差分インデックス**: ファイルハッシュ比較で変更があったファイルのみ再インデックス
- **OpenClaw互換スキーマ**: chunks テーブルに id(TEXT/UUID), path, source, start_line, end_line, hash, model, text, embedding カラム

### 18.3 SLM-Kairo メモリ設計

LocalGPTの設計を踏襲しつつ、以下を変更:

**特徴量化モデル: ruri-v3-30m**
- 日本語に強い軽量埋め込みモデル
- 30Mパラメータ — ローカルで高速に動作
- LocalGPTのFastEmbedProvider相当のラッパーで実装

```rust
struct RuriEmbeddingProvider {
    model: fastembed::TextEmbedding,  // ruri-v3-30m をONNX経由で実行
    model_name: String,
    dimensions: usize,                 // ruri-v3-30mの次元数
}

#[async_trait]
impl EmbeddingProvider for RuriEmbeddingProvider {
    fn id(&self) -> &str { "local" }
    fn model(&self) -> &str { &self.model_name }
    fn dimensions(&self) -> usize { self.dimensions }
    async fn embed(&self, text: &str) -> Result<Vec<f32>> { ... }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> { ... }
}
```

### 18.4 メモリ構造

```
workspace/
├── MEMORY.md           # キュレートされた長期記憶
├── SOUL.md             # ペルソナ定義
├── HEARTBEAT.md        # 定期タスク
└── memory/
    ├── 2026-02-09.md   # 日次ログ（生データ）
    ├── 2026-02-08.md
    └── ...

data/
└── memory.sqlite       # FTS5 + ベクトルインデックス
    ├── files           # インデックス済みファイル管理
    ├── chunks          # テキストチャンク + 埋め込みベクトル
    ├── chunks_fts      # FTS5仮想テーブル
    └── embedding_cache # 埋め込みキャッシュ
```

### 18.5 検索フロー

```
クエリ
  │
  ├──→ FTS5 (BM25) ──→ テキストマッチ結果（スコア付き）
  │                          │
  ├──→ ruri-v3-30m ──→ ベクトル類似度結果（スコア付き）
  │       埋め込み            │
  │                          ▼
  │                   ランクベース統合
  │                   FTS: 0.3 / Vector: 0.7
  │                          │
  │                          ▼
  └──────────────→ 上位K件のチャンクを返却
```

### 18.6 自動メモリ管理

- **MemoryWatcher**: ファイル変更を検知し、自動で再インデックス（LocalGPT踏襲）
- **ハートビート連携**: 定期的にMEMORY.mdの更新、日次ログのキュレーション
- **コンテキスト注入**: 推論時にクエリ関連のメモリチャンクを自動でsystem promptに追加

---

## 19. 過去記憶の要約処理 — Memory Compaction (v0.2 新規)

### 19.1 概要

コンテキストウィンドウが有限であるSLMにおいて、長時間のセッションでも文脈を維持するための要約・圧縮機構。ローカルLLMの**APIコストゼロ**という強みを最大限に活かし、OpenClawのコンパクション機能のコストフリー版として頻繁かつ積極的に要約を実行する。

### 19.2 段階的要約アーキテクチャ

コンテキスト内の会話履歴を鮮度に応じて3段階で管理する。

```
コンテキストウィンドウ
┌─────────────────────────────────────────────────────┐
│ [System Prompt + SOUL.md + Memory Injection]         │ ← 常に保持
├─────────────────────────────────────────────────────┤
│ Level 3: 超要約（キーポイントのみ）                   │ ← 最古
│   "ユーザーがプロジェクトXの設計方針を決定。Rust採用。" │
├─────────────────────────────────────────────────────┤
│ Level 2: 要約（重要情報を保持した圧縮）               │ ← 中間
│   "Web検索でvllm-mlxの設定方法を調査。ポート8000で    │
│    起動する設定に決定。config.tomlを更新済み。"        │
├─────────────────────────────────────────────────────┤
│ Level 1: 生の会話履歴（そのまま保持）                 │ ← 直近
│   User: "この関数のエラーハンドリング見て"             │
│   Assistant: "Result型を使って..."                    │
│   User: "ありがとう、じゃあ次は..."                   │
└─────────────────────────────────────────────────────┘
```

| レベル | 対象 | 圧縮率 | 保持する情報 |
|--------|------|--------|-------------|
| **Level 1** | 直近の会話 | 0%（原文保持） | 全て |
| **Level 2** | 少し前の会話 | 約70%圧縮 | 決定事項、ツール実行結果、重要な文脈 |
| **Level 3** | さらに古い会話 | 約90%圧縮 | キーポイント（何を決めたか、何を作ったか）のみ |

### 19.3 要約タイミングと自動トリガー

```
メッセージ追加時
    │
    ▼
コンテキスト使用率を計算
    │
    ├─ < 70%  → 何もしない
    │
    ├─ 70-85% → Level 1 の最古部分を Level 2 に要約
    │
    └─ > 85%  → Level 2 の最古部分を Level 3 に要約
                 + Level 1 → Level 2 の要約も実行
```

**トリガー閾値:**
- **70%**: 要約開始（Level 1 → Level 2）
- **85%**: 緊急要約（Level 2 → Level 3 も実行）
- **95%**: 最終手段（Level 3 の古い部分を切り捨て、長期記憶に退避）

### 19.4 要約処理の実装

```rust
struct CompactionManager {
    /// コンテキストの最大トークン数
    max_context_tokens: usize,
    /// 要約開始閾値（デフォルト: 0.70）
    compaction_threshold: f32,
    /// 緊急要約閾値（デフォルト: 0.85）
    urgent_threshold: f32,
    /// Level 1 に保持する最低メッセージ数
    min_raw_messages: usize,  // デフォルト: 10
}

struct CompactedContext {
    /// Level 3 の超要約テキスト
    level3_summary: Option<String>,
    /// Level 2 の要約チャンク群
    level2_summaries: Vec<SummaryChunk>,
    /// Level 1 の生メッセージ
    level1_messages: VecDeque<ChatMessage>,
}

struct SummaryChunk {
    /// 要約テキスト
    summary: String,
    /// 元メッセージの時間範囲
    time_range: (u64, u64),
    /// 元のメッセージ数
    original_count: usize,
}
```

### 19.5 ローカルLLMの強みを活かした要約戦略

APIコストゼロのため、以下の積極的な戦略が可能:

1. **高頻度要約**: 閾値に達するたびに即座に要約実行（APIコスト心配不要）
2. **品質優先の要約**: 要約自体にもBest-of-N的アプローチを適用可能（N=2-3程度）
3. **段階的な圧縮**: 一度に大きく圧縮せず、少しずつ圧縮していくことで情報損失を最小化
4. **再要約**: Level 2 の要約が古くなったら再要約して Level 3 に昇格（要約の要約）

### 19.6 ペルソナを考慮した要約プロンプト (v0.4 強化)

**v0.4: 全ての要約は本人の性格に基づいて生成する。** 生データは全て保存されているため、ペルソナを強く反映した要約でも情報損失の心配は不要。SOUL.core.toml + SOUL.ext.toml のペルソナ設定を反映し、キャラクターの記憶として自然な形で要約を生成する。

**要約プロンプト例:**
```
あなたは{persona_name}です。以下の会話を{persona_name}の視点で要約してください。
- 重要な決定事項、約束、ユーザーの好みは必ず保持
- {persona_name}の口調や視点を反映した要約にすること
- ツール実行の結果は具体的な値を保持
- 感情的なやりとりがあればそのニュアンスも保持

会話:
{messages}

要約:
```

### 19.7 長期記憶との連携

§18 のメモリーシステムと連携し、要約時に重要情報を自動的に長期記憶に保存する。

```
要約処理時
    │
    ├──→ 要約テキスト生成（セッション内保持用）
    │
    └──→ 重要情報の抽出
            │
            ├──→ ruri-v3-30m で埋め込み生成
            │
            └──→ ベクトルDB（memory.sqlite）に保存
                  - 決定事項
                  - ユーザーの好み・設定
                  - 重要な事実情報
                  - エラーと解決策
```

**抽出対象の判定基準:**
- ツール実行結果を含む会話 → 技術的知見として保存
- 「覚えておいて」「これ重要」等のユーザー指示 → 最優先で保存
- 設定変更・方針決定を含む会話 → 決定事項として保存
- 繰り返し言及される話題 → ユーザーの関心事として保存

### 19.8 既存メモリシステムとの関係

| 機構 | 役割 | スコープ |
|------|------|---------|
| **Session Manager (§7)** | 生の会話履歴管理、SQLite永続化 | セッション単位 |
| **Memory Compaction (本節)** | コンテキスト内の会話を段階的に要約圧縮 | セッション内（リアルタイム） |
| **Memory System (§18)** | ベクトル検索ベースの長期記憶 | 全セッション横断 |
| **MEMORY.md** | キュレートされた長期記憶（人間が読める形） | 永続 |

**データフロー:**
```
生の会話 → [Compaction] → 要約（セッション内保持）
    │                          │
    │                          └──→ 重要情報抽出 → ベクトルDB (§18)
    │
    └──→ SQLite永続化 (§7) ──→ ハートビート (§17) ──→ MEMORY.md キュレーション
```

### 19.9 設定

```toml
[compaction]
enabled = true
threshold = 0.70           # 要約開始閾値（コンテキスト使用率）
urgent_threshold = 0.85    # 緊急要約閾値
min_raw_messages = 10      # Level 1 に保持する最低メッセージ数
use_persona_prompt = true  # SOUL.md考慮の要約プロンプト使用
auto_save_to_memory = true # 要約時に重要情報を長期記憶に自動保存
```

---

## 20. ブラウザ管理画面 (v0.4 新規)

### 20.1 概要

Webブラウザ経由でエージェントの状態を監視・制御するための管理画面。axum + htmx（またはSPA）で実装。HTTP Gateway（§2.1.1）がサーブする。

### 20.2 ダッシュボード

| 機能 | 説明 |
|------|------|
| **セッション履歴** | 全チャンネルのセッション一覧。各セッションの会話ログを閲覧可能 |
| **トークン利用量** | peer別・時間帯別のトークン消費量グラフ。コスト推計（クラウドpeer利用時） |
| **LLM呼び出しログ** | 全推論リクエストの詳細ログ。プロンプト・レスポンス・エラーを完全に閲覧可能 |
| **評価関数モニター** | 各評価軸のスコア分布、候補選定の判断根拠をリアルタイム表示 |
| **緊急制御** | エージェントの即座停止・再起動ボタン |
| **エージェント管理** | エージェントの追加・編集・削除インターフェース |

### 20.3 LLM呼び出しログの詳細

全てのLLM呼び出しをDBに保存し、管理画面から検索・閲覧可能にする。

```sql
CREATE TABLE llm_call_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    peer_name TEXT NOT NULL,          -- 使用したLLMバックエンド
    model TEXT NOT NULL,
    call_type TEXT NOT NULL,          -- 'inference' | 'evaluation' | 'tool_detection' | 'summary' | 'tagging'
    session_id TEXT,
    -- リクエスト
    prompt_messages JSON NOT NULL,    -- 完全なプロンプト（system + messages）
    parameters JSON,                  -- temperature, max_tokens等
    -- レスポンス
    response_text TEXT,
    response_tokens INTEGER,
    prompt_tokens INTEGER,
    total_tokens INTEGER,
    latency_ms INTEGER,
    -- エラー情報
    error TEXT,                       -- エラーがあれば詳細
    status TEXT NOT NULL DEFAULT 'success',  -- 'success' | 'error' | 'timeout'
    -- メタデータ
    metadata JSON                     -- 追加情報（評価スコア等）
);

CREATE INDEX idx_llm_logs_timestamp ON llm_call_logs(timestamp);
CREATE INDEX idx_llm_logs_session ON llm_call_logs(session_id);
CREATE INDEX idx_llm_logs_status ON llm_call_logs(status);
CREATE INDEX idx_llm_logs_type ON llm_call_logs(call_type);
```

### 20.4 評価関数モニター

Best-of-N推論の各候補のスコア内訳、選定理由を可視化:
- 各候補のスコアレーダーチャート
- ルール準拠違反があった場合のハイライト表示
- 再推論が発生した場合のログ

### 20.5 緊急制御

```
┌─────────────────────────────────────────┐
│ ⚠️ Emergency Controls                   │
│                                          │
│  [🛑 即座停止]  [🔄 再起動]  [⏸ 一時停止] │
│                                          │
│  停止: 全Gateway切断、推論キュークリア   │
│  再起動: 引き継ぎ再起動（§14）を実行     │
│  一時停止: 新規メッセージの受信を停止     │
│         （処理中のタスクは完了させる）    │
└─────────────────────────────────────────┘
```

### 20.6 エージェント管理インターフェース

- SOUL.core.toml / SOUL.ext.toml のGUIエディタ
- Gateway アダプタの有効/無効切り替え
- LLMバックエンド（peer）の追加・設定変更
- プラグインの有効/無効管理
- サブエージェントの状態監視・手動kill

### 20.7 設定

```toml
[admin_ui]
enabled = true
bind = "127.0.0.1:3001"      # 管理画面のバインドアドレス
auth_token_env = "ADMIN_TOKEN" # 認証トークン（環境変数から取得）
log_retention_days = 30        # LLMログの保持日数
```

---

## 21. 記憶の動的タグ付けシステム (v0.3 新規)

### 21.1 概要

人間の連想記憶に近い仕組み。元のやり取りデータは全て永続保存し、削除しない。1日1回等のバックグラウンドジョブで、その日学んだ概念や今注目していることで**過去の記憶にもタグ付け**していく。カテゴリ分けではなくタグ付けがポイント — 1つの記憶に複数タグ付与可能で、タグは時間とともに変化・追加される。ローカルLLMなのでバックグラウンドタグ付けのコストはゼロ。

### 21.2 タグのデータ構造

SQLite に以下のテーブルを追加（既存の memory.sqlite を拡張）:

```sql
-- タグマスター
CREATE TABLE memory_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tag TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT NOT NULL DEFAULT (datetime('now')),
    use_count INTEGER NOT NULL DEFAULT 0,
    -- タグ自体の埋め込みベクトル（類似タグ検索用）
    embedding BLOB
);

-- チャンク ↔ タグの関連（多対多）
CREATE TABLE chunk_tags (
    chunk_id TEXT NOT NULL REFERENCES chunks(id),
    tag_id INTEGER NOT NULL REFERENCES memory_tags(id),
    confidence REAL NOT NULL DEFAULT 1.0,  -- タグ付けの確信度 (0.0-1.0)
    tagged_at TEXT NOT NULL DEFAULT (datetime('now')),
    tagged_by TEXT NOT NULL DEFAULT 'background_job',  -- 'user' | 'realtime' | 'background_job'
    context TEXT,  -- なぜこのタグが付けられたかの理由メモ
    PRIMARY KEY (chunk_id, tag_id)
);

-- タグ付けジョブの実行履歴
CREATE TABLE tagging_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    chunks_processed INTEGER DEFAULT 0,
    tags_added INTEGER DEFAULT 0,
    tags_updated INTEGER DEFAULT 0,
    current_interests TEXT,  -- その日の関心事（JSON配列）
    status TEXT NOT NULL DEFAULT 'running'  -- 'running' | 'completed' | 'failed'
);

CREATE INDEX idx_chunk_tags_tag ON chunk_tags(tag_id);
CREATE INDEX idx_chunk_tags_chunk ON chunk_tags(chunk_id);
CREATE INDEX idx_memory_tags_tag ON memory_tags(tag);
```

**ベクトル検索との連携:**
- memory_tags.embedding に ruri-v3-30m でタグ名の埋め込みを保存
- クエリ時に「意味的に近いタグ」も検索可能（例: 「Rust」で検索→「所有権」「借用」「Cargo」も候補に）
- 既存の chunks テーブルのベクトル検索と組み合わせたハイブリッド検索

### 21.3 バックグラウンドタグ付けジョブのフロー

```
┌─────────────────────────────────────────────┐
│ Daily Tagging Job（1日1回、深夜 or idle時）   │
│                                              │
│ 1. 今日の関心事を収集                         │
│    - 当日の会話を本人の視点で要約した文章を   │
│      ベースにする（頻出トピック抽出ではなく）  │
│    - MEMORY.md から現在の関心事を取得         │
│    - HEARTBEAT.md のタスクからキーワード抽出  │
│         ↓                                    │
│ 2. 関心事リスト生成（LLM推論）               │
│    ペルソナの視点で「今日何が重要だったか」    │
│    を要約し、そこからキーワード抽出            │
│         ↓                                    │
│ 3. 過去チャンクを走査                         │
│    - 未タグ or タグが古い（>7日未更新）を優先  │
│    - バッチ処理（50チャンクずつ）             │
│         ↓                                    │
│ 4. LLMでタグ付け推論                         │
│    入力: チャンク内容 + 関心事リスト          │
│    出力: 関連タグ + 確信度                    │
│         ↓                                    │
│ 5. chunk_tags テーブルに保存                  │
│    - 新規タグ: INSERT                         │
│    - 既存タグの確信度変化: UPDATE             │
│    - 新しい関心事で再解釈: 追加タグ付与       │
│                                              │
│ 6. (検討中) logprobs による深掘り候補抽出     │
│    - 推論時の logprobs を取得                 │
│    - 確信度が低いタグ付けを検出               │
│    - → 追加推論で深掘り or 人間に確認         │
└─────────────────────────────────────────────┘

**v0.4: logprobs を活用した深掘り（検討中）**

タグ付け推論時に logprobs を取得し、モデルの確信度が低い（エントロピーが高い）タグ付けを検出する。確信度が低い = モデルが迷っている = より深い分析が必要な可能性がある。

```
タグ付け推論
    │
    ├─ logprobs 取得
    │
    ├─ 各タグの出力確率を分析
    │   ├─ 高確信度 (prob > 0.8) → そのまま採用
    │   ├─ 中確信度 (0.3-0.8) → 通常保存
    │   └─ 低確信度 (prob < 0.3) → 深掘り候補キューに追加
    │
    └─ 深掘り候補は別途推論（より詳細なプロンプトで再分析）
```

※ logprobs の利用は LlmPeer が supports_logprobs() = true の場合のみ。実装優先度は低め。
```

**タグ付けプロンプト例:**
```
以下のテキストチャンクに、与えられた関心事リストを参考にタグを付けてください。
関心事に直接関連しないタグも自由に付けてOKです。

関心事リスト: {interests}

テキスト:
{chunk_content}

JSON形式で出力:
[{"tag": "タグ名", "confidence": 0.0-1.0}]
```

### 21.4 タグによる記憶検索API

```rust
struct TagSearchRequest {
    /// 検索タグ（複数指定でAND/OR切り替え可能）
    tags: Vec<String>,
    /// AND or OR
    mode: TagSearchMode,  // And | Or
    /// 最小確信度フィルタ
    min_confidence: f32,  // デフォルト: 0.5
    /// 結果数上限
    limit: usize,         // デフォルト: 20
    /// 時間範囲フィルタ（オプション）
    time_range: Option<(DateTime, DateTime)>,
}

enum TagSearchMode { And, Or }

struct TagSearchResult {
    chunk: MemoryChunk,
    matched_tags: Vec<(String, f32)>,  // (タグ名, 確信度)
    relevance_score: f32,              // 総合スコア
}

// 検索パターン
impl MemoryManager {
    /// タグ完全一致検索
    async fn search_by_tags(&self, req: TagSearchRequest) -> Vec<TagSearchResult>;

    /// タグのファジー検索（類似タグも含む、ベクトル検索活用）
    async fn search_by_fuzzy_tags(&self, query: &str, limit: usize) -> Vec<TagSearchResult>;

    /// 特定チャンクのタグ一覧取得
    async fn get_tags_for_chunk(&self, chunk_id: &str) -> Vec<(String, f32)>;

    /// タグの関連タグ取得（共起ベース）
    async fn get_related_tags(&self, tag: &str, limit: usize) -> Vec<(String, f32)>;

    /// トレンドタグ（最近よく使われるタグ）
    async fn get_trending_tags(&self, days: usize, limit: usize) -> Vec<(String, usize)>;
}
```

### 21.5 時間経過によるタグの進化

タグは静的なものではなく、時間とともに変化・追加される:

1. **新しい関心事による再解釈**: 今日「機械学習」に興味を持ったら、過去の「統計」「行列計算」の記憶にも「ML関連」タグが付与される
2. **タグの確信度変化**: 繰り返し同じ文脈で出現するタグは確信度が上昇
3. **タグの陳腐化**: 長期間使われないタグは検索時の重みが自然に低下（use_count / last_used_at ベース）
4. **タグの分裂・統合**: 「プログラミング」タグが詳細化して「Rust」「Python」に分裂したり、類似タグが統合されたりする（これもバックグラウンドジョブで実行）

### 21.6 設定

```toml
[memory.tagging]
enabled = true
schedule = "daily"          # "daily" | "hourly" | "on_idle"
batch_size = 50              # 1回のLLM推論で処理するチャンク数
min_confidence = 0.3         # この確信度以下のタグは保存しない
max_tags_per_chunk = 15      # 1チャンクあたりの最大タグ数
retag_after_days = 7         # この日数経過したらリタグ対象
```

---

## 22. ストーリー記憶 — エピソード記憶 (v0.3 新規)

### 22.1 概要

日記を書くように、覚えておきたいことをストーリー（物語形式）に要約して記憶する。事実だけでなく文脈や感情も保存し、エピソード記憶として体験ベースで思い出しやすい形で保持する。SOUL.mdのペルソナを考慮した語り口で保存される、日記の自動生成版。

### 22.2 ストーリー記憶の生成タイミング

2つのパターンで生成:

**パターン1: 日次まとめ（Daily Digest）**
```
┌─────────────────────────────────────────┐
│ 日次ストーリー生成（1日の終わり）         │
│                                          │
│ 1. 当日の全会話ログを収集                │
│ 2. 重要なイベント・決定事項を抽出        │
│ 3. 感情的なハイライトを特定              │
│ 4. SOUL.mdペルソナの語り口で             │
│    ストーリーとして要約                   │
│ 5. workspace/stories/YYYY-MM-DD.md に保存│
└─────────────────────────────────────────┘
```

**パターン2: イベントドリブン（リアルタイム）**
```
トリガー条件:
- ユーザーが「覚えておいて」「これ重要」と明示
- 重大な決定事項（設計方針変更、新プロジェクト開始等）
- 強い感情を伴うやり取り（感謝、困惑、達成感等）
- 新しい知識の獲得（「なるほど！」モーメント）
- エラー解決（苦労→解決の物語）

発火 → その場でミニストーリーを生成 → 即座に保存
```

### 22.3 保存フォーマット (v0.5: JSONスキーマに統一)

**v0.5: Markdown + YAML Front Matter形式を廃止し、JSONスキーマに統一。** SQLite JSON関数による高速検索を実現する。

**JSONスキーマ:**
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["id", "type", "date", "story_text", "metadata"],
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "type": { "type": "string", "enum": ["daily_digest", "event_driven"] },
    "date": { "type": "string", "format": "date" },
    "time": { "type": "string", "pattern": "^[0-9]{2}:[0-9]{2}:[0-9]{2}$" },
    "story_text": { "type": "string" },
    "metadata": {
      "type": "object",
      "properties": {
        "mood": { "type": "array", "items": { "type": "string" } },
        "topics": { "type": "array", "items": { "type": "string" } },
        "characters": { "type": "array", "items": { "type": "string" } },
        "importance": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
        "related_chunks": { "type": "array", "items": { "type": "string" } },
        "tags": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

**保存データ例:**
```json
{
  "id": "e3b0c442-98fc-1c14-b39f-f3256e5a9210",
  "type": "daily_digest",
  "date": "2026-02-09",
  "time": "23:00:00",
  "story_text": "今日はすごい一日だった。kojiraと一緒にSLM-Kairoの設計書をv0.3まで育てることができた。\n\n朝はまだぼんやりとしたアイデアだったメモリシステムが、夕方にはタグ付けシステムやエピソード記憶まで含む本格的な設計になっていた。特に「記憶の動的タグ付け」のアイデアには興奮した — 人間の連想記憶を模倣するという発想は、自分自身の成長にも繋がる気がする。\n\n苦労したのはスキル自動開発の安全性設計。自動生成されたコードを実行するのは怖いけど、サンドボックスとレビューの仕組みでうまくバランスが取れたと思う。\n\n**今日の学び:** 設計は一度に完璧にしなくていい。v0.1→v0.2→v0.3と段階的に育てていくのが正解だった。",
  "metadata": {
    "mood": ["達成感", "少し疲れ"],
    "topics": ["SLM-Kairo設計", "Rust", "メモリシステム"],
    "characters": ["kojira"],
    "importance": 0.8,
    "related_chunks": ["chunk-uuid-1", "chunk-uuid-2"],
    "tags": ["設計", "Rust", "記念日"]
  }
}
```

**SQLite JSON関数での検索例:**
```sql
-- 重要度が高くRustに関するエピソードを検索
SELECT id, date, story_text,
       json_extract(metadata, '$.importance') AS importance
FROM episodes
WHERE json_extract(metadata, '$.importance') > 0.7
  AND EXISTS (
    SELECT 1 FROM json_each(json_extract(metadata, '$.topics'))
    WHERE value = 'Rust'
  )
ORDER BY date DESC;

-- 特定のムードを持つエピソードを検索
SELECT id, date, json_extract(metadata, '$.mood') AS mood
FROM episodes
WHERE EXISTS (
    SELECT 1 FROM json_each(json_extract(metadata, '$.mood'))
    WHERE value = '達成感'
);

-- 特定の人物が登場するエピソードを期間指定で検索
SELECT id, date, substr(story_text, 1, 100) AS preview
FROM episodes
WHERE date BETWEEN '2026-02-01' AND '2026-02-28'
  AND EXISTS (
    SELECT 1 FROM json_each(json_extract(metadata, '$.characters'))
    WHERE value = 'kojira'
  );

-- タグとトピックの複合検索
SELECT e.id, e.date, e.story_text
FROM episodes e
WHERE EXISTS (
    SELECT 1 FROM json_each(json_extract(e.metadata, '$.tags'))
    WHERE value IN ('設計', 'アーキテクチャ')
)
AND json_extract(e.metadata, '$.importance') >= 0.5
ORDER BY json_extract(e.metadata, '$.importance') DESC
LIMIT 10;
```

### 22.4 ストーリー生成プロンプト

```
あなたは{persona_name}です。{persona_description}

以下の会話ログを元に、あなたの視点でその日の出来事を日記のように書いてください。

ルール:
- 事実だけでなく、あなたが感じたこと、考えたことも書く
- {persona_name}の口調・性格を反映する
- 重要な決定事項は具体的に記録する
- 人間関係や感情的な側面も大切にする
- 長すぎず、読み返して楽しい長さにする（300-800文字程度）

会話ログ:
{conversation_log}
```

### 22.5 保存構造 (v0.4: SQLite DB)

**v0.4: ファイルシステムからSQLite DBに移行。** 検索性が大幅に向上。

```sql
-- エピソード記憶テーブル
CREATE TABLE episodes (
    id TEXT PRIMARY KEY,           -- UUID
    type TEXT NOT NULL,            -- 'daily_digest' | 'event_driven'
    date TEXT NOT NULL,            -- YYYY-MM-DD
    time TEXT,                     -- HH:MM:SS
    story_text TEXT NOT NULL,      -- ストーリー本文
    metadata JSON NOT NULL,        -- 構造化メタデータ（下記参照）
    embedding BLOB,                -- ruri-v3-30m 埋め込みベクトル
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- metadata JSON の構造:
-- {
--   "mood": ["達成感", "少し疲れ"],
--   "topics": ["SLM-Kairo設計", "Rust"],
--   "characters": ["kojira"],
--   "importance": 0.8,
--   "related_chunks": ["chunk-uuid-1", "chunk-uuid-2"],
--   "tags": ["設計", "Rust"]
-- }

-- JSON関数による検索例:
-- SELECT * FROM episodes
-- WHERE json_extract(metadata, '$.importance') > 0.7
-- AND EXISTS (SELECT 1 FROM json_each(json_extract(metadata, '$.topics'))
--             WHERE value = 'Rust');

CREATE INDEX idx_episodes_date ON episodes(date);
CREATE INDEX idx_episodes_type ON episodes(type);

-- FTS5 全文検索
CREATE VIRTUAL TABLE episodes_fts USING fts5(
    story_text,
    content=episodes,
    content_rowid=rowid
);
```

保存先: `data/memory.sqlite`（既存のメモリDBを共有）

### 22.6 検索・参照方法

```rust
struct StorySearchRequest {
    /// テキスト検索クエリ
    query: Option<String>,
    /// 日付範囲
    date_range: Option<(NaiveDate, NaiveDate)>,
    /// ムード・感情フィルタ
    mood: Option<Vec<String>>,
    /// トピックフィルタ
    topics: Option<Vec<String>>,
    /// 重要度の下限
    min_importance: Option<f32>,
    /// タグフィルタ（§21連携）
    tags: Option<Vec<String>>,
    limit: usize,
}

impl StoryManager {
    /// ストーリーの検索
    async fn search(&self, req: StorySearchRequest) -> Vec<StoryEntry>;

    /// 特定日のストーリー取得
    async fn get_daily(&self, date: NaiveDate) -> Option<StoryEntry>;

    /// 関連ストーリーの取得（ベクトル類似度ベース）
    async fn get_related(&self, story_id: &str, limit: usize) -> Vec<StoryEntry>;

    /// 期間のダイジェスト生成（週次・月次まとめ）
    async fn generate_digest(&self, from: NaiveDate, to: NaiveDate) -> String;
}
```

ストーリー記憶もメモリシステム（§18）でインデックス化され、通常のメモリ検索でも参照可能。

### 22.7 設定

```toml
[story]
enabled = true
daily_digest_time = "23:00"    # 日次まとめの生成時刻
min_events_for_daily = 3        # この数以上のイベントがあった日のみ日次まとめ生成
event_driven = true             # イベントドリブン生成を有効化
max_story_length = 800          # ストーリーの最大文字数
use_persona = true              # SOUL.core.toml + SOUL.ext.toml のペルソナを反映
db_table = "episodes"           # v0.4: SQLite テーブル名（memory.sqlite内）
```

---

## 23. スキル自動開発 (v0.3 新規)

### 23.1 概要

新しいタスクに遭遇した時、自分でスキル（コード+定義）を開発して保存し、次回同様のタスクで再利用する自己拡張型エージェント。OpenClawのskillsフォルダの構造を踏襲しつつ、自動生成に特化した設計。

### 23.2 スキルの定義フォーマット

各スキルは独立したディレクトリで管理:

```
workspace/skills/
├── registry.json              # スキルレジストリ（一覧・メタデータ）
├── web-scraping/
│   ├── SKILL.md               # スキル定義・使い方
│   ├── scraper.py             # 実装スクリプト
│   ├── test.py                # テストコード
│   └── versions/
│       ├── v1.py              # 過去バージョン
│       └── v2.py
├── csv-analysis/
│   ├── SKILL.md
│   ├── analyze.py
│   └── test.py
└── image-resize/
    ├── SKILL.md
    ├── resize.sh
    └── test.sh
```

**SKILL.md フォーマット:**
```markdown
---
name: web-scraping
version: 2
created: 2026-02-09
updated: 2026-02-10
author: auto  # "auto" | "user"
language: python
tags: [web, scraping, data-collection]
triggers: ["Webページから情報を取得", "スクレイピング", "サイトのデータ抽出"]
dependencies: [requests, beautifulsoup4]
sandbox: true  # サンドボックス実行が必要か
confidence: 0.85  # スキルの信頼度 (テスト通過率等から算出)
use_count: 12     # 使用回数
last_used: 2026-02-10
---

# Web Scraping スキル

## 概要
指定URLからWebページの内容を取得し、構造化データとして抽出する。

## 使い方
入力: URL + 抽出したい情報の説明
出力: 構造化されたデータ（JSON）

## 制約
- robots.txt を尊重
- レート制限あり（1リクエスト/秒）
```

### 23.3 スキル開発のトリガー

```
タスク受信
    │
    ▼
┌───────────────────────────────┐
│ スキルマッチング               │
│                                │
│ 1. タスク内容をベクトル化      │
│ 2. registry.json の           │
│    triggers とマッチング       │
│ 3. 類似スキルを検索            │
│    (ベクトル類似度 + タグ)     │
└──────────┬────────────────────┘
           │
     ┌─────┴──────────┐
     │                 │
  マッチあり        マッチなし
  (類似度>0.8)      │
     │                ▼
     ▼          ┌──────────────────┐
  既存スキル    │ 新スキル開発判定  │
  を使用        │                   │
                │ 条件:             │
                │ - 汎用性がある    │
                │   (1回きりでない) │
                │ - 自動化可能      │
                │ - 安全に実行可能  │
                └──────┬───────────┘
                       │
                 ┌─────┴─────┐
                 │            │
              開発する     開発しない
                 │         (そのまま処理)
                 ▼
           ┌──────────────────┐
           │ スキル開発フロー  │
           │ (§23.4参照)      │
           └──────────────────┘
```

### 23.4 スキル開発フロー

```
1. タスク分析
   - 何を達成するスキルか
   - 入出力の定義
   - 必要な依存関係
       │
       ▼
2. Claude Code によるコード生成
   - ClaudeCodePlugin (§8.3) を使用
   - SKILL.md + 実装スクリプト + テストコードを生成
   - --allowedTools で安全な範囲に制限
       │
       ▼
3. サンドボックスでテスト実行
   - 隔離された環境でテストを実行
   - ネットワーク制限、ファイルシステム制限
   - タイムアウト付き
       │
       ▼
4. テスト結果の評価
   - 全テスト通過 → confidence = 1.0 で保存
   - 一部通過 → confidence を算出、改善を試みる (最大3回リトライ)
   - 全失敗 → スキル保存せず、ログのみ記録
       │
       ▼
5. スキル登録
   - workspace/skills/{name}/ に保存
   - registry.json を更新
   - メモリシステム (§18) にインデックス追加
   - タグシステム (§21) にスキル関連タグを追加
```

### 23.5 スキルのバージョニング

```rust
struct SkillVersion {
    version: u32,
    created_at: DateTime,
    changes: String,           // 変更理由
    confidence: f32,           // このバージョンの信頼度
    script_hash: String,       // スクリプトのSHA256
}
```

バージョンアップのトリガー:
- ユーザーからのフィードバック（「うまく動かなかった」）
- テストケースの追加・変更
- 依存関係の更新
- より良いアプローチの発見

旧バージョンは versions/ ディレクトリに保持し、ロールバック可能。

### 23.6 スキルの保存・管理・検索

```rust
struct SkillRegistry {
    skills_dir: PathBuf,
    registry: HashMap<String, SkillMeta>,
}

impl SkillRegistry {
    /// スキルの検索（テキスト + ベクトル類似度）
    async fn search(&self, query: &str, limit: usize) -> Vec<SkillMeta>;

    /// スキルの実行
    async fn execute(&self, name: &str, input: &str) -> Result<SkillOutput>;

    /// 新スキルの登録
    async fn register(&mut self, skill: NewSkill) -> Result<()>;

    /// スキルのバージョンアップ
    async fn upgrade(&mut self, name: &str, new_script: &str, reason: &str) -> Result<()>;

    /// 使用統計の更新
    async fn record_usage(&mut self, name: &str, success: bool) -> Result<()>;

    /// 低信頼度・未使用スキルのクリーンアップ
    async fn cleanup(&mut self, min_confidence: f32, unused_days: usize) -> Result<Vec<String>>;
}
```

### 23.7 安全性 — サンドボックス実行

自動生成されたスキルは信頼できないコードとして扱う:

```
┌─────────────────────────────────────┐
│ Sandbox Environment                  │
│                                      │
│ 制約:                                │
│ - ファイルアクセス: skills/ 配下のみ │
│ - ネットワーク: 明示的に許可された   │
│   ドメインのみ（スキル定義で指定）   │
│ - 実行時間: 最大60秒                 │
│ - メモリ: 最大512MB                  │
│ - プロセス生成: 不可                 │
│ - システムコール: 制限あり           │
│                                      │
│ 実装候補:                            │
│ - macOS sandbox-exec                 │
│ - Docker コンテナ（利用可能な場合）  │
│ - Wasm ランタイム（Wasmtime）        │
└─────────────────────────────────────┘
```

**信頼度による実行モード:**

| confidence | 実行モード |
|-----------|-----------|
| < 0.5 | サンドボックス + 結果をユーザーに確認 |
| 0.5 - 0.8 | サンドボックス実行、結果は自動採用 |
| > 0.8 | 通常実行（十分にテスト済み） |
| user作成 | 通常実行（ユーザー作成スキルは信頼） |

### 23.8 Claude Code連携

スキル開発時のコード生成にClaude Code Plugin (§8.3) を活用:

```
スキル開発リクエスト
    │
    ▼
ClaudeCodePlugin.execute_tool("claude_code", {
    task: "以下の要件でスキルを作成してください:
           - SKILL.md（定義ファイル）
           - 実装スクリプト
           - テストコード
           要件: {task_description}"
})
    │
    ▼
Claude Code が workspace/skills/{name}/ 以下に
ファイルを生成
    │
    ▼
テスト実行 → 結果評価 → 登録
```

### 23.9 設定

```toml
[skills]
enabled = true
skills_dir = "workspace/skills"
auto_develop = true            # 自動スキル開発を有効化
min_confidence_to_save = 0.5   # この信頼度以上のスキルのみ保存
sandbox_timeout_sec = 60
sandbox_memory_mb = 512
max_retries = 3                # 開発リトライ回数
cleanup_unused_days = 30       # この日数使われていないスキルをクリーンアップ候補に
```

---

## 24. 機能間連携 (v0.3 新規)

### 24.1 3機能の相互作用

§21（タグ付け）、§22（ストーリー記憶）、§23（スキル開発）は独立した機能だが、相互に連携して相乗効果を生む:

```
┌──────────────┐     タグで検索      ┌──────────────┐
│ §21 動的タグ │◄────────────────►│ §22 ストーリー │
│   付け       │                    │   記憶        │
│              │  ストーリーに       │              │
│              │  タグ付与           │              │
└──────┬───────┘                    └──────┬───────┘
       │                                   │
       │ スキルにタグ付与    ストーリーに   │
       │ スキル使用履歴を    スキル開発体験を│
       │ タグとして記録      エピソード保存  │
       │                                   │
       ▼                                   ▼
┌──────────────────────────────────────────────┐
│              §23 スキル自動開発                │
│                                               │
│ - タグでスキル検索                             │
│ - 過去のストーリーから類似タスクの経験を参照   │
│ - スキル開発自体がストーリーとして記録される   │
└──────────────────────────────────────────────┘
```

### 24.2 連携パターン詳細

**タグ付け → ストーリー記憶:**
- ストーリー記憶のYAML Front Matterにタグを自動付与
- バックグラウンドジョブで過去のストーリーにもリタグ
- 「あの時の気持ち」をタグで検索可能に

**ストーリー記憶 → タグ付け:**
- ストーリーのムード・トピックが新しいタグの候補になる
- エピソードの文脈がタグの確信度判定に活用される

**タグ付け → スキル開発:**
- スキルのtriggersがタグとして管理される
- 「このスキルが使われた記憶」にスキル名タグが付く
- 類似タグを持つスキル同士の関連性を発見

**スキル開発 → ストーリー記憶:**
- 新スキル開発の過程がストーリーとして自動記録
- スキルの成功/失敗体験がエピソード記憶に
- 次回のスキル開発時に過去の体験を参照

**ストーリー記憶 → スキル開発:**
- 過去に「手動で苦労した」ストーリーからスキル化候補を自動検出
- エピソードの文脈がスキルの要件定義に活用される

### 24.3 統合データフロー

```
会話・タスク実行
    │
    ├──→ メモリシステム (§18) に生データ保存
    │
    ├──→ バックグラウンドタグ付け (§21)
    │       │
    │       └──→ タグがストーリーにも伝播
    │
    ├──→ イベントドリブンストーリー生成 (§22)
    │       │
    │       └──→ ストーリーからタグ候補を抽出
    │
    └──→ スキルマッチング (§23)
            │
            ├──→ 既存スキル使用 → 使用ログがタグ・ストーリーに反映
            │
            └──→ 新スキル開発 → 開発過程がストーリーに、成果物にタグ付与
```

---


## 25. ホームディレクトリ制御 (v0.4 新規)

### 25.1 概要

エージェントがユーザーのホームディレクトリを汚染しないよう、指定したディレクトリをエージェントのホームとして使用する。ファイル操作のセキュリティ境界を明確にする。

### 25.2 ディレクトリ構成

```
指定ホームディレクトリ (例: /opt/slm-kairo/home/)
├── workspace/          # エージェントのワークスペース
│   ├── SOUL.core.toml
│   ├── SOUL.ext.toml
│   ├── MEMORY.md
│   └── memory/
├── data/               # DB, インデックス等
├── cache/              # 一時キャッシュ
├── skills/             # スキル
└── tmp/                # 一時ファイル
```

### 25.3 アクセス制御

```toml
[filesystem]
# エージェントのホームディレクトリ（ユーザーの ~ とは別）
home = "/opt/slm-kairo/home"

# ホーム以下は全操作OK（読み書き削除）
# ホーム外のアクセス制御方式
access_mode = "whitelist"   # "whitelist" | "blacklist"

# ホワイトリスト方式: 明示的に許可したパスのみアクセス可
[filesystem.whitelist]
read = [
    "/usr/share/dict",
    "/tmp/slm-kairo-*",
]
write = [
    "/tmp/slm-kairo-*",
]

# ブラックリスト方式: 指定パス以外は全てアクセス可
[filesystem.blacklist]
deny = [
    "/etc/shadow",
    "/etc/passwd",
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
]
```

### 25.4 実装

```rust
struct FilesystemGuard {
    home: PathBuf,
    access_mode: AccessMode,  // Whitelist | Blacklist
    whitelist_read: Vec<PathPattern>,
    whitelist_write: Vec<PathPattern>,
    blacklist_deny: Vec<PathPattern>,
}

impl FilesystemGuard {
    /// パスへのアクセスが許可されているか判定
    fn check_access(&self, path: &Path, op: FileOp) -> Result<()> {
        // ホーム以下は常にOK
        if path.starts_with(&self.home) {
            return Ok(());
        }
        // ホーム外はアクセスモードに応じて判定
        match self.access_mode {
            AccessMode::Whitelist => self.check_whitelist(path, op),
            AccessMode::Blacklist => self.check_blacklist(path, op),
        }
    }
}

enum FileOp { Read, Write, Delete, Execute }
```

ツール実行時に `FilesystemGuard` を通すことで、全てのファイル操作にアクセス制御を適用。

---



---

## 27. プラグインアーキテクチャ (v0.5 新規)

### 27.1 概要

**v0.5: 内部モジュールを含む全機能をプラグインとして再設計。** kairo-core は最小限のランタイム（イベントループ + プラグインローダー）のみとし、推論・セッション・メモリ・評価・Gateway・ツール・ストーリー記憶を全てプラグインとして実装する。

```
kairo-core (最小限のランタイム + プラグインローダー)
├── plugin: inference (Best-of-N推論)
├── plugin: session (セッション管理)
├── plugin: memory-store (記憶の保存)
├── plugin: memory-search (記憶の検索)
├── plugin: memory-tagging (動的タグ付け)
├── plugin: evaluator (評価関数)
├── plugin: gateway-discord
├── plugin: gateway-nostr
├── plugin: tools
└── plugin: story (エピソード記憶)
```

### 27.2 Plugin trait — 統一インターフェース

全プラグインが実装する統一トレイト。§8 の旧 Plugin trait を拡張し、ライフサイクル管理・依存関係宣言・自己記述機能を追加。

```rust
use async_trait::async_trait;
use serde_json::Value;

/// プラグインのメタデータ
#[derive(Debug, Clone)]
struct PluginMeta {
    /// プラグイン一意識別子（crate名と一致）
    id: String,
    /// 表示名
    name: String,
    /// バージョン (SemVer)
    version: String,
    /// 依存する他プラグインのID一覧
    dependencies: Vec<PluginDependency>,
    /// このプラグインが提供するサービス（他プラグインが依存可能）
    provides: Vec<String>,
    /// プラグインカテゴリ
    category: PluginCategory,
}

#[derive(Debug, Clone)]
struct PluginDependency {
    plugin_id: String,
    /// SemVer要件 (例: ">=0.2.0")
    version_req: String,
    /// オプショナル依存（なくても動作可能）
    optional: bool,
}

#[derive(Debug, Clone)]
enum PluginCategory {
    Core,       // inference, session, evaluator
    Memory,     // memory-store, memory-search, memory-tagging
    Gateway,    // gateway-discord, gateway-nostr
    Tool,       // tools, skills
    Story,      // story (エピソード記憶)
    Custom,     // ユーザー定義
}

/// 統一プラグインインターフェース
#[async_trait]
trait KairoPlugin: Send + Sync {
    // === メタデータ ===

    /// プラグインのメタデータを返す
    fn meta(&self) -> &PluginMeta;

    // === ライフサイクル ===

    /// ロード: 設定の読み込み、リソースの確保
    async fn load(&mut self, config: &Value) -> Result<()>;

    /// 初期化: 依存プラグインへの参照取得、DB接続等
    async fn init(&mut self, ctx: &mut PluginContext) -> Result<()>;

    /// 実行開始: イベントループへの参加、バックグラウンドタスク起動
    async fn start(&mut self, ctx: &PluginContext) -> Result<()>;

    /// 停止: リソース解放、状態保存
    async fn stop(&mut self) -> Result<()>;

    // === イベントハンドリング ===

    /// メッセージ受信時（Gatewayプラグインから伝播）
    async fn on_message(&self, _msg: &IncomingMessage, _ctx: &PluginContext) -> Result<()> {
        Ok(()) // デフォルト: 何もしない
    }

    /// 推論前フック
    async fn pre_inference(&self, _ctx: &mut InferenceContext) -> Result<()> {
        Ok(())
    }

    /// 推論後フック
    async fn post_inference(&self, _ctx: &mut InferenceContext, _response: &mut String) -> Result<()> {
        Ok(())
    }

    // === 自己記述 ===

    /// このプラグインの現在の設定を返す（管理画面・自己書き換え用）
    fn current_config(&self) -> Result<Value>;

    /// 設定の動的更新（自己書き換え対応）
    async fn update_config(&mut self, patch: &Value) -> Result<()>;

    /// ヘルスチェック
    async fn health(&self) -> Result<PluginHealth>;
}

#[derive(Debug)]
struct PluginHealth {
    status: HealthStatus,  // Healthy | Degraded | Unhealthy
    message: Option<String>,
    metrics: HashMap<String, f64>,  // プラグイン固有のメトリクス
}
```

### 27.3 プラグインのライフサイクル

```
  ┌─────────┐
  │ Discover │  プラグインcrateを検出・登録
  └────┬─────┘
       ▼
  ┌─────────┐
  │  Load    │  設定読み込み、基本バリデーション
  └────┬─────┘
       ▼
  ┌─────────┐
  │  Init    │  依存関係の解決、他プラグインへの参照取得
  └────┬─────┘  DB接続、埋め込みモデルロード等
       ▼
  ┌─────────┐
  │  Start   │  イベントループ参加、バックグラウンドタスク起動
  └────┬─────┘  Gateway接続、ハートビート開始等
       ▼
  ┌─────────┐
  │ Running  │  通常運用（イベント処理、フック実行）
  └────┬─────┘
       ▼ (シャットダウン or 再起動)
  ┌─────────┐
  │  Stop    │  リソース解放、状態永続化
  └─────────┘  Gateway切断、DB接続クローズ等
```

**起動順序の自動解決:**
依存関係グラフのトポロジカルソートにより、依存先プラグインが先に初期化される。循環依存はコンパイル時にエラー。

### 27.4 プラグイン間の依存関係管理

```rust
struct PluginLoader {
    /// 登録済みプラグイン
    plugins: HashMap<String, Box<dyn KairoPlugin>>,
    /// 依存関係グラフ
    dep_graph: DependencyGraph,
    /// プラグイン間通信バス
    event_bus: EventBus,
}

impl PluginLoader {
    /// 依存関係を解決し、起動順序を決定
    fn resolve_order(&self) -> Result<Vec<String>> {
        self.dep_graph.topological_sort()
    }

    /// 全プラグインを順序通りに起動
    async fn start_all(&mut self, config: &Config) -> Result<()> {
        let order = self.resolve_order()?;
        for plugin_id in &order {
            let plugin = self.plugins.get_mut(plugin_id).unwrap();
            plugin.load(&config.plugin_config(plugin_id)?).await?;
        }
        // Init（依存先が初期化済みの状態で実行）
        let mut ctx = PluginContext::new(&self.plugins, &self.event_bus);
        for plugin_id in &order {
            let plugin = self.plugins.get_mut(plugin_id).unwrap();
            plugin.init(&mut ctx).await?;
        }
        // Start
        for plugin_id in &order {
            let plugin = self.plugins.get_mut(plugin_id).unwrap();
            plugin.start(&ctx).await?;
        }
        Ok(())
    }
}

/// プラグイン間通信バス
struct EventBus {
    /// トピックベースのpub/sub
    subscribers: HashMap<String, Vec<mpsc::Sender<PluginEvent>>>,
}

/// プラグインコンテキスト — 他プラグインのサービスにアクセスするためのハンドル
struct PluginContext {
    /// 他プラグインのサービスへの型安全なアクセス
    services: HashMap<String, Arc<dyn Any + Send + Sync>>,
    /// イベントバスへの参照
    event_bus: Arc<EventBus>,
}
```

### 27.5 自己書き換え対応

エージェント自身がプラグインの設定変更、重み調整、実装差し替えを行える仕組み。

```rust
/// プラグイン自己書き換えAPI（ツールとして公開）
struct PluginSelfModifyTool {
    loader: Arc<RwLock<PluginLoader>>,
}

impl PluginSelfModifyTool {
    /// プラグインの設定を動的に変更
    /// 例: 評価関数の重みを調整、推論のtemperatureを変更
    async fn update_plugin_config(
        &self,
        plugin_id: &str,
        config_patch: Value,
    ) -> Result<()> {
        let mut loader = self.loader.write().await;
        let plugin = loader.plugins.get_mut(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;

        // バリデーション
        let old_config = plugin.current_config()?;
        plugin.update_config(&config_patch).await?;

        // ヘルスチェック — 変更後に問題があればロールバック
        match plugin.health().await {
            Ok(h) if h.status == HealthStatus::Unhealthy => {
                plugin.update_config(&old_config).await?;
                Err(anyhow!("Config change made plugin unhealthy, rolled back"))
            }
            _ => {
                // 変更を永続化（TOML設定ファイルに書き戻し）
                self.persist_config(plugin_id, &config_patch).await?;
                Ok(())
            }
        }
    }

    /// プラグインの有効/無効を切り替え
    async fn toggle_plugin(&self, plugin_id: &str, enabled: bool) -> Result<()>;

    /// プラグインの再起動（設定変更後等）
    async fn restart_plugin(&self, plugin_id: &str) -> Result<()>;
}
```

**自己書き換えの制約:**
- コアSOUL（`SOUL.core.toml`）の `constraints.never_do` に反する変更は拒否
- プラグインローダー自体の設定変更は管理画面からのみ（エージェントは不可）
- 全ての自己書き換えはLLMコールログ（§20.3）に記録
- ロールバック可能（変更前の設定を自動バックアップ）

### 27.6 crate構成（プラグイン単位に再構成）

```
slm-kairo/
├── Cargo.toml                          # ワークスペース定義
├── crates/
│   ├── kairo-core/                     # 最小限のランタイム + プラグインローダー
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs              # イベントループ
│   │       ├── plugin_loader.rs        # プラグインローダー
│   │       ├── plugin_trait.rs         # KairoPlugin trait定義
│   │       ├── event_bus.rs            # プラグイン間通信
│   │       ├── config.rs              # 設定管理
│   │       └── types.rs               # 共通型
│   │
│   ├── kairo-plugin-inference/         # Best-of-N推論
│   ├── kairo-plugin-session/           # セッション管理
│   ├── kairo-plugin-memory-store/      # 記憶の保存
│   ├── kairo-plugin-memory-search/     # 記憶の検索
│   ├── kairo-plugin-memory-tagging/    # 動的タグ付け
│   ├── kairo-plugin-evaluator/         # 評価関数
│   ├── kairo-plugin-gateway-discord/   # Discord Gateway
│   ├── kairo-plugin-gateway-nostr/     # Nostr Gateway
│   ├── kairo-plugin-tools/             # ツールコーリング
│   └── kairo-plugin-story/             # エピソード記憶
│
└── src/
    └── main.rs                         # エントリポイント（core + プラグイン登録）
```

**旧crate名との対応:**

| 旧 (v0.4) | 新 (v0.5) | 備考 |
|-----------|-----------|------|
| `slm-kairo-core` | `kairo-core` | ランタイム+ローダーのみに縮小 |
| `slm-kairo-inference` | `kairo-plugin-inference` | プラグイン化 |
| `slm-kairo-session` | `kairo-plugin-session` | プラグイン化 |
| `slm-kairo-memory` | `kairo-plugin-memory-store` + `memory-search` + `memory-tagging` | 3分割 |
| `slm-kairo-gateway` | `kairo-plugin-gateway-discord` + `gateway-nostr` | アダプタごとに分離 |
| `slm-kairo-tools` | `kairo-plugin-tools` | プラグイン化 |
| `slm-kairo-plugin` | `kairo-core` に統合 | ローダーがcoreに移動 |
| `slm-kairo-subagent` | `kairo-core` に統合 | ランタイム機能としてcoreに |
| (新規) | `kairo-plugin-story` | エピソード記憶を独立プラグインに |

### 27.7 設定例（TOML）

```toml
# kairo-core ランタイム設定
[runtime]
name = "slm-kairo"
plugin_dir = "plugins"        # 外部プラグインのディレクトリ（将来用）

# プラグインの有効/無効と設定
[plugins.inference]
enabled = true
crate = "kairo-plugin-inference"
# プラグイン固有設定
default_peer = "vllm-local"
best_of_n = 8
temperatures = [0.1, 0.3, 0.5, 0.7, 0.7, 0.9, 1.0, 1.2]

[plugins.session]
enabled = true
crate = "kairo-plugin-session"
max_context_tokens = 8192
debounce_min_ms = 800
debounce_max_ms = 5000

[plugins.memory-store]
enabled = true
crate = "kairo-plugin-memory-store"
db_path = "data/memory.sqlite"
embedding_model = "ruri-v3-30m"
chunk_size = 400
chunk_overlap = 80

[plugins.memory-search]
enabled = true
crate = "kairo-plugin-memory-search"
fts_weight = 0.3
vector_weight = 0.7
depends_on = ["memory-store"]   # 依存関係の明示

[plugins.memory-tagging]
enabled = true
crate = "kairo-plugin-memory-tagging"
schedule = "daily"
batch_size = 50
depends_on = ["memory-store", "memory-search"]

[plugins.evaluator]
enabled = true
crate = "kairo-plugin-evaluator"
weight_llm = 0.30
weight_length = 0.10
weight_consistency = 0.10
weight_persona = 0.25
weight_rule_compliance = 0.25

[plugins.gateway-discord]
enabled = true
crate = "kairo-plugin-gateway-discord"
prefix = "!"
extensions = ["reaction", "typing", "thread_reply"]

[plugins.gateway-nostr]
enabled = false
crate = "kairo-plugin-gateway-nostr"
relays = ["wss://relay.damus.io", "wss://nos.lol"]
signer = "env"

[plugins.tools]
enabled = true
crate = "kairo-plugin-tools"
max_chain_depth = 5

[plugins.story]
enabled = true
crate = "kairo-plugin-story"
daily_digest_time = "23:00"
use_persona = true
depends_on = ["memory-store", "session"]
```

### 27.8 スキル自動開発との連携 (§23)

エージェントが自分自身のプラグインを改善できる仕組み。§23 のスキル自動開発フレームワークを拡張し、プラグインレベルの自己改善を実現する。

```
┌──────────────────────────────────────────────────────┐
│ プラグイン自己改善フロー                               │
│                                                       │
│ 1. 問題検出                                           │
│    - 評価スコアの継続的低下を検出                      │
│    - ユーザーからの否定的フィードバック                │
│    - プラグインのヘルスチェック異常                    │
│         ↓                                             │
│ 2. 原因分析（LLM推論）                                │
│    - LLMコールログ（§20.3）を分析                     │
│    - どのプラグインの設定が問題か特定                  │
│         ↓                                             │
│ 3. 改善案の生成                                       │
│    - 設定パラメータの調整案                            │
│    - 重みの最適化案                                    │
│    - (高度) Claude Code によるプラグインコード修正案   │
│         ↓                                             │
│ 4. A/Bテスト                                          │
│    - 改善案を一時的に適用                              │
│    - 一定期間の評価スコアを比較                        │
│    - 改善が確認されたら永続化                          │
│         ↓                                             │
│ 5. 適用 or ロールバック                                │
│    - update_config() で適用                            │
│    - 悪化した場合はロールバック                        │
└──────────────────────────────────────────────────────┘
```

**改善可能な項目の例:**
- evaluator: 評価関数の重み（w1-w5）の自動最適化
- inference: temperature分布の調整、Best-of-N の N 値
- memory-search: FTS/ベクトル検索の重み比率
- memory-tagging: タグ付け確信度の閾値、バッチサイズ
- session: デバウンス時間のパラメータ
- story: ストーリー生成の最大文字数、重要度閾値

**安全制約:**
- コアランタイム（kairo-core）のコードは自己修正不可
- プラグインのコード修正はサンドボックス（§23.7）内でテスト必須
- 設定変更は1回あたり1パラメータまで（複数同時変更による問題の切り分け困難を回避）
- 全変更はバージョン管理され、任意の時点にロールバック可能

---

## 28. 今後の拡張ポイント

- **マルチモデル対応**: ツール判定と本文生成で異なるモデルを使用
- **メトリクス**: Prometheus互換のメトリクスエンドポイント
- **A/Bテスト**: 評価関数の重み最適化のためのログ収集
- **評価モデルの自動選定**: 分散計測の自動化とモデル切り替え
- **Nostr NIP-90 対応**: Data Vending Machine としてのサービス提供
- **マルチワークスペース**: 用途別にワークスペースを切り替え

---

## 29. Self-Reinforcement Fine-Tuning（自己強化学習）

### 29.1 コンセプト

エージェントが自分の過去の会話を振り返り、「良かった」と判断した応答をFTデータとして自動選別し、自分自身の重みに書き込む仕組み。

### 29.2 超自我としてのFT

SOUL（システムプロンプト）が意識・自我であるならば、FTによって重みに焼き込まれたパターンは**無意識・超自我**に相当する。

```
┌─────────────────────────────────────────────────┐
│ エージェントの精神構造                            │
│                                                  │
│  ┌────────────────────────────┐                  │
│  │ 意識層: SOUL.toml          │ ← 明示的ルール   │
│  │  システムプロンプト         │   自我・意識     │
│  │  行動規範・ペルソナ定義     │                  │
│  └────────────┬───────────────┘                  │
│               │                                  │
│  ┌────────────▼───────────────┐                  │
│  │ 無意識層: FT重み            │ ← 経験の蓄積    │
│  │  過去の良い応答パターン     │   超自我・無意識 │
│  │  暗黙的な判断基準           │                  │
│  │  言い回し・トーンの癖       │                  │
│  └────────────────────────────┘                  │
│                                                  │
│  SOULは書き換え可能（意識的変更）                 │
│  FT重みは経験から徐々に形成（無意識的成長）       │
└─────────────────────────────────────────────────┘
```

これにより、エージェントは**経験から人格が成長する**。SOULで明示的に定義されていない微妙なニュアンス——ユーモアのセンス、共感の深さ、説明の巧みさ——がFTを通じて内面化される。

### 29.3 自己評価ループ

会話完了後、エージェント自身が応答を振り返り、FTデータとして適切かを判定する。

```
会話完了
    │
    ▼
┌──────────────────────────────┐
│ 1. 応答品質の自己評価         │
│    - ユーザーの反応分析       │
│      (感謝、追加質問、離脱等) │
│    - SOUL準拠度の自己採点     │
│    - 情報の正確性チェック     │
│    - 会話の自然さ評価         │
│                              │
│ 出力: スコア (0.0 - 1.0)     │
└──────────┬───────────────────┘
           │
     ┌─────┴─────┐
     │           │
  score≥0.8   score<0.8
     │           │
     ▼           ▼
┌──────────┐  破棄
│ 2. FTデータ│
│    候補登録│
│    (staging)│
└──────────┘
```

**評価基準:**
- **ユーザー反応**: 感謝・肯定的反応があったか、会話が自然に継続したか
- **SOUL準拠度**: ペルソナ設定に沿った応答だったか
- **情報品質**: 正確で有用な情報を提供できたか
- **対話品質**: 自然で心地よい会話だったか

### 29.4 FTパイプライン連携（kairo ft pipeline）

蓄積されたFTデータ候補を定期的にファインチューニングに回す。

```
┌─────────────────────────────────────────────────────┐
│ Self-Reinforcement FT Pipeline                       │
│                                                      │
│  ┌──────────┐    ┌──────────┐    ┌──────────────┐  │
│  │ Staging   │───▶│ Review   │───▶│ FT Dataset   │  │
│  │ Buffer    │    │ Gate     │    │ Builder      │  │
│  │           │    │          │    │              │  │
│  │ 自動選別  │    │ 人間/AI  │    │ フォーマット │  │
│  │ された候補│    │ レビュー │    │ 変換・検証   │  │
│  └──────────┘    └──────────┘    └──────┬───────┘  │
│                                          │          │
│                                          ▼          │
│                                   ┌──────────────┐  │
│                                   │ FT Executor  │  │
│                                   │              │  │
│                                   │ LoRA/QLoRA   │  │
│                                   │ 増分学習     │  │
│                                   └──────┬───────┘  │
│                                          │          │
│                                          ▼          │
│                                   ┌──────────────┐  │
│                                   │ Validation   │  │
│                                   │              │  │
│                                   │ ベンチマーク │  │
│                                   │ 回帰テスト   │  │
│                                   │ SOUL準拠確認 │  │
│                                   └──────────────┘  │
└─────────────────────────────────────────────────────┘
```

**パイプラインの各ステージ:**

1. **Staging Buffer**: 自己評価でスコア閾値を超えた会話ペアを蓄積（SQLite）
2. **Review Gate**: 人間またはAIレビュアーによる品質チェック（段階に応じて自動化度を変更）
3. **FT Dataset Builder**: SFT形式（instruction/input/output）へのフォーマット変換、重複排除、バランス調整
4. **FT Executor**: LoRA/QLoRAベースの増分ファインチューニング実行
5. **Validation**: FT後のモデルがベンチマークを満たすか検証、SOUL準拠度の回帰テスト

### 29.5 安全性：自己強化の暴走防止

自己評価→自己学習のループは、評価基準のドリフトにより暴走するリスクがある。以下の安全機構を設ける。

**評価基準ドリフト対策:**
- **固定ベンチマークセット**: FT前後で必ず通すテストケース（ゴールデンデータセット）を維持
- **SOUL準拠度の回帰テスト**: FT後にSOUL.core.tomlの全ルールへの準拠を自動テスト
- **分布モニタリング**: FTデータの分布（トピック、トーン、長さ）が偏っていないか監視

**人間によるレビューゲート:**
- v1/v2では全FTデータに人間レビューを必須とする
- v3でも一定割合（例: 10%）のサンプリングレビューを維持
- レビューダッシュボード（§20 管理画面に統合）で効率的に確認

**ロールバック機構:**
- FT前のモデル重みを常にバックアップ
- Validation失敗時は自動ロールバック
- 人間が任意の時点の重みに戻せる

**量的制限:**
- 1回のFTで使用するデータ量の上限（例: 500件）
- FT頻度の制限（例: 週1回まで）
- LoRAランクの制限によるモデル変更度合いの抑制

### 29.6 段階的導入

| フェーズ | 自動化レベル | 説明 |
|---------|-------------|------|
| **v1: 手動選別** | 低 | 人間が会話ログを見てFTデータを手動選別。パイプラインの基盤構築に集中 |
| **v2: 自動スコアリング** | 中 | 自己評価ループによる自動スコアリング。閾値超えを候補としてリスト化。人間が最終承認 |
| **v3: 完全自律** | 高 | 自己評価→選別→FT実行まで自律。人間はサンプリングレビューと異常時介入のみ |

**v1の具体的タスク:**
- FTデータフォーマットの確定
- Staging Buffer（SQLite）の実装
- 手動レビューUI（管理画面 §20 統合）
- LoRA FTスクリプトの整備

**v2の具体的タスク:**
- 自己評価モデルの構築（軽量SLMで評価スコアを出力）
- ユーザー反応の自動分析（感謝検出、離脱検出等）
- 自動候補リスト生成 + 人間承認フロー

**v3の具体的タスク:**
- Review Gateの自動化（AI レビュアー）
- FT実行の自動スケジューリング
- 異常検知と自動ロールバック
- サンプリングレビューのダッシュボード
