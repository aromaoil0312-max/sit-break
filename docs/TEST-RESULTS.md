# テスト結果・残作業

確認日: 2026-09-26（日本時間）
状態: **実装済みの開発版。配布用exe未生成、Windows実機未検証。完成条件は未達。**

## 実行環境

Ubuntu 24.04 x64 / Rust 1.98.1 / TypeScript 6 / Vite 8.2.2。
対象: Windows 10/11 x64。今回Windows実機、Windows runnerは利用できていません。

## 実行した検証

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
Windowsレジストリ自動起動の単体テストもコードに含めていますが、今回は未実行です。

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

1. 利用者のforkにfeatureブランチをpushし、Windows Actionsで`cargo test`・exe/NSIS生成を実施。
2. 上表9ケースをWindows 10/11で受け入れ確認。日本語、音、最前面、×、Alt＋F4、ロック・スリープ復帰、設定保存を確認。
3. 未設定環境で初期値120分／5分、短縮モードが本番で無効、再起動・自動起動と二重起動を確認。
4. 高DPI・複数モニターで緊急解除ボタンが画面内に収まり、通知終了後に作業へ戻れることを確認。
5. ポータブルzipへexe・LICENSE・NOTICE・READMEを同梱し、SHA-256を付けて配布。

## GitHubの状態

元リポジトリは全履歴をclone済み。ローカルブランチ `feature/120-5-break-timer` に変更をコミットしています。
接続済みGitHubツールはfork／新規リポジトリ作成に非対応で、ブラウザーも未ログインだったため、forkとPRは未作成です。
ユーザーの既存の無関係なリポジトリや元リポジトリのmainには書き込んでいません。
