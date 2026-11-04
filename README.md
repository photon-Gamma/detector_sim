# pd_anim

光検出器の動作を粒子シミュレーションで計算し、解説動画 (1280×720, 30 fps) を 2 本書き出す Rust プログラム。外部クレートなし。動画の圧縮は ffmpeg か、Python の OpenCV のどちらかを使う。

- `photodetector.mp4` (90 秒) … Si PIN フォトダイオードと APD
- `pmt.mp4` (96 秒) … 光電子増倍管 (PMT, 7 段)、ハイブリッド光検出器 (HPD)、増倍ゆらぎと過剰雑音係数 F の比較

詳しい解説:
- コード: [pd_anim コード解説](https://claude.ai/artifact/BGurY7QnXqmUR2YUNGJ2u8)
- 物理: [光検出器の物理](https://claude.ai/artifact/BwWtb4W6ViTxFZ3G9XWPU3) (同じ内容が `docs/光検出器の物理.md` にもある)
- 出典: [参考文献](https://claude.ai/artifact/9Rq1VWLRAjBbezdhQuS5EF) (`docs/参考文献.md`)

## 必要なもの

- Rust (cargo)
- 動画のエンコーダ (どちらか一方)
  - ffmpeg … Mac は `brew install ffmpeg`、Windows は `winget install --id Gyan.FFmpeg -e`
  - Python + OpenCV … `pip install opencv-python` (ffmpeg のインストール不要)
- 日本語の TrueType フォント … Mac は Arial Unicode を自動で使う。別のフォントは `PD_FONT=/path/to/font.ttf` で指定 (.otf は不可)

## 使い方

| やりたいこと | コマンド |
| --- | --- |
| 動画をすべて作る | `cargo run --release` → `out/photodetector.mp4` と `out/pmt.mp4` |
| 片方だけ作る | `cargo run --release -- render pd` / `render pmt` |
| 出力先を変える | `cargo run --release -- --out フォルダ名` |
| エンコーダを選ぶ | `cargo run --release -- --encoder opencv` (`auto` / `ffmpeg` / `opencv`) |
| 1コマを PNG で確認 | `cargo run --release -- still pmt 120 300` |
| 増倍率の統計だけ見る | `cargo run --release -- gainstats [apd｜apd70｜pmt｜hpd] [回数]` |
| 使い方を表示 | `cargo run --release -- help` |
| テスト / 静的チェック | `cargo test` / `cargo clippy` |

実行ログを残すとき: `cargo run --release 2>&1 | tee run.log`

## エンコーダ: ffmpeg と OpenCV

`--encoder auto` (既定) では、ffmpeg があれば ffmpeg、なければ Python の OpenCV を使う。使ったものは最初に「エンコーダ: …」と表示される。

| | ffmpeg | OpenCV |
| --- | --- | --- |
| 必要なもの | ffmpeg コマンド | Python と `opencv-python` |
| コーデック | H.264 (libx264, crf 20) | H.264 (avc1) を試し、使えなければ MPEG-4 Part 2 (mp4v) |
| ファイルの大きさ (pmt.mp4) | 約 3 MB | mp4v だと約 29 MB (画質はほぼ同じ) |
| シーンごとのファイル | 作る (`pmt_hpd.mp4` など) | 作らない (1 本を通しで書く) |

- OpenCV で H.264 が使えるかは環境しだい。pip の opencv-python では mp4v になることが多い (Linux・Windows)。Mac では AVFoundation 経由で avc1 が使えることがある。どちらになったかは「OpenCV (mp4v): 2870 コマ」のように表示される
- mp4v は QuickTime・Windows のメディアプレーヤー・PowerPoint で再生できる。小さくしたいときは下の `compress_mp4.py` で H.264 に変換する
- OpenCV で使う Python は `python3` → `python` → `py` の順に探す。別のものを使うときは `PD_PYTHON=/path/to/python`
- コーデックを決め打ちするときは `PD_FOURCC=mp4v` など
- OpenCV 用のスクリプト (`tools/cv_writer.py`) はビルド時にバイナリへ埋め込まれるので、実行時にファイルの場所は関係ない
- `still` (PNG の保存) はエンコーダを使わず、Rust だけで書き出す

## mp4 をさらに圧縮する (Python)

ffmpeg なしで、できた mp4 を H.264 で圧縮し直せる。PyAV (`av`) が ffmpeg のライブラリを同梱している。

```
pip install -r tools/requirements.txt
python tools/compress_mp4.py out/pmt.mp4               # → out/pmt_small.mp4
python tools/compress_mp4.py out/*.mp4 --crf 28        # まとめて、もっと小さく
python tools/compress_mp4.py out/pmt.mp4 --scale 0.5   # 縦横を半分に
```

| オプション | 意味 |
| --- | --- |
| `--crf` | 画質 (小さいほど高画質で大きい。18 ほぼ劣化なし / 23 標準 / 28 かなり小さい) |
| `--preset` | 速さと圧縮率 (`ultrafast` … `medium` … `veryslow`。遅いほど小さい) |
| `--scale` | 縦横の倍率 |
| `-o` | 出力ファイル、または出力フォルダ |
| `--encoder` | `libx264` / `h264_videotoolbox` / `h264` を指定 (省略時は使えるものを自動で選ぶ) |
| `--bitrate` | libx264 以外のときのビットレート (既定 1500k) |

書き出しは `.part.mp4` に行い、最後まで書けたときだけ名前を変える。音声は扱わない。

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
└── video/         動画の書き出し (ffmpeg / OpenCV の選択、連結) と PNG (png.rs)
docs/              物理の解説と参考文献 (Markdown)
tools/             cv_writer.py (OpenCV で mp4 を書く、バイナリに埋め込み)、compress_mp4.py (mp4 の圧縮、PyAV)
```

依存は main → scenes → render / physics の一方向。physics は描画に依存しないので単独でテストできる。

## 安全性

- `Cargo.toml` の `[lints]` で unsafe・`unwrap`・`expect`・`panic` を禁止
- 壊れたフォント、NaN の座標、範囲外の引数でもパニックせず、日本語のエラーで止まる
- 書き出しは `.part.mp4` に行い、成功したときだけ名前を変える (失敗時は消す)
- `cargo test` 45 件 (電荷保存、Si PD の M = 1、PMT・HPD の F が理論値と一致、不正入力への耐性など)

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
