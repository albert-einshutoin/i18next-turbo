# P1 watch差分抽出とNode並列出力保持（2026-10-14）

## watch設定解決の回帰検証（PR #81以降）

下記「再実行」のNode依存導入、CLI toolchainのlockコピーと`npm ci`、release buildを済ませてから実行する。
CLIは**1.75.3**に固定し、versionとtoolchainのlockが同梱`cli-lock.json`に一致しなければ検証前に失敗する。

```sh
python3 benchmarks/watch-incremental-20261014/run.py --regression --sizes 300 --runs 1
```

既存の303入力＋ignore対象1件、3言語・6namespace・18JSON、prune=trueのfixtureと独立literal期待値を再利用する。
初期＋6操作後の7状態をCLI／Rust直接／Nodeラッパーで全抽出し、21条件の一致と既存18翻訳の保持を確認する。
各watchでは型union変更、定数変更、追加、削除、純rename、依存module rename＋import更新を連続実行する。

| watch経路 | 設定引数 |
|---|---|
| Rust直接 | `--config i18next-turbo.json` |
| Node＋CJS | `--config i18next.config.cjs` |
| Node＋JSON（空plugins） | `-vci18next-turbo.json` |
| Node＋任意拡張子（空plugins） | `--config=project.conf` |
| Node＋拡張子なし（空plugins） | `-csettings` |

5経路×6操作＝30操作すべてで、完了通知、同じ状態の全抽出と独立期待値への一致、既存18翻訳保持、150ms後の一致、同期1回を必須とする。
続けてrename済みのimport先定数を解析不能な値へ変更し、5経路すべてでprune拒否の診断・終了コード1・全翻訳ファイルのbyte無変更を確認する。
欠測・timeout・不一致は非0終了とし、成功するまで再試行しない。結果は最後に表示する`REPORT`のJSON、各状態のcatalog snapshot、各watchの`process.log`へ保存する。
JSONの`mode: regression`、`status: completed`、`regression_checks`の21／30／5が成功条件。

これは各条件1回の機能回帰検証であり、速度比較ではない。Linux／macOSに対応し、生成物は`target/watch-import-evaluation-20261013/runs/`配下に限定する。
アプリsourceを実行せず、repo外の過去harnessや未追跡draftにも依存しない。plugin併用watchの拒否は別のNode wrapper integration testで維持する。
以降の速度表・保存結果は過去の測定記録であり、この回帰モードで更新しない。

### Linux CIと失敗時の動作

既存Ubuntu `benchmark-regression` jobで同じコマンドを実行する。Node 24、Python 3.11以上、同梱CLI lock、既存jobでbuildしたRustバイナリを使用する。
Nodeラッパー依存は`AGENTS.md`の`npm install --ignore-scripts --omit=optional --no-audit --no-fund --package-lock=false`で準備し、既存root lockを変更しない。CLI専用toolchainは同梱lockをコピーして`npm ci`する。
CPU情報はOSごとの方法で取得し、LinuxでCPU名が得られない場合は`null`として記録する。Windowsのwatch実動作は対応範囲外。

全抽出とwatchの子プロセスは専用process groupで起動する。正常・異常終了を問わず同じ終了処理を通し、親が先に終了しても残った子孫へTERM／KILLを送り、親を期限付きでwaitする。
全抽出のstdout／stderrはファイルへ直接保存するため、timeoutでも直前までの出力を保持する。実行中のエラーは`results.json`へ`status: failed`と診断を保存して非0終了する。
CIの`tee`は`pipefail`付きで終了コードを保持し、`if: always()`で実行ログ・結果・snapshot・process logを`benchmark-report`へ保存する。準備段階のエラーはCI stepのログに残る。

```sh
python3 -B -m unittest discover -s benchmarks/watch-incremental-20261014 -p test_runtime.py -v
```

この異常系チェックはLinuxの環境取得、code7の両出力、timeout時の出力保持と子孫停止、親終了後の子孫停止、runner自体の非0終了とfailed reportを検証する。

## P1改善の保存測定

相対importのある入力では変更ごとに全抽出していたwatchを、変更ファイルと影響する利用先の再解析へ切り替えた。初回の有限値indexを保持し、旧・新import候補の逆依存をたどる。rename後の同内容通知は解析・同期を省略する。入力/ignore境界を維持し、未解決・曖昧な値はprune前に拒否する。旧namespaceも同期し、削除cacheは影響sourceすべての解析成功後に更新する。

Node側はchild終了後の出力切捨てを防ぎ、stdout/stderrの先行write後のcallbackを両方待って明示終了する。child非0 codeを保持し、失敗時は後続hookを実行しない。既存assertと並列設定は変更していない。

## 一致・保持・影響範囲

- CLI **1.75.3**固定。同じ300/1,200件のTSX80%＋TS20%に共通3入力（実入力303/1,203件）とignore対象1件。20 consumer、追加後21 consumer。en/ja/ru・6namespace・18JSON、prune=true。
- 型union open|closed→open|pending、定数map actions.save→actions.publish、追加、削除、純rename、定数module rename＋21 consumer import更新の6操作。
- 2サイズ×6操作×Rust直接/Nodeラッパー×5回の **120 watch検証が全抽出と一致**。欠落/余分/値違い0。既存18翻訳を全回保持し、150ms後も一致。
- 操作後180回＋初期30回、計 **210全抽出**（CLI/Rust/Node各5回）が独立literal期待値に一致。前後source/config/seedの144 SHA照合も成功。
- 全120操作で全体再抽出0回、増分同期1回。型/定数は21ファイル、追加/純renameは1、削除は0、依存renameは22を再解析。前回の依存renameは全抽出2回。

## watch反映速度

中央値ms、成功条件n=5。変更開始→完了ログ＋全localeのcatalog一致、300ms debounceと5ms pollを含む。初回抽出/登録待ち1秒は除外。前回P0とP1の保存実測の比較であり、交互A/Bではない。背景負荷とfilesystem cacheの影響を含む。

| 規模 | 操作 | Rust P0→P1 | Node P0→P1 | CLI 1.75.3 P1時 | 再解析 |
|---|---|---:|---:|---:|---:|
| 300 | 型変更 | 371.23→361.08 (2.7%短縮) | 371.29→357.27 (3.8%短縮) | 296.02 | 21 |
| 300 | 定数変更 | 368.94→350.31 (5.1%短縮) | 362.97→346.79 (4.5%短縮) | 309.40 | 21 |
| 300 | 追加 | 369.09→335.27 (9.2%短縮) | 371.58→332.33 (10.6%短縮) | 8秒で完了未観測 | 1 |
| 300 | 削除 | 370.74→333.23 (10.1%短縮) | 365.98→329.07 (10.1%短縮) | 8秒で完了未観測 | 0 |
| 300 | 純rename | 368.07→332.54 (9.7%短縮) | 360.59→338.63 (6.1%短縮) | 8秒で完了未観測 | 1 |
| 300 | 依存rename＋import更新 | 374.77→333.38 (11.0%短縮) | 364.41→334.38 (8.2%短縮) | 3609.78 | 22 |
| 1200 | 型変更 | 486.28→403.58 (17.0%短縮) | 505.62→397.32 (21.4%短縮) | 1093.38 | 21 |
| 1200 | 定数変更 | 493.01→407.25 (17.4%短縮) | 497.48→407.42 (18.1%短縮) | 1031.29 | 21 |
| 1200 | 追加 | 490.94→390.21 (20.5%短縮) | 486.32→395.90 (18.6%短縮) | 8秒で完了未観測 | 1 |
| 1200 | 削除 | 481.57→397.29 (17.5%短縮) | 484.42→397.69 (17.9%短縮) | 8秒で完了未観測 | 0 |
| 1200 | 純rename | 491.28→396.60 (19.3%短縮) | 512.29→397.23 (22.5%短縮) | 8秒で完了未観測 | 1 |
| 1200 | 依存rename＋import更新 | 483.56→403.37 (16.6%短縮) | 486.03→396.02 (18.5%短縮) | 8秒で完了未観測 | 22 |

1,200件では16.6〜22.5%短縮。300件の型/定数変更はCLIの方が速い。CLI watchの追加/削除/純rename（両サイズ）と1,200件の依存renameは8秒で打ち切り、欠測として保持し残4回は未実施。成功まで再試行しない。純renameも完了ログを必須とし同値catalogだけで0msとは扱わない。全samples、min/max、全抽出時間と差分は[p0-watch.json](p0-watch.json) / [p1-watch.json](p1-watch.json)。

## Node修正の初期検証（最終結果は追補）

- 修正前の既存Node13件は並列でdrainとplugin fail-fastの2件が失敗。修正後は同13件が成功。
- 成功/child code7/plugin例外の3条件×8回、8worker並列で、child stdout/stderr各262,144 bytesと大きな例外診断を検証。保存した旧24条件は全件で出力保持失敗、新24条件は全件で完全出力・code0/7/1保持・非0でafterSync非実行を確認。[node-output.json](node-output.json)。
- 最終通常test204件、全feature **通常並列220件**成功（失敗/ignored 0）。RUST_TEST_THREADS unset、直列化なし。Node/native13件を含む。Clippy -D warnings、fmt、JS構文、release/native build、diff checkも成功。[validation.json](validation.json)。
- watch11件・CLI互換12件、Rust/JavaScript専門レビュー、最終Stop Review Gateは成功/静的ALLOW。静的レビューは実行証拠と区別する。
- P1速度・210全抽出はNode終了修正前の測定。最終Node修正後は並列/全体検証を実施し、1,200件の速度を再測定したとは扱わない。source/binary hashと段階の違いは[evidence.json](evidence.json)とrawのenvironmentで明示。

## 再実行

Python3.11以上、Node、Rust、gitで上記base commitを参照可能にする。測定はApple M4 / arm64 / macOS / Node v24.2.0。watch scriptのsysctl環境取得とNode probeの実行stubはこのUnix/Mac条件向け。別OS、実案件全パターン、pluginありwatch、公開配布の保証ではない。固定CLIは専用target toolchainで使い、隣のCLI checkoutは使用しない。
既存のevaluation-20261013/run.pyのfixture/catalog/helperを再利用し、sourceアプリコードは実行しない。生成データ/ログ/reportはtarget配下の新しいrunディレクトリだけに保存する。Node probeはbinに所有する一時entryを作り、終了時に削除する。これらの測定scriptはscope外の既存ファイルを変更しない。依存準備はroot node_modulesを変更するため、既存環境を保存したい場合は別checkoutで実行する。

```sh
# repo root
npm install --ignore-scripts --omit=optional --no-audit --no-fund --package-lock=false
mkdir -p target/watch-import-evaluation-20261013/toolchain
cp benchmarks/watch-incremental-20261014/cli-package.json target/watch-import-evaluation-20261013/toolchain/package.json
cp benchmarks/watch-incremental-20261014/cli-lock.json target/watch-import-evaluation-20261013/toolchain/package-lock.json
npm ci --prefix target/watch-import-evaluation-20261013/toolchain --ignore-scripts --no-audit --no-fund
cargo build --release --locked
python3 benchmarks/watch-incremental-20261014/run.py --sizes 300 1200 --runs 5
python3 benchmarks/watch-incremental-20261014/node_output.py
cargo build --lib --all-features --locked
# Mac/arm64
cp target/debug/libi18next_turbo.dylib target/debug/i18next-turbo.darwin-arm64.node
cargo test --all-targets --locked
cargo test --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt -- --check
node --check bin/cli.js
```

公開用scriptは計測harnessの配置/保存先と保護対象の参照をrepo内に揃えたもの。原計測と同じfixture/独立期待値/完了判定/統計を使う。原計測harnessのSHAもevidence.jsonに記録する。raw結果はbyte単位で元の保存物からcopyし、失敗・timeoutを削除していない。CLI lockとtarball provenanceは[cli-lock.json](cli-lock.json) / [cli-install.json](cli-install.json)。
Hosted CIの結果はPR上の最終headに対する別証拠とし、この資料のローカル結果へ混ぜない。

保存測定とsource bytesが一致するコミット: P1 `01af467a52cc93485575a332caf52674d95d49b7`、Node修正後の全体検証 `9c6490cc220a681970fe956481dcb559e343dbe9`。測定は未commit時に実行しており、ここでは後から一致するtreeを照合した。配布位置を直したscriptの30件×1回確認（全抽出21/watch12）とNode24件は[packaging-checks.json](packaging-checks.json)に分離した。

公開script確認のrawは[packaging-watch.json](packaging-watch.json) / [packaging-node.json](packaging-node.json)。実行command・exit status・日時はpackaging-checks.json、各ファイルSHAはevidence.jsonに保存。root wrapper依存は追跡済みpackage-lock.json、CLI依存は専用cli-lock.jsonで固定する。

## PR #80の終了処理P1指摘への追補

自然終了だけではsetupのinterval等が残るとCLIが終了しない回帰を確認した。終了コードをmainから返し、stdout/stderrの空write callbackが両方完了した後にprocess.exitする共通経路へ修正。正常drain時は0/child非0/例外1を保持し、write失敗は1。新規依存・固定sleep・汎用teardownは追加しない。

成功0、child failure7、afterSync failure1の3回帰testを追加。pluginのintervalを残し、child stdout/stderr各256KiBと大きな診断の全byte、後続hookの実行有無、5秒以内の終了を検証。両pipeを並列読取し、timeout時は所有するCLIをkill/waitして回収する。修正前は3件ともtimeout、修正後はNode/native16件・通常207件・全feature通常並列223件が成功。Clippy -D warnings、fmt、JS構文、actionlint、release/native build、diff checkも成功。

CIのnode-smokeはnpm依存導入後にcargo test --test node_wrapper_integration --lockedを実行する。lintで依存未導入のためtestがskipする状況と区別し、回帰を実際に実行する。source SHA・全実command・結果は[plugin-exit-validation.json](plugin-exit-validation.json)。任意のbackground outputの無期限継続を保証するものではなく、command/hook完了時の先行writeをdrainしてCLIを終了させる。

P0/P1の大規模測定は保存済みの歴史的結果のまま保持し、この追補で速度再測定したとは扱わない。initial_node_*は初期Node修正のsource/検証、latest_plugin_exit_fixが今回の最終検証を示す。Hosted CIはPRの最終headとmerge後mainの別証拠として確認する。
