# Rustpture

Rustpture は、Windows向けの軽量な画面範囲ピン留めツールです。Raptureの「切り取った画面をその場に置いて参照する」操作感を、RustとWin32 APIだけで小さく作り直しています。

![Rustpture icon](resources/rustpture-preview.png)

## 特徴

- 常駐中はWin32のメッセージ待ちだけ。タスクバーをクリックすると、事前作成済みの選択画面を即座に表示
- マルチモニターと負の画面座標に対応
- Per-Monitor V2 DPI対応。異なる拡大率のモニターへ移しても、画像の表示範囲とズーム率を維持
- 枠なし・常に手前の画像ウィンドウ
- 画像ウィンドウを複数表示可能
- WebView、GUIフレームワーク、常駐タイマーなし
- MIT License

## 操作

### 範囲選択

| 操作 | 動作 |
|---|---|
| 左ドラッグ | 範囲を選択してピン留め |
| `Esc` | 選択をキャンセル |

### ピン留め画像

| 操作 | 動作 |
|---|---|
| 左ドラッグ | 移動 |
| マウスホイール | カーソル位置を中心に拡大・縮小 |
| `Ctrl` + マウスホイール | 透明度を変更 |
| 左ダブルクリック | 100%表示へ戻す |
| 右クリック | ネイティブメニューを表示 |
| `Alt` + `F4` | その画像を閉じる |

右クリックメニューには、再キャプチャ、クリップボードへのコピー、ズーム率、画面に合わせる、常に手前、画面内へ戻す、終了を用意しています。

## 必要環境

- Windows 10またはWindows 11
- Rust 1.85以降
- MSVC C++ Build ToolsとWindows SDK

RustをMSVC版で導入し、Visual Studio Build Toolsの「C++によるデスクトップ開発」を入れてください。

## ビルド

PowerShellでプロジェクトのルートを開きます。

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1
```

生成物：

```text
dist\Rustpture.exe
```

手動でビルドする場合：

```powershell
cargo build --release
```

## インストール

ビルド、`%LOCALAPPDATA%\Rustpture` への配置、スタートメニューとスタートアップへのショートカット作成をまとめて行います。

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

スタートメニューで **Rustpture** を右クリックし、タスクバーにピン留めしてください。常駐後は、タスクバーのアイコンをクリックするだけで範囲選択が始まります。

アンインストール：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall.ps1
```

## コマンドライン

```text
Rustpture.exe                 範囲選択を開始し、その後も常駐
Rustpture.exe --capture       同上
Rustpture.exe --background    選択を開始せず常駐
Rustpture.exe --resident      --backgroundの別名
Rustpture.exe --quit          常駐中のRustptureを終了
```

すでに起動している場合、新しいプロセスは既存プロセスへ命令を送ってすぐ終了します。

## 設計

```text
最小化されたコントローラー窓
├─ タスクバー上の起動点
├─ GetMessageWで待機
└─ クリック時に非表示済みオーバーレイを表示

選択オーバーレイ
├─ 仮想画面全体を覆う
├─ Escでキャンセル
└─ 選択確定後に非表示 → DwmFlush → GDI BitBlt

ピン留め窓
├─ WS_POPUP（枠なし）
├─ WS_EX_LAYERED（透明度）
├─ WS_EX_TOPMOST（常に手前）
└─ GDI StretchBltで元画像を非破壊表示
```

詳細は [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) を参照してください。

## 現時点の制約

- 保護された映像や一部のGPUオーバーレイは、Windows側の制限により取得できない場合があります。
- HDR画面では、GDI経由の色が元表示と完全一致しない場合があります。
- 描画、文字入れ、画像連結、保存履歴はまだ実装していません。ピン留めの速さと軽さを優先しています。

## 開発時の確認

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

実機確認項目は [`docs/TESTING.md`](docs/TESTING.md) にあります。
検証範囲とWindows CIについては [`docs/VALIDATION.md`](docs/VALIDATION.md) にまとめています。
