# pd_anim

光検出器の動作を粒子シミュレーションで計算し、解説動画 (1280×720, 30 fps) を 2 本書き出す Rust プログラム。外部クレートなし。動画の圧縮だけ ffmpeg を使う。

- `photodetector.mp4` (90 秒) … Si PIN フォトダイオードと APD
- `pmt.mp4` (96 秒) … 光電子増倍管 (PMT, 7 段)、ハイブリッド光検出器 (HPD)、増倍ゆらぎと過剰雑音係数 F の比較

詳しい解説:
- コード: [pd_anim コード解説](https://claude.ai/artifact/BGurY7QnXqmUR2YUNGJ2u8)
- 物理: [光検出器の物理](https://claude.ai/artifact/BwWtb4W6ViTxFZ3G9XWPU3) (同じ内容が `docs/光検出器の物理.md` にもある)
- 出典: [参考文献](https://claude.ai/artifact/9Rq1VWLRAjBbezdhQuS5EF) (`docs/参考文献.md`)

## 必要なもの

- Rust (cargo)
- ffmpeg … Mac は `brew install ffmpeg`
- 日本語の TrueType フォント … Mac は Arial Unicode を自動で使う。別のフォントは `PD_FONT=/path/to/font.ttf` で指定 (.otf は不可)

## 使い方

| やりたいこと | コマンド |
| --- | --- |
| 動画をすべて作る | `cargo run --release` → `out/photodetector.mp4` と `out/pmt.mp4` |
| 片方だけ作る | `cargo run --release -- render pd` / `render pmt` |
| 出力先を変える | `cargo run --release -- --out フォルダ名` |
| 1コマを PNG で確認 | `cargo run --release -- still pmt 120 300` |
| 増倍率の統計だけ見る | `cargo run --release -- gainstats [apd｜apd70｜pmt｜hpd] [回数]` |
| 使い方を表示 | `cargo run --release -- help` |
| テスト / 静的チェック | `cargo test` / `cargo clippy` |

実行ログを残すとき: `cargo run --release 2>&1 | tee run.log`

## シーン

| 動画 | 名前 | 内容 | 長さ |
| --- | --- | --- | --- |
| photodetector | title | タイトル | 4 秒 |
| photodetector | si | Si PIN PD: 1光子のスロー再生 → 連続光 | 18 秒 |
| photodetector | apd | APD: 1光子からのなだれ増倍をスローで → 連続光 | 24 秒 |
| photodetector | compare | 同じ光での出力比較、光子ごとの M のゆらぎ (σ, F) | 12 秒 |
| photodetector | histogram | 1光子ごとの利得 M の分布 (1,000 回の事前計算 + ライブ追加) | 14 秒 |
| photodetector | clt | N 個の光子の平均利得の分布 (中心極限定理と σ/√N) | 18 秒 |
| pmt | pmt_title | タイトル | 4 秒 |
| pmt | pmt | 光電子 1 個が 7 段のダイノード (各段 4) で増える様子 | 22 秒 |
| pmt | pmt_hist | PMT の増倍率の分布 (1,000 回の事前計算 + 追加) | 14 秒 |
| pmt | pmt_vs_apd | PMT と APD (〈M〉≈70) の増倍ゆらぎの比較 | 18 秒 |
| pmt | hpd | HPD: 電子打ち込み (平均 200) + APD (平均 70) の 2 段増倍 | 20 秒 |
| pmt | pmt_vs_hpd | PMT と HPD の分布と過剰雑音係数 F の比較 | 18 秒 |

`still` では `pmt:1` のように 動画名:番号 でも指定できる。

## フォルダ構成

```
src/
├── main.rs        コマンドを実行 (エラーは終了コードで返す)
├── cli.rs         引数の解析と検証
├── error.rs       エラー型
├── physics/       物理モデル (描画を知らない)
│   ├── device.rs    層・電界・衝突電離率 (平均利得 70 の APD の設定も)
│   ├── sim.rs       光子・電子・正孔のシミュレーション、光子ごとの利得
│   ├── pmt.rs       PMT のダイノード増倍 (ポアソン二次電子放出)
│   ├── hpd.rs       HPD: 電子打ち込み (ポアソン) × APD 増倍、2 段の F の式
│   ├── stats.rs     平均・σ・過剰雑音係数 F・歪度・標本平均
│   └── rng.rs       再現性のある乱数
├── render/        描画
│   ├── raster.rs    アンチエイリアス付きの塗りつぶし
│   ├── font.rs      TrueType フォントの読み込み
│   ├── canvas.rs    図形と文字の描画 API
│   ├── theme.rs     画面サイズと配色
│   └── widgets.rs   断面図・グラフ・字幕などの部品
├── scenes/        動画ごとにフォルダ、1シーン1ファイル (mod.rs に動画とシーンの一覧)
│   ├── pd/          photodetector.mp4 のシーン
│   └── pmt/         pmt.mp4 のシーン
└── video/         ffmpeg での書き出しと連結
docs/              物理の解説と参考文献 (Markdown)
```

依存は main → scenes → render / physics の一方向。physics は描画に依存しないので単独でテストできる。

## 安全性

- `Cargo.toml` の `[lints]` で unsafe・`unwrap`・`expect`・`panic` を禁止
- 壊れたフォント、NaN の座標、範囲外の引数でもパニックせず、日本語のエラーで止まる
- 書き出しは `.part.mp4` に行い、成功したときだけ名前を変える (失敗時は消す)
- `cargo test` 41 件 (電荷保存、Si PD の M = 1、PMT・HPD の F が理論値と一致、不正入力への耐性など)

## よく変えるパラメータ

| 変えたいこと | 場所 |
| --- | --- |
| APD の増倍の強さ | `src/physics/device.rs` の `DeviceSpec::apd()` と `APD70_ALPHA_E` |
| 電子・正孔の速さ | `src/physics/sim.rs` の `VE` / `VH` |
| PMT の段数とゲイン | `src/physics/pmt.rs` の `PmtSpec::standard()` |
| HPD の打ち込みゲイン・APD の利得 | `src/physics/hpd.rs` の `EB_GAIN`・`APD_DESIGN_GAIN` (APD の実際の設定は `APD70_ALPHA_E`) |
| 光の強さ | 各シーンの `PHOTON_RATE` |
| 事前計算の回数 | `scenes/pd/histogram.rs`・`scenes/pmt/histogram.rs` の `PRECOMPUTED`、`scenes/pd/clt.rs` の `BATCHES` |
| 色 | `src/render/theme.rs` |
