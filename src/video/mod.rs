//! 動画ファイルの書き出し。
//!
//! エンコーダは 2 種類から選ぶ (どちらも外部プロセスに生の RGB フレームを流す):
//! - ffmpeg: H.264 (libx264)。シーンごとの .mp4 を作り、再エンコードせずにつなぐ
//! - OpenCV: Python の cv2.VideoWriter (`tools/cv_writer.py` を埋め込んで実行)。
//!   ffmpeg のインストールが不要。つなぐ機能がないので、1 本の動画を 1 回で書く
//!
//! - `Backend::select`: 使えるエンコーダを選ぶ (重い計算を始める前に)
//! - `Encoder`:         フレームを流し込んで .mp4 にする
//! - `concat`:          シーンごとの .mp4 をつなぐ (ffmpeg のみ)
//! - `write_png`:       1 フレームを PNG で保存 (外部ツール不要)
//!
//! 途中で失敗・中断しても、書きかけのファイルが完成品の名前で残らないようにしている。

mod png;

use crate::error::{AppError, Result};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

/// OpenCV で書くための Python スクリプト (バイナリに埋め込む)
const CV_WRITER: &str = include_str!("../../tools/cv_writer.py");

#[derive(Clone, Copy, Debug)]
pub struct VideoSpec {
    pub width: usize,
    pub height: usize,
    pub fps: u32,
}

impl VideoSpec {
    fn frame_bytes(&self) -> usize {
        self.width * self.height * 3
    }
    fn size_arg(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

/// コマンドラインで指定するエンコーダ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncoderChoice {
    /// ffmpeg があれば ffmpeg、なければ OpenCV
    Auto,
    Ffmpeg,
    OpenCv,
}

/// 実際に使うエンコーダ
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backend {
    Ffmpeg,
    /// python: 実行する Python のコマンド名
    OpenCv { python: String },
}

impl Backend {
    /// 使えるエンコーダを探す。見つからなければ、入れ方を添えたエラー
    pub fn select(choice: EncoderChoice) -> Result<Backend> {
        let ff = || ffmpeg_works().then_some(Backend::Ffmpeg);
        let cv = || find_python_with_opencv().map(|python| Backend::OpenCv { python });
        let found = match choice {
            EncoderChoice::Auto => ff().or_else(cv),
            EncoderChoice::Ffmpeg => ff(),
            EncoderChoice::OpenCv => cv(),
        };
        found.ok_or(AppError::NoEncoder(choice))
    }

    /// シーンごとのファイルを再エンコードせずにつなげるか
    pub fn can_concat(&self) -> bool {
        matches!(self, Backend::Ffmpeg)
    }

    pub fn describe(&self) -> String {
        match self {
            Backend::Ffmpeg => "ffmpeg (H.264)".into(),
            Backend::OpenCv { python } => format!("OpenCV ({python})"),
        }
    }

    fn command(&self, spec: VideoSpec, tmp: &Path) -> Command {
        match self {
            Backend::Ffmpeg => {
                let mut c = ffmpeg();
                c.args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &spec.size_arg(), "-r", &spec.fps.to_string(), "-i", "-"])
                    .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"])
                    .arg(tmp);
                c
            }
            Backend::OpenCv { python } => {
                let mut c = Command::new(python);
                c.arg("-c").arg(CV_WRITER).arg(spec.width.to_string()).arg(spec.height.to_string()).arg(spec.fps.to_string()).arg(tmp);
                c
            }
        }
    }
}

fn ffmpeg() -> Command {
    let mut c = Command::new("ffmpeg");
    c.args(["-hide_banner", "-loglevel", "error", "-y"]);
    c
}

fn ffmpeg_works() -> bool {
    Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// OpenCV が import できる Python を探す。PD_PYTHON が設定されていればそれだけを試す
fn find_python_with_opencv() -> Option<String> {
    let candidates: Vec<String> = match std::env::var("PD_PYTHON") {
        Ok(p) if !p.trim().is_empty() => vec![p],
        // py (Python ランチャー) は Windows だけ。ほかの OS では別のプログラムの名前かもしれない
        _ => ["python3", "python"].iter().chain(if cfg!(windows) { &["py"][..] } else { &[][..] }).map(|s| s.to_string()).collect(),
    };
    candidates.into_iter().find(|py| {
        Command::new(py).arg("-c").arg(CV_WRITER).arg("--check").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    })
}

/// 一時ファイル名: scene1.mp4 → scene1.part.mp4 (拡張子 .mp4 は残す: エンコーダが形式を判断するため)
fn temp_path(dst: &Path) -> PathBuf {
    let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    dst.with_file_name(format!("{stem}.part.mp4"))
}

fn spawn_error(e: std::io::Error) -> AppError {
    AppError::Encoder(format!("エンコーダを起動できません: {e}"))
}

/// フレームを1枚ずつ受け取って .mp4 に書き出す
pub struct Encoder {
    spec: VideoSpec,
    child: Child,
    stdin: Option<ChildStdin>,
    tmp: PathBuf,
    dst: PathBuf,
    finished: bool,
}

impl Encoder {
    pub fn create(backend: &Backend, spec: VideoSpec, dst: &Path) -> Result<Encoder> {
        let tmp = temp_path(dst);
        let mut child = backend.command(spec, &tmp).stdin(Stdio::piped()).spawn().map_err(spawn_error)?;
        let stdin = child.stdin.take();
        Ok(Encoder { spec, child, stdin, tmp, dst: dst.to_path_buf(), finished: false })
    }

    pub fn write_frame(&mut self, rgb: &[u8]) -> Result<()> {
        if rgb.len() != self.spec.frame_bytes() {
            return Err(AppError::Encoder(format!("フレームの大きさが違います ({} バイト, 期待値 {})", rgb.len(), self.spec.frame_bytes())));
        }
        let stdin = self.stdin.as_mut().ok_or_else(|| AppError::Encoder("入力がすでに閉じています".into()))?;
        stdin.write_all(rgb).map_err(|e| {
            if e.kind() == ErrorKind::BrokenPipe {
                AppError::Encoder("エンコーダが途中で終了しました (上に表示されたメッセージを確認してください)".into())
            } else {
                AppError::io("エンコーダへのフレーム送信")(e)
            }
        })
    }

    /// 入力を閉じてエンコーダの終了を待ち、成功したら完成品の名前に変える
    pub fn finish(mut self) -> Result<PathBuf> {
        drop(self.stdin.take());
        let st = self.child.wait().map_err(AppError::io("エンコーダの終了待ち"))?;
        if !st.success() {
            return Err(AppError::Encoder(format!("エンコードに失敗しました ({st})")));
        }
        std::fs::rename(&self.tmp, &self.dst).map_err(AppError::io_path("出力ファイルの名前変更", &self.dst))?;
        self.finished = true;
        Ok(self.dst.clone())
    }
}

impl Drop for Encoder {
    /// エラーなどで finish されなかったら、エンコーダを止めて書きかけを消す
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.tmp);
    }
}

/// 同じ設定で作った .mp4 を順につなぐ (ffmpeg、再エンコードなし)
/// 連結リストはシーンのファイルと同じフォルダに置く (リストの中はファイル名だけで書くため)。
pub fn concat(parts: &[PathBuf], dst: &Path) -> Result<()> {
    let first = parts.first().ok_or_else(|| AppError::Encoder("つなぐシーンがありません".into()))?;
    let dir = first.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if parts.iter().any(|p| p.parent().filter(|q| !q.as_os_str().is_empty()).unwrap_or(Path::new(".")) != dir) {
        return Err(AppError::Encoder("つなぐシーンは同じフォルダに置いてください".into()));
    }
    // 利用者のファイルを上書きしないよう、出力名から作った専用の名前にし、終わったら消す
    let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    let list_path = dir.join(format!("{stem}.concat.txt"));
    let mut list = String::new();
    for p in parts {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| AppError::Encoder(format!("ファイル名が不正です: {}", p.display())))?;
        // concat 形式では ' を '\'' と書く
        list.push_str(&format!("file '{}'\n", name.replace('\'', "'\\''")));
    }
    std::fs::write(&list_path, list).map_err(AppError::io_path("連結リストの書き込み", &list_path))?;
    let tmp = temp_path(dst);
    let st = ffmpeg().args(["-f", "concat", "-i"]).arg(&list_path).args(["-c", "copy", "-movflags", "+faststart"]).arg(&tmp).status();
    let _ = std::fs::remove_file(&list_path);
    let st = st.map_err(spawn_error)?;
    if !st.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Encoder(format!("動画の連結に失敗しました ({st})")));
    }
    std::fs::rename(&tmp, dst).map_err(AppError::io_path("出力ファイルの名前変更", dst))
}

/// 1フレームを PNG で保存 (外部ツール不要)
pub fn write_png(spec: VideoSpec, rgb: &[u8], dst: &Path) -> Result<()> {
    let bytes = png::encode_rgb(spec.width, spec.height, rgb).ok_or_else(|| AppError::Encoder("フレームの大きさが違います".into()))?;
    std::fs::write(dst, bytes).map_err(AppError::io_path("PNG の保存", dst))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_name_keeps_mp4_extension() {
        assert_eq!(temp_path(Path::new("out/scene3.mp4")), PathBuf::from("out/scene3.part.mp4"));
        assert_eq!(temp_path(Path::new("x")), PathBuf::from("x.part.mp4"));
    }

    #[test]
    fn only_ffmpeg_can_concat() {
        assert!(Backend::Ffmpeg.can_concat());
        assert!(!Backend::OpenCv { python: "python3".into() }.can_concat());
    }
}
