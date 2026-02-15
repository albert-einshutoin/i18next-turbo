# パフォーマンステスト

## クイック実行

```bash
npm run benchmark
# または
node scripts/run-benchmark.mjs 5
```

## CI向けしきい値判定

```bash
npm run benchmark:ci
```

このコマンドは以下を実施します。
- 5回の計測を実行
- `benchmarks/latest.json` にJSONレポートを保存
- 速度比が `10x` 未満なら失敗

## カスタム実行

```bash
node scripts/run-benchmark.mjs 5 \
  --json benchmarks/latest.json \
  --min-speedup 20
```

## 現在のサンプル結果（共有値）

- 実行A:
  - turbo 平均: `8.62 ms`
  - cli 平均: `419.20 ms`
  - 速度比: `~48.60x`
- 実行B:
  - turbo 平均: `14.21 ms`
  - cli 平均: `1034.04 ms`
  - 速度比: `~72.77x`

## 注意

- 計測時間はプロセス起動とI/Oを含みます。
- 比較時は同一マシン・低負荷状態で実行してください。

## プラグインオーバーヘッド測定

```bash
npm install
npm run benchmark:plugin
# または
node scripts/run-plugin-benchmark.mjs 5 --json benchmarks/plugin-latest.json
```

しきい値判定:

```bash
node scripts/run-plugin-benchmark.mjs 5 --max-overhead 0.20
```

- 同一fixtureで plugin off/on を比較
- オーバーヘッド比率 `(withPlugin.avg - withoutPlugin.avg) / withoutPlugin.avg` を出力
