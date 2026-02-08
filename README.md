# USSP - Universal Sync Stream Protocol

USSPは、単一の映像信号と複数のマルチトラックオーディオをUDP上で同期して送信するためのRustライブラリです。

## 特徴

- **映像・音声同期**: マイクロ秒精度のPTS（Presentation Timestamp）による正確な同期
- **マルチトラックオーディオ**: 無制限のオーディオトラックをサポート
- **DMX/Art-Net対応**: 照明制御データも映像・音声と同期して送信可能
- **FEC (Forward Error Correction)**: Reed-Solomonコードによるパケットロス耐性
- **フラグメンテーション**: 大きな映像フレームの自動分割・再構成
- **低遅延設計**: リアルタイムストリーミング向けに最適化
- **SFUサーバー**: インターネット配信向けのスケーラブルなサーバーアーキテクチャ
- **冗長化対応**: Primary/Backupセンダーによる自動フェイルオーバー

## プロトコル仕様

### パケット構造

すべてのUSSPパケットは24バイトの共通ヘッダで始まります:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      'U'      |      'S'      |      'S'      |      'P'      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |  Packet Type  |            Flags              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          Session ID                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Sequence Number                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                    Timestamp (64-bit, μs)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### パケットタイプ

| 値   | タイプ        | 説明                         |
|------|---------------|------------------------------|
| 0x01 | VideoFrame    | 映像フレームデータ           |
| 0x02 | AudioFrame    | 音声フレームデータ           |
| 0x03 | SyncPoint     | 同期ポイント                 |
| 0x04 | FecRepair     | FEC修復パケット              |
| 0x05 | SessionInit   | セッション初期化             |
| 0x06 | SessionAck    | セッション確認応答           |
| 0x07 | Heartbeat     | キープアライブ               |
| 0x08 | DmxFrame      | DMXデータ（Art-Net互換）     |

### フラグ

| ビット | フラグ          | 説明                         |
|--------|-----------------|------------------------------|
| 0x0001 | KEY_FRAME       | キーフレーム（映像用）       |
| 0x0002 | END_OF_STREAM   | ストリーム終了               |
| 0x0004 | REQUIRES_ACK    | 確認応答が必要               |
| 0x0008 | RETRANSMISSION  | 再送パケット                 |
| 0x0010 | FEC_PROTECTED   | FEC保護されている            |

### 映像ペイロード (32バイトヘッダ + データ)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Frame ID                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|        Fragment Index         |       Total Fragments         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Codec     |  Key Frame    |           Reserved            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                           PTS (μs)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                           DTS (μs)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          Data Length                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Frame Data...                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**対応映像コーデック**: Raw, H.264, H.265, VP8, VP9, AV1

### 音声ペイロード (24バイトヘッダ + データ)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Track ID             |         Sample Rate...        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      ...Sample Rate           |   Channels    | Bits/Sample   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Codec     |   Reserved    |           Reserved            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                           PTS (μs)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Sample Count                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          Data Length                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Audio Data...                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**対応音声コーデック**: PCM, AAC, Opus, MP3, FLAC

### DMXペイロード (16バイトヘッダ + データ)

Art-Net互換のDMX512照明制御データを送信します。

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           Universe            |      Net      |    Subnet     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  DMX Sequence |   Reserved    |         Channel Count         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                           PTS (μs)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      DMX Channel Data...                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Universe**: Art-Net 15ビットポートアドレス (Net:7bit + SubNet:4bit + Universe:4bit)
- **Channel Count**: 1-512チャンネル
- **DMX Channel Data**: 各チャンネル0-255の値

## FEC (Forward Error Correction)

USSPはReed-Solomonコードを使用したFEC機能を提供します。

- **デフォルト設定**: 10データシャード + 3パリティシャード
- **パケットロス耐性**: 最大3パケットまでのロスを復元可能
- **設定可能**: アプリケーションの要件に応じてパラメータ調整可能

## 使用方法

### ライブラリとして使用

#### 依存関係の追加

```toml
[dependencies]
ussp = { path = "path/to/ussp" }
tokio = { version = "1", features = ["full"] }
```

#### 送信側

```rust
use ussp::transport::{Sender, SenderConfig};
use ussp::protocol::{VideoCodecType, AudioCodecType};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 送信設定
    let config = SenderConfig {
        fec_enabled: true,
        ..Default::default()
    };

    // 送信者を作成
    let sender = Sender::bind("0.0.0.0:5000", "192.168.1.100:5001", config).await?;

    // セッション初期化
    sender.init_session().await?;

    // 映像フレーム送信
    let video_data = Bytes::from(vec![0u8; 1920 * 1080 * 3]);
    sender.send_video_frame(
        VideoCodecType::H264,
        true,  // キーフレーム
        0,     // PTS
        0,     // DTS
        video_data,
    ).await?;

    // 音声フレーム送信（トラック0）
    let audio_data = Bytes::from(vec![0u8; 960 * 2 * 2]);
    sender.send_audio_frame(
        0,      // トラックID
        48000,  // サンプルレート
        2,      // チャンネル数
        16,     // ビット深度
        AudioCodecType::Opus,
        0,      // PTS
        960,    // サンプル数
        audio_data,
    ).await?;

    Ok(())
}
```

#### 受信側

```rust
use ussp::transport::{Receiver, ReceiverConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ReceiverConfig::default();
    let receiver = Receiver::bind("0.0.0.0:5001", config).await?;

    loop {
        match receiver.receive().await? {
            ReceivedData::Video(frame) => {
                println!("Video frame: {} bytes, PTS: {}μs",
                    frame.data.len(), frame.pts);
            }
            ReceivedData::Audio(frame) => {
                println!("Audio track {}: {} samples, PTS: {}μs",
                    frame.track_id, frame.sample_count, frame.pts);
            }
            ReceivedData::Sync(sync) => {
                println!("Sync point: video={}, audio={:?}",
                    sync.video_pts, sync.audio_pts);
            }
        }
    }
}
```

## USSP Tester (GUIアプリケーション)

テスト用GUIアプリケーション（Tauri 2 + React）が同梱されています。

### ビルド方法

```bash
cd apps/ussp-tester

# 依存関係インストール
npm install

# 開発モード
npm run tauri dev

# リリースビルド
npm run tauri build
```

### 機能

#### 送信モード
- 送信先IPアドレス・ポート設定
- テストパターン生成（カラーバー）
- オーディオトラック数設定（1/2/4/8）
- FPS設定（24/25/30/50/60）
- FEC有効/無効切り替え
- リアルタイム統計表示
  - 送信パケット数
  - 送信バイト数
  - 映像/音声フレーム数
  - 現在のFPS

#### 受信モード
- 受信ポート設定
- 受信映像プレビュー（Canvas描画）
- オーディオレベルメーター
- リアルタイム統計表示
  - 受信パケット数
  - 受信バイト数
  - 映像/音声フレーム数
  - パケットロス率
  - A/V同期差（μs）

### テスト手順

1. アプリケーションを2つのインスタンスで起動
2. 一方を「Sender」モードに設定
3. もう一方を「Receiver」モードに設定
4. 送信側で宛先アドレスとポートを設定
5. 受信側で受信ポートを設定
6. 送信開始 → 受信側で映像・統計を確認

同一マシンでのテスト:
- 送信先: `127.0.0.1:5001`
- 受信ポート: `5001`

## USSP Admin (SFU管理GUIアプリケーション)

SFUサーバーを管理するためのデスクトップGUIアプリケーション（Tauri 2 + React）です。

### ビルド方法

```bash
cd apps/ussp-admin

# 依存関係インストール
npm install

# 開発モード
npm run tauri dev

# リリースビルド
npm run tauri build
```

### 機能

#### ダッシュボード
- Control Planeのヘルス状態表示
- グローバル統計（サーバー数、ルーム数、参加者数、転送量）
- 5秒間隔の自動リフレッシュ

#### ルーム管理
- ルーム一覧表示（状態、参加者数、割り当てサーバー）
- 新規ルーム作成（カスタムID または 自動生成）
- ルーム削除
- ルーム詳細表示:
  - 統計情報（パケット数、転送バイト数、稼働時間）
  - Primary/Backup Sender状態
  - 参加者一覧（ID、ロール、接続先アドレス、セッションID）
  - 参加者の強制退出

#### サーバー管理
- SFUサーバー一覧表示
- サーバーヘルス状態（Healthy/Unhealthy）
- ロード状況のビジュアル表示
- ルーム数・参加者数の確認

#### 接続設定
- Control Plane URL設定
- API Key認証
- 接続状態表示

### 使用方法

1. アプリケーションを起動
2. 「Settings」タブでControl Plane URLとAPI Keyを入力
3. 「Connect」をクリックして接続
4. 「Dashboard」「Rooms」「Servers」タブで管理

### スクリーンショット概要

```
┌─────────────────────────────────────────────────────────────┐
│  USSP Admin   [Dashboard] [Rooms] [Servers] [Settings]  ●  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Control Plane Status                          [Healthy]    │
│  Version: 0.1.0                                             │
│                                                             │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │    3    │ │   12    │ │   45    │ │ 1.2 GB  │           │
│  │ Servers │ │  Rooms  │ │ Users   │ │Forwarded│           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 注意事項

- 管理対象のSFUサーバー（ussp-server）が起動している必要があります
- admin ロールのAPI Keyが必要です
- 開発モード（`--no-auth`）で起動されたサーバーには任意のAPI Keyで接続可能

## USSP SFUサーバー

インターネット配信向けのSFU（Selective Forwarding Unit）サーバーを提供します。

### アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                      Control Plane                           │
│  (REST API: ルーム管理、参加者管理、認証、統計)             │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   SFU Server 1  │  │   SFU Server 2  │  │   SFU Server N  │
│  ┌───────────┐  │  │  ┌───────────┐  │  │  ┌───────────┐  │
│  │  Room A   │  │  │  │  Room C   │  │  │  │  Room E   │  │
│  │  Room B   │  │  │  │  Room D   │  │  │  │  Room F   │  │
│  └───────────┘  │  │  └───────────┘  │  │  └───────────┘  │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         ▲                    ▲                    ▲
         │                    │                    │
   ┌─────┴─────┐        ┌─────┴─────┐        ┌─────┴─────┐
   │  Sender   │        │  Sender   │        │  Sender   │
   │ (Primary) │        │ (Backup)  │        │(Receivers)│
   └───────────┘        └───────────┘        └───────────┘
```

### 特徴

- **ルーム管理**: セッション単位でのルーム作成・管理
- **ロールベース**: Primary Sender / Backup Sender / Receiver
- **自動フェイルオーバー**: Primary障害時にBackupへ自動切り替え
- **ロードバランシング**: 複数サーバー間での負荷分散
- **REST API**: 完全なHTTP APIによる管理
- **JWT認証**: ロールベースのアクセス制御

### サーバーの起動

```bash
# ビルド
cargo build -p ussp-server --release

# 開発モード（認証なし）
cargo run -p ussp-server -- --no-auth

# 本番モード
cargo run -p ussp-server -- \
  --server-id sfu-1 \
  --media-addr 0.0.0.0:5000 \
  --control-addr 0.0.0.0:8080 \
  --jwt-secret "your-secure-secret" \
  --failover

# 環境変数での設定も可能
USSP_SERVER_ID=sfu-1 \
USSP_MEDIA_ADDR=0.0.0.0:5000 \
USSP_CONTROL_ADDR=0.0.0.0:8080 \
USSP_JWT_SECRET="your-secure-secret" \
USSP_FAILOVER=true \
cargo run -p ussp-server
```

### CLI オプション

| オプション | 環境変数 | デフォルト | 説明 |
|-----------|----------|-----------|------|
| `--server-id` | `USSP_SERVER_ID` | `sfu-1` | サーバー識別子 |
| `--media-addr` | `USSP_MEDIA_ADDR` | `0.0.0.0:5000` | メディア用UDPアドレス |
| `--control-addr` | `USSP_CONTROL_ADDR` | `0.0.0.0:8080` | 制御API HTTPアドレス |
| `--jwt-secret` | `USSP_JWT_SECRET` | - | JWT署名用シークレット |
| `--no-auth` | `USSP_NO_AUTH` | `false` | 認証を無効化（開発用） |
| `--max-rooms` | `USSP_MAX_ROOMS` | `100` | 最大ルーム数 |
| `--max-participants` | `USSP_MAX_PARTICIPANTS` | `100` | ルームあたり最大参加者数 |
| `--failover` | `USSP_FAILOVER` | `false` | 自動フェイルオーバーを有効化 |
| `--log-level` | `RUST_LOG` | `info` | ログレベル |

### REST API

#### 認証

```bash
# トークン取得
curl -X POST http://localhost:8080/auth/token \
  -H "Content-Type: application/json" \
  -d '{"api_key": "my-api-key", "role": "admin"}'

# レスポンス
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 3600
}
```

#### ルーム管理

```bash
# ヘルスチェック
curl http://localhost:8080/health

# ルーム一覧取得
curl http://localhost:8080/api/v1/rooms \
  -H "Authorization: Bearer <token>"

# ルーム作成
curl -X POST http://localhost:8080/api/v1/rooms \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"room_id": "my-room"}'

# ルーム詳細取得
curl http://localhost:8080/api/v1/rooms/my-room \
  -H "Authorization: Bearer <token>"

# ルーム統計取得
curl http://localhost:8080/api/v1/rooms/my-room/stats \
  -H "Authorization: Bearer <token>"

# ルーム削除
curl -X DELETE http://localhost:8080/api/v1/rooms/my-room \
  -H "Authorization: Bearer <token>"
```

#### 参加者管理

```bash
# 参加者一覧
curl http://localhost:8080/api/v1/rooms/my-room/participants \
  -H "Authorization: Bearer <token>"

# 参加者追加（Primary Sender）
curl -X POST http://localhost:8080/api/v1/rooms/my-room/participants \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "participant_id": "sender-1",
    "addr": "192.168.1.100:5001",
    "role": "primary_sender",
    "session_id": 12345
  }'

# 参加者追加（Receiver）
curl -X POST http://localhost:8080/api/v1/rooms/my-room/participants \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "participant_id": "receiver-1",
    "addr": "192.168.1.101:5002",
    "role": "receiver",
    "session_id": 12345
  }'

# 参加者削除
curl -X DELETE http://localhost:8080/api/v1/rooms/my-room/participants/sender-1 \
  -H "Authorization: Bearer <token>"
```

#### サーバー管理（管理者のみ）

```bash
# サーバー一覧
curl http://localhost:8080/api/v1/servers \
  -H "Authorization: Bearer <admin-token>"

# サーバー統計
curl http://localhost:8080/api/v1/servers/sfu-1/stats \
  -H "Authorization: Bearer <admin-token>"

# グローバル統計
curl http://localhost:8080/api/v1/stats \
  -H "Authorization: Bearer <admin-token>"
```

### ユーザーロール

| ロール | 権限 |
|--------|------|
| `admin` | 全操作可能（サーバー管理含む） |
| `operator` | ルーム・参加者の管理可能 |
| `sender` | 指定ルームへの送信可能 |
| `receiver` | 指定ルームからの受信可能 |

### フェイルオーバー

Primary Senderが切断された場合の動作:

1. ルーム状態が `Failover` に遷移
2. 設定された遅延（デフォルト500ms）後にBackup Senderを昇格
3. Backup SenderがPrimary Senderとしてストリーミング継続
4. ルーム状態が `Active` に復帰

Backup Senderがない場合はルームが `Paused` 状態になります。

## プロジェクト構造

```
ussp/                               # ワークスペースルート
├── Cargo.toml                      # ワークスペース定義
├── LICENSE
├── README.md
├── crates/
│   ├── ussp/                       # コアライブラリ
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs              # ライブラリエントリ
│   │   │   ├── error.rs            # エラー定義
│   │   │   ├── protocol/           # プロトコル定義
│   │   │   │   ├── header.rs       # パケットヘッダ
│   │   │   │   ├── video.rs        # 映像ペイロード
│   │   │   │   ├── audio.rs        # 音声ペイロード
│   │   │   │   ├── dmx.rs          # DMXペイロード
│   │   │   │   ├── sync.rs         # 同期ポイント
│   │   │   │   └── types.rs        # 共通型
│   │   │   ├── transport/          # 送受信実装
│   │   │   │   ├── sender.rs       # 送信者
│   │   │   │   ├── receiver.rs     # 受信者
│   │   │   │   └── session.rs      # セッション管理
│   │   │   ├── fec/                # 前方誤り訂正
│   │   │   │   ├── encoder.rs      # FECエンコーダ
│   │   │   │   └── decoder.rs      # FECデコーダ
│   │   │   └── sync/               # 同期機構
│   │   │       ├── clock.rs        # クロック同期
│   │   │       └── buffer.rs       # ジッタバッファ
│   │   ├── examples/
│   │   │   ├── sender.rs           # 送信サンプル
│   │   │   └── receiver.rs         # 受信サンプル
│   │   └── tests/
│   │       └── integration.rs      # 統合テスト
│   │
│   ├── ussp-sfu/                   # SFUサーバーライブラリ
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # ライブラリエントリ
│   │       ├── server.rs           # SFUサーバー実装
│   │       ├── room.rs             # ルーム管理
│   │       ├── participant.rs      # 参加者管理
│   │       ├── forwarder.rs        # パケット転送
│   │       ├── config.rs           # 設定
│   │       ├── stats.rs            # 統計
│   │       └── error.rs            # エラー定義
│   │
│   └── ussp-control/               # コントロールプレーン
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs              # ライブラリエントリ
│           ├── api.rs              # REST API
│           ├── auth.rs             # JWT認証
│           ├── state.rs            # アプリ状態
│           └── error.rs            # エラー定義
│
└── apps/
    ├── ussp-server/                # SFUサーバーバイナリ
    │   ├── Cargo.toml
    │   └── src/
    │       └── main.rs             # エントリポイント
    │
    ├── ussp-tester/                # GUIテスター
    │   ├── src-tauri/              # Tauri バックエンド
    │   │   ├── Cargo.toml
    │   │   └── src/
    │   ├── src/                    # React フロントエンド
    │   ├── package.json
    │   └── README.md
    │
    └── ussp-admin/                 # SFU管理GUI
        ├── src-tauri/              # Tauri バックエンド
        │   ├── Cargo.toml
        │   └── src/
        │       ├── lib.rs          # Tauriコマンド
        │       ├── api.rs          # HTTPクライアント
        │       └── main.rs         # エントリポイント
        ├── src/                    # React フロントエンド
        │   ├── App.tsx             # メインコンポーネント
        │   ├── components/         # UIコンポーネント
        │   │   ├── Dashboard.tsx   # ダッシュボード
        │   │   ├── Rooms.tsx       # ルーム一覧
        │   │   ├── RoomDetail.tsx  # ルーム詳細
        │   │   ├── Servers.tsx     # サーバー一覧
        │   │   └── Settings.tsx    # 接続設定
        │   └── types/              # 型定義
        └── package.json
```

## クイックスタート

### P2P通信テスト

```bash
# ターミナル1: 受信側
cargo run --example receiver

# ターミナル2: 送信側
cargo run --example sender
```

### SFUサーバー経由の通信テスト

```bash
# ターミナル1: SFUサーバー起動
cargo run -p ussp-server -- --no-auth

# ターミナル2: ルーム作成
curl -X POST http://localhost:8080/api/v1/rooms \
  -H "Content-Type: application/json" \
  -d '{"room_id": "test-room"}'

# ターミナル3: GUIテスターで送受信
cd apps/ussp-tester && npm run tauri dev
```

## テスト

```bash
# 全テスト実行
cargo test

# 特定のクレートのテスト
cargo test -p ussp
cargo test -p ussp-sfu
cargo test -p ussp-control

# 特定のテストを実行
cargo test test_header_roundtrip
```

## ライセンス

MIT License

## 今後の予定

- [ ] RTT計測と適応的FEC
- [ ] 帯域推定とビットレート制御
- [ ] NACK（否定応答）による再送要求
- [ ] マルチキャスト対応
- [ ] 暗号化（DTLS/SRTP）
- [ ] STUN/TURN/ICEによるNATトラバーサル
- [ ] SFUクラスタリング（複数サーバー間の同期）
- [ ] WebRTC互換ゲートウェイ
- [ ] Prometheus/Grafanaメトリクス連携
- [ ] Kubernetes/Docker対応
