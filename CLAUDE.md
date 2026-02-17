# USSP - Universal Sync Stream Protocol

## プロジェクト概要

複数の信号の同期をとった状態でインターネット越しに遠隔地へ送るプロトコル。
単一の映像信号と複数のマルチトラックオーディオをUDPで同期送信するためのRustライブラリ。

---

## プロジェクト固有の原則

### Rustプロジェクト

- Rust 2021 editionを使用
- `cargo fmt`でフォーマット
- `cargo clippy`で静的解析
- 全てのpublic APIにドキュメントコメントを付与
- エラー処理には`thiserror`を使用
- 非同期処理には`tokio`を使用

---

## ビルド方法

```bash
cargo build --release
```

## テスト実行

```bash
cargo test
```

## サンプル実行

```bash
# 送信側
cargo run --example sender -- --addr 0.0.0.0:5000 --dest 192.168.1.100:5001

# 受信側
cargo run --example receiver -- --addr 0.0.0.0:5001
```

---

## プロトコル仕様

### パケット構造

全てのパケットは24バイトの共通ヘッダーを持つ:

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | Magic ("USSP") |
| 4 | 1 | Version |
| 5 | 1 | Packet Type |
| 6 | 2 | Flags |
| 8 | 4 | Session ID |
| 12 | 4 | Sequence Number |
| 16 | 8 | Timestamp (microseconds) |

### パケットタイプ

- `0x01`: VIDEO_FRAME - 映像フレーム
- `0x02`: AUDIO_FRAME - オーディオフレーム
- `0x03`: SYNC_POINT - 同期ポイント
- `0x04`: FEC_REPAIR - FEC修復パケット
- `0x05`: SESSION_INIT - セッション初期化
- `0x06`: SESSION_ACK - セッション確認
- `0x07`: HEARTBEAT - 接続維持

### FEC設定

Reed-Solomon符号を使用:
- デフォルト: k=10 (データ), m=3 (パリティ)
- 設定により調整可能

---

## ディレクトリ構造

```
src/
├── lib.rs           # ライブラリエントリ
├── error.rs         # エラー型定義
├── protocol/        # プロトコル定義
│   ├── mod.rs
│   ├── types.rs     # 共通型
│   ├── header.rs    # パケットヘッダー
│   ├── video.rs     # 映像ペイロード
│   ├── audio.rs     # オーディオペイロード
│   └── sync.rs      # 同期ポイント
├── fec/             # 前方誤り訂正
│   ├── mod.rs
│   ├── encoder.rs   # エンコーダー
│   └── decoder.rs   # デコーダー
├── transport/       # トランスポート層
│   ├── mod.rs
│   ├── session.rs   # セッション管理
│   ├── sender.rs    # 送信者
│   └── receiver.rs  # 受信者
└── sync/            # 同期機構
    ├── mod.rs
    ├── clock.rs     # クロック同期
    └── buffer.rs    # ジッターバッファ
```

---

## #．Unity プロジェクトを扱う場合の原則

- Unity上の問題などにより、解決に時間を要した課題について、原因・対策を同ディレクトリに存在する「CLAUDE-UnityCaution.md」に書き溜めていく。また、エラー・課題に当たった際にそのファイルを参照して解決案を模索するようにする。
- 実装時、可能な限り Monobehaviour を最小限に抑える。
- 実装時、名前やタグで検索する実装は可能な限り行わない。
- 実装完了後は必ず Unity Editor の強制リロードを実行する。（方法は下記参照）
    - これを行う際、UnityEditor が開いているか否かは確認しなくて良い。
    - 強制リロード後、UnityEditor 内 Console のログを読み込み、エラーが無いか確認する。（方法は下記参照）
    - エラーが発生した場合には、先程取得したログの内容をもとに修正を行う。
- Debug.Log などで敷いていたデバッグメッセージの確認が必要な場合にも、MCP サーバー経由で Unity エディタの情報を取得する。（方法は下記参照）

### MCP サーバー経由での Unity 接続方法（動作確認済み）

以下の方法で Unity エディタとの連携が確実に動作することを確認済み：

#### 1. ホットリロード実行方法

```bash
# 方法1: MCP API直接実行（推奨）
curl http://localhost:8090/mcp/tools/hot_reload

```

#### 2. Unity コンソールログ取得方法

```bash
# 現在のコンソールログを取得
curl http://localhost:8090/mcp/tools/get_console_logs

# テストメッセージを Unity コンソールに送信
curl http://localhost:8090/mcp/tools/send_console_log -X POST -H "Content-Type: application/json" -d '{"message":"Test message from MCP"}'
```

#### 3. 接続状態確認方法

```bash
# MCP サーバーの状態確認
curl http://localhost:8090/health

# Unity エディタプロセス確認
curl http://localhost:8090/mcp/tools/get_unity_process_info
```

#### 4. トラブルシューティング

- **ポート 8090 が使用できない場合**: `.mcp.json` でポート番号を変更
- **Unity エディタが応答しない場合**: Unity エディタを再起動してから MCP サーバーを再起動
- **WebSocket 接続エラーの場合**: Windows ファイアウォール設定を確認

#### 5. Task ツールからの MCP サーバー利用

- Claude Code の Task ツール（Agent）から MCP サーバーを利用する場合：
    - `subagent_type: "general-purpose"` を使用
    - プロンプトで「Unity MCP server を使用してコンソールログを取得」等と指示
    - Agent が自動的に適切な MCP API を呼び出して Unity エディタと連携

## #．c++プロジェクトを扱う場合の原則

- 実装を終えたら、ビルドを行う。その後、エラーが吐いていないかを確認し、エラーがある場合には修正を行う。
  　　 エラーが完全に無くなるまで修正を行う。
- ソースコードは基本的に「src」フォルダ内に作成する。外部的に公開した方が有用と思われるもののみ、「include」フォルダ内にhファイルを作成する。
