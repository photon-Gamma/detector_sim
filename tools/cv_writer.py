"""pd_anim から呼ばれ、標準入力の RGB フレームを OpenCV で mp4 に書く (ffmpeg のインストール不要)。

pd_anim のバイナリに埋め込まれていて、`python -c <このファイル> ...` の形で実行される。
単体でも使える:
    python tools/cv_writer.py --check                       OpenCV が使えるか確認
    python tools/cv_writer.py 1280 720 30 out.mp4 < frames.rgb

コーデックは H.264 (avc1) を試し、使えなければ MPEG-4 Part 2 (mp4v) にする。
環境変数 PD_FOURCC=mp4v などで指定もできる。
"""

import os
import sys

# 使えないコーデックを試したときのエラー表示を抑える (cv2 を読み込む前に設定)
os.environ.setdefault("OPENCV_LOG_LEVEL", "OFF")
os.environ.setdefault("OPENCV_FFMPEG_LOGLEVEL", "-8")


def fail(msg, code=1):
    print(f"cv_writer: {msg}", file=sys.stderr)
    return code


def read_exact(stream, view, n):
    """n バイト読む。読めたバイト数を返す (終わりなら 0)"""
    got = 0
    while got < n:
        k = stream.readinto(view[got:])
        if not k:
            break
        got += k
    return got


def main(argv):
    try:
        import cv2
        import numpy as np
    except ImportError as e:
        return fail(f"OpenCV か numpy を読み込めません ({e})。pip install opencv-python で入れてください", 3)

    if argv == ["--check"]:
        print(cv2.__version__)
        return 0
    if len(argv) != 4:
        return fail("使い方: cv_writer.py 幅 高さ fps 出力.mp4", 2)
    try:
        w, h, fps = int(argv[0]), int(argv[1]), float(argv[2])
    except ValueError:
        return fail(f"幅・高さ・fps が数ではありません: {argv[:3]}", 2)
    if not (0 < w <= 16384 and 0 < h <= 16384 and 0 < fps <= 1000):
        return fail(f"幅・高さ・fps の範囲が不正です: {w}x{h}, {fps}", 2)
    out = argv[3]

    wanted = os.environ.get("PD_FOURCC")
    writer, used = None, None
    for fourcc in ([wanted] if wanted else ["avc1", "mp4v"]):
        if len(fourcc) != 4:
            return fail(f"PD_FOURCC は 4 文字にしてください: {fourcc!r}", 2)
        wr = cv2.VideoWriter(out, cv2.VideoWriter_fourcc(*fourcc), fps, (w, h))
        if wr.isOpened():
            writer, used = wr, fourcc
            break
        wr.release()
    if writer is None:
        return fail(f"VideoWriter を開けません ({out})")

    n = w * h * 3
    buf = bytearray(n)
    view = memoryview(buf)
    frame = np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 3)
    stdin = sys.stdin.buffer
    frames = 0
    try:
        while True:
            got = read_exact(stdin, view, n)
            if got == 0:
                break
            if got < n:
                return fail(f"最後のフレームが途中で切れています ({got}/{n} バイト)")
            writer.write(cv2.cvtColor(frame, cv2.COLOR_RGB2BGR))  # OpenCV は BGR の順
            frames += 1
    finally:
        writer.release()
    if frames == 0:
        return fail("フレームが 1 枚も来ませんでした")
    print(f"  OpenCV ({used}): {frames} コマ", file=sys.stderr)
    return 0


if __name__ == "__main__":
    # `python -c` で実行したときは argv[0] が "-c" になる
    sys.exit(main(sys.argv[1:]))
