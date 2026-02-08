# USSP Studio

USSPプロトコルによるストリーム送受信GUIアプリケーションです。Tauri 2 + React + TypeScriptで構築されています。

## 必要環境

- Node.js 18以上
- Rust 1.70以上
- macOS / Windows / Linux

## セットアップ

```bash
# リポジトリのクローン後
cd apps/ussp-studio

# 依存関係のインストール
npm install
```

## 起動方法

### 開発モード

```bash
npm run tauri dev
```

ホットリロードが有効になり、コード変更が即座に反映されます。

### リリースビルド

```bash
npm run tauri build
```

ビルド成果物は `src-tauri/target/release/bundle/` に出力されます。

- **macOS**: `.app`, `.dmg`
- **Windows**: `.exe`, `.msi`
- **Linux**: `.AppImage`, `.deb`

## 使い方

### モード切替

アプリ上部のタブで「Sender」と「Receiver」を切り替えられます。

---

### Sender（送信）モード

テストパターン（カラーバー）を生成し、USSPプロトコルで送信します。

#### 設定項目

| 項目 | 説明 | デフォルト |
|------|------|-----------|
| Destination Address | 送信先IPアドレス | 127.0.0.1 |
| Port | 送信先ポート番号 | 5001 |
| Audio Tracks | 音声トラック数 | 2 |
| FPS | フレームレート | 30 |
| FEC | 前方誤り訂正の有効/無効 | 有効 |

#### 統計表示

- **Packets Sent**: 送信済みパケット数
- **Bytes Sent**: 送信済みバイト数
- **Video Frames**: 送信済み映像フレーム数
- **Audio Frames**: 送信済み音声フレーム数
- **FPS**: 現在のフレームレート

#### テストパターン

送信モードでは、SMPTE風カラーバーと動くスキャンライン、フレーム番号を含むテストパターンが生成されます。

---

### Receiver（受信）モード

USSPストリームを受信し、映像プレビューと統計を表示します。

#### 設定項目

| 項目 | 説明 | デフォルト |
|------|------|-----------|
| Port | 受信ポート番号 | 5001 |

#### 統計表示

- **Packets Received**: 受信パケット数
- **Bytes Received**: 受信バイト数
- **Video Frames**: 受信映像フレーム数
- **Audio Frames**: 受信音声フレーム数
- **Packet Loss**: パケットロス率
- **A/V Sync Diff**: 映像と音声の同期差（マイクロ秒）
- **FPS**: 現在のフレームレート

#### 映像プレビュー

受信した映像フレームをCanvasにレンダリングして表示します。

#### 音声レベルメーター

各オーディオトラックのレベル（dB）をリアルタイム表示します。

---

## ローカルテスト手順

同一マシンで送受信をテストする場合:

1. **アプリを2つ起動**
   ```bash
   # ターミナル1
   npm run tauri dev

   # ターミナル2（別のウィンドウ）
   npm run tauri dev
   ```

2. **1つ目を送信モードに設定**
   - Senderタブを選択
   - Destination Address: `127.0.0.1`
   - Port: `5001`

3. **2つ目を受信モードに設定**
   - Receiverタブを選択
   - Port: `5001`

4. **受信開始 → 送信開始**
   - 受信側で「Start Receiving」をクリック
   - 送信側で「Start Sending」をクリック

5. **確認**
   - 受信側に映像プレビューが表示される
   - 両側で統計がリアルタイム更新される

---

## トラブルシューティング

### ポートが使用中

```
Error: Address already in use
```

別のポート番号を使用するか、既存のプロセスを終了してください。

### 映像が表示されない

- ファイアウォール設定を確認
- 送信先アドレスとポートが正しいか確認
- 受信側が先に起動しているか確認

### FPSが低い

- CPUの負荷を確認
- デバッグビルドの場合、リリースビルドを試す

---

## 技術スタック

- **フロントエンド**: React 18 + TypeScript
- **ビルドツール**: Vite 5
- **バックエンド**: Tauri 2 (Rust)
- **プロトコル**: USSP (独自実装)

## プロジェクト構造

```
ussp-studio/
├── src/                    # Reactフロントエンド
│   ├── App.tsx            # メインコンポーネント
│   ├── App.css            # スタイル
│   ├── components/
│   │   ├── Sender.tsx     # 送信UI
│   │   ├── Receiver.tsx   # 受信UI
│   │   ├── AudioMeter.tsx # 音声レベルメーター
│   │   └── Stats.tsx      # 統計表示
│   └── types/
│       └── index.ts       # TypeScript型定義
├── src-tauri/             # Tauriバックエンド
│   ├── Cargo.toml
│   ├── tauri.conf.json    # Tauri設定
│   └── src/
│       ├── lib.rs         # コマンド定義
│       ├── main.rs        # エントリポイント
│       ├── sender.rs      # 送信ロジック
│       ├── receiver.rs    # 受信ロジック
│       └── state.rs       # 状態管理
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## ライセンス

MIT License
