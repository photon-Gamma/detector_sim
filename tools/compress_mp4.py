#!/usr/bin/env python3
"""mp4 を H.264 で圧縮し直す (ffmpeg のインストール不要、PyAV を使う)。

準備:
    pip install -r tools/requirements.txt      (中身は av だけ)

使い方:
    python tools/compress_mp4.py out/pmt.mp4                    → out/pmt_small.mp4
    python tools/compress_mp4.py out/*.mp4 --crf 28             → まとめて、もっと小さく
    python tools/compress_mp4.py in.mp4 -o small.mp4 --preset slow
    python tools/compress_mp4.py in.mp4 --scale 0.5             → 縦横を半分に

画質と大きさ:
    --crf      小さいほど高画質で大きい。18 ≒ 見た目ほぼ劣化なし、23 = 標準、28 = かなり小さい
    --preset   遅いほど同じ画質で小さくなる (ultrafast … medium … veryslow)

エンコーダは libx264 → h264_videotoolbox (Mac) → h264 の順に、使えるものを選ぶ。
videotoolbox は crf が使えないので、--bitrate (既定 1500k) で大きさを決める。
音声は扱わない (pd_anim の動画には音声がない)。音声があれば警告して捨てる。
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

try:
    import av
except ImportError:
    sys.exit("PyAV がありません。`pip install av` を実行してください。")

# PyAV の版によって置き場所が違うので、見つからなければ OSError 以外の例外も広く受ける
FFmpegError = getattr(av, "FFmpegError", None) or getattr(getattr(av, "error", None), "FFmpegError", Exception)

ENCODERS = ("libx264", "h264_videotoolbox", "h264")
PRESETS = ("ultrafast", "superfast", "veryfast", "faster", "fast",
           "medium", "slow", "slower", "veryslow")


def pick_encoder(wanted: str | None) -> str:
    """使える H.264 エンコーダの名前を返す。"""
    names = [wanted] if wanted else list(ENCODERS)
    for name in names:
        try:
            av.codec.Codec(name, "w")
            return name
        except Exception:  # 見つからない・エンコードできない
            continue
    raise SystemExit(f"エンコーダが見つかりません: {', '.join(names)}")


def parse_bitrate(s: str) -> int:
    s = s.strip().lower()
    mult = {"k": 1_000, "m": 1_000_000}.get(s[-1:], 1)
    num = s[:-1] if s[-1:] in "km" else s
    try:
        value = int(float(num) * mult)
    except ValueError:
        raise argparse.ArgumentTypeError(f"ビットレートの形式が違います: {s} (例: 1500k, 2m)")
    if value <= 0:
        raise argparse.ArgumentTypeError("ビットレートは正の値にしてください")
    return value


def even(x: float) -> int:
    """yuv420p は縦横が偶数でないといけない"""
    return max(2, int(round(x / 2)) * 2)


def compress(src: Path, dst: Path, args: argparse.Namespace, encoder: str) -> None:
    if src.resolve() == dst.resolve():
        raise SystemExit(f"入力と出力が同じファイルです: {src}")
    tmp = dst.with_name(dst.stem + ".part" + dst.suffix)
    t0 = time.perf_counter()
    frames = 0
    try:
        with av.open(str(src)) as inp, av.open(str(tmp), "w") as out:
            if not inp.streams.video:
                raise SystemExit(f"映像がありません: {src}")
            if inp.streams.audio:
                print(f"  注意: {src.name} の音声は捨てます", file=sys.stderr)
            ivs = inp.streams.video[0]
            ivs.thread_type = "AUTO"
            ictx = ivs.codec_context

            rate = ivs.average_rate or ivs.guessed_rate or 30
            ovs = out.add_stream(encoder, rate=rate)
            ovs.width = even(ictx.width * args.scale)
            ovs.height = even(ictx.height * args.scale)
            ovs.pix_fmt = "yuv420p"
            # タイムスタンプは入力のものをそのまま使う (コマ落ちや可変フレームレートでも長さが変わらない)
            ovs.time_base = ivs.time_base
            ovs.codec_context.time_base = ivs.time_base
            if encoder == "libx264":
                ovs.options = {"crf": str(args.crf), "preset": args.preset}
            else:
                ovs.bit_rate = args.bitrate

            for frame in inp.decode(ivs):
                if frame.pts is None:
                    continue
                pts = frame.pts
                frame = frame.reformat(width=ovs.width, height=ovs.height, format="yuv420p")
                frame.pts = pts
                frame.time_base = ivs.time_base
                for packet in ovs.encode(frame):
                    out.mux(packet)
                frames += 1
            for packet in ovs.encode():  # エンコーダに残ったコマを出し切る
                out.mux(packet)
        tmp.replace(dst)  # 最後まで書けたときだけ本来の名前にする
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise

    a, b = src.stat().st_size, dst.stat().st_size
    dt = time.perf_counter() - t0
    print(f"{src.name}: {a / 1e6:.2f} MB → {dst.name}: {b / 1e6:.2f} MB "
          f"({b / a * 100:.0f}%)  {frames} コマ, {dt:.1f} 秒")


def main() -> None:
    p = argparse.ArgumentParser(description="mp4 を H.264 で圧縮し直す (PyAV)")
    p.add_argument("inputs", nargs="+", type=Path, help="入力 mp4 (複数可)")
    p.add_argument("-o", "--output", type=Path,
                   help="出力ファイル (入力が 1 つのとき) かフォルダ。省略時は 〇〇_small.mp4")
    p.add_argument("--crf", type=int, default=23, help="画質 0–51 (libx264、既定 23)")
    p.add_argument("--preset", default="medium", choices=PRESETS, help="速さと圧縮率 (libx264)")
    p.add_argument("--bitrate", type=parse_bitrate, default=parse_bitrate("1500k"),
                   help="libx264 以外で使うビットレート (既定 1500k)")
    p.add_argument("--scale", type=float, default=1.0, help="縦横の倍率 (例 0.5)")
    p.add_argument("--encoder", choices=ENCODERS, help="エンコーダを指定する")
    args = p.parse_args()

    if not 0 <= args.crf <= 51:
        p.error("--crf は 0〜51 にしてください")
    if not 0.05 <= args.scale <= 4:
        p.error("--scale は 0.05〜4 にしてください")
    if args.output and len(args.inputs) > 1 and not args.output.is_dir():
        p.error("入力が複数のときは -o にフォルダを指定してください")

    encoder = pick_encoder(args.encoder)
    print(f"エンコーダ: {encoder}"
          + (f" (crf {args.crf}, {args.preset})" if encoder == "libx264"
             else f" ({args.bitrate // 1000} kb/s)"))

    failed = 0
    for src in args.inputs:
        if not src.is_file():
            print(f"見つかりません: {src}", file=sys.stderr)
            failed += 1
            continue
        if args.output is None:
            dst = src.with_name(src.stem + "_small.mp4")
        elif args.output.is_dir():
            dst = args.output / (src.stem + "_small.mp4")
        else:
            dst = args.output
        try:
            compress(src, dst, args, encoder)
        except (FFmpegError, OSError, ValueError) as e:
            print(f"失敗: {src}: {e}", file=sys.stderr)
            failed += 1
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
