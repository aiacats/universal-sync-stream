# USSP - Universal Sync Stream Protocol

## プロジェクト概要

単一の映像信号と複数のマルチトラックオーディオをUDPで同期送信するためのRustライブラリ。

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

## コーディング規約

- Rust 2021 editionを使用
- `cargo fmt`でフォーマット
- `cargo clippy`で静的解析
- 全てのpublic APIにドキュメントコメントを付与
- エラー処理には`thiserror`を使用
- 非同期処理には`tokio`を使用

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
