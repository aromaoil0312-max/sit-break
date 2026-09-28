# Windowsビルド

Windows 10/11 x64で、Node.js 22.12以降・Rust stable (MSVC)・Visual Studio Build Toolsの「C++によるデスクトップ開発」とWindows SDKを用意します。実行にはMicrosoft Edge WebView2 Runtimeが必要です。

```powershell
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- build --target x86_64-pc-windows-msvc
```

出力:
- `src-tauri/target/x86_64-pc-windows-msvc/release/sit-break.exe`
- `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe`

exeを配布するときはLICENSE・NOTICEも添付してください。NSISインストーラーにも両ファイルを同梱します。
インストーラーはWebView2をダウンロードしません。未導入PCにはMicrosoft公式のオフラインインストーラーで事前に導入してください。

## 開発モード

```powershell
$env:SIT_BREAK_TEST_MODE = "1"
npm run tauri -- dev
```

開発ビルドのみ60秒作業・30秒休憩となり、画面に「テスト」を表示します。終了後に `Remove-Item Env:SIT_BREAK_TEST_MODE` で解除します。
releaseビルドはこの変数を無視し、未設定時の初期値は必ず120分・5分です。
緊急解除の30秒待機を確認する場合、短縮モードは使わず、設定を作業1分・休憩1分にします。

## GitHub Actions

`.github/workflows/ci.yml` はWindows runnerでテストし、exeとNSISをArtifactsへ保存します。
featureブランチへのpush、PR、手動実行に対応しています。
タグのrelease workflowは公開を急がずdraft/pre-releaseを作成します。

## Linuxからexeを生成する場合

LLVM（clang-cl / lld-link / llvm-rc）、Rust、cargo-xwinを用意します。cargo-xwinがMicrosoft CRT/SDKを取得するため、開発時のみネット接続が必要です。

```sh
rustup target add x86_64-pc-windows-msvc
npm ci
npm run build
cd src-tauri
cargo xwin build --release --target x86_64-pc-windows-msvc --features tauri/custom-protocol --locked
```

クロスビルドはWindows実機での受け入れ確認の代わりにはなりません。
