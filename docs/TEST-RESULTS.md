# テスト結果・残作業

検証日: 2026-09-26／2026-09-29（日本時間）
状態: **Windows x64 exe・NSIS生成済み。Windows上15テスト成功。Windows 10/11実機での受け入れ確認は未実施。**

## 実行環境

Ubuntu 24.04 x64 / Rust 1.98.1 / TypeScript 6 / Vite 8.2.2。
対象: Windows 10/11 x64。9月29日はGitHub ActionsのWindows Server 2025 runner（windows-2025-vs2026）でネイティブビルドを実施しました。Windows 10/11実機のデスクトップ操作は未確認です。

## Windowsネイティブビルド（9月29日）

[成功した実行](https://github.com/aromaoil0312-max/sit-break/actions/runs/36485516520)

- ソースコミット: `825433b93d1bb86d60d9f4d6ab9512eb51707dd2`
- `npm ci`、TypeScript/Vite本番ビルド、`cargo test --locked` 成功（15件）。
- 12件のタイマー検証に加えて、初期設定・入力検証、短縮時間設定、Windowsレジストリ自動起動の登録／削除を確認。
- Tauri Windows x64 releaseビルド、NSIS生成、Artifactsアップロードが成功。
- 本体exe: 4,230,144 bytes、PE machine `0x8664`（x64）。
- 本体SHA-256: `3f32aec8439d9e7e218ac769971f36587e4c5385e66caf199002b752a71c9ec9`
- NSIS: 1,340,244 bytes。NSISの起動部分はx86形式で、同梱アプリは上記x64です。
- NSIS SHA-256: `7746a289d5a20e2195a5c3747622db0616fee3023410adf83f7685174031798d`
- 元ArtifactsのZIP SHA-256とGitHub提供digestの一致、ZIP CRC、LICENSE・NOTICE同梱を確認。
- ログ抜粋: [windows-native-build.txt](evidence/windows-native-build.txt)。

pushとPRのイベントで2実行が起動しました。追加の再ビルドは行わず、後続の文書更新には `[skip ci]` を付けています。配布ZIPは生成済みバイナリを変更せず、説明書とフォルダー構成を整えたものです。

## 先行するLinux環境での検証（9月26日）

| 項目 | 回数 | 結果 |
|---|---:|---|
| Rustタイマー単体テスト | 2 | 両方12件成功 |
| TypeScript検査＋Vite本番ビルド | 2 | 両方成功。フロントエンド出力合計約11KB（非圧縮、exeやWebView2を除く） |
| WindowsターゲットRust型チェック | 2 | 初回は依存ビルド途中で終了し確定結果なし。並列数を2に制限した2回目は成功 |
| Windows exeクロスビルド | 2 | 両方Microsoft CRT/SDK取得前に接続エラーで停止。exeなし |
| ブラウザーによる画面検証 | 2経路 | Cloud BrowserはローカルURLを拒否。ローカルPlaywrightはブラウザー取得失敗。未検証 |
| Git差分の空白・競合確認 | 1 | `git diff --check` 成功 |

Windows型チェックには `GNU compiler is not supported for this target` のリソースコンパイラー警告が出ています。Rustコードの型チェックは完了していますが、Windowsのリンク・アイコンリソース生成・実行を確認したものではありません。
試行を増やして見かけの成功を作らず、依頼の上限に達した項目は打ち切りました。

## 指定9ケースとの対応

| ケース | 自動確認できた範囲 | Windows実機 |
|---|---|---|
| 1 起動→120分 | 状態機械の初期値7200秒 | 未確認 |
| 2 作業1分→休憩 | 59.999秒までWork、60秒でBreak | 休憩画面表示は未確認 |
| 3 休憩1分→次の作業 | 60秒休憩後、自動でWork | 終了表示・自動復帰は未確認 |
| 4 一時停止・再開 | ミリ秒の端数を含む残り時間を維持 | トレイ・ボタン操作は未確認 |
| 5 ×→トレイ | closeイベントでhideする実装を確認 | 未確認 |
| 6 トレイ終了 | 終了メニューでapp.exitする実装を確認 | 未確認 |
| 7 スリープ・ロック | 80分作業＋1時間ロック後に残り40分、スリープ時計の停止を模擬 | 実Windowsイベント・RDPは未確認 |
| 8 緊急解除 | 29.999秒で拒否、30秒経過しても自動解除せず、確認で解除 | UI・実時間待機は未確認 |
| 9 長時間動作 | 不規則な137ms刻み、1000サイクルの仮想時刻で検査 | 実時間での長時間起動は未確認 |

追加確認: 休憩中のpause/reset/configure拒否、ロック時の解除取り消し、遅延しても未表示の休憩を飛ばさないこと、通知音だけの変更で時間をリセットしない設計。
Windowsレジストリ自動起動の単体テストは、9月29日のWindows runnerで成功しました。

## UI確認の状態

検証対象フロー: 通常画面 → 一時停止／再開、休憩画面 → 緊急解除待機 → 確認ボタン。
実行時画面の同一性、空白画面、オーバーレイ、コンソール、スクリーンショット、実クリックは**いずれも未確認**です。
TypeScriptとHTML/CSSをビルドできたことだけを、表示・操作が正常である証拠にはしていません。

## 実行コマンド

```sh
rustc --test src-tauri/src/timer.rs -o timer-tests
./timer-tests
npm run build
CARGO_BUILD_JOBS=2 cargo check --target x86_64-pc-windows-msvc --manifest-path src-tauri/Cargo.toml --locked
cargo xwin build --release --target x86_64-pc-windows-msvc --features tauri/custom-protocol --locked
```

exeビルドの停止理由:

```text
Error: Failed to setup MSVC CRT
HTTP GET request for https://aka.ms/vs/17/release/channel failed
io: connection refused
```

UIの停止理由:

```text
Cloud Browser: net::ERR_BLOCKED_BY_CLIENT (http://127.0.0.1:1420/panel.html)
Playwright: browser executable unavailable; browser download returned an invalid/truncated ZIP
```

## 完成までに必要な作業

1. Windows Actionsのテスト・exe/NSIS生成、forkへの反映、PR作成は完了。
2. 上表9ケースをWindows 10/11で受け入れ確認。日本語、音、最前面、×、Alt＋F4、ロック・スリープ復帰、設定保存を確認。
3. 未設定環境で初期値120分／5分、短縮モードが本番で無効、再起動・自動起動と二重起動を確認。
4. 高DPI・複数モニターで緊急解除ボタンが画面内に収まり、通知終了後に作業へ戻れることを確認。
5. 配布ZIPへexe・インストーラー・LICENSE・NOTICE・README・テスト結果・ビルド手順・SHA-256を同梱済み。

## GitHubの状態

元リポジトリの履歴を保持した利用者のforkへ、アプリ本体と説明書・ビルド設定を2コミットで反映しました。
- リポジトリ: https://github.com/aromaoil0312-max/sit-break
- ブランチ: `feature/120-5-break-timer`
- Draft PR: https://github.com/aromaoil0312-max/sit-break/pull/1
- GitHub側の2つのtree SHAがローカルコミットと一致することを確認済み。

利用者がActionsを有効化した後、featureブランチに起動用コミットを追加し、Windowsビルドの成功を確認しました。
mainへの直接変更、マージ、リリース公開は行っていません。
