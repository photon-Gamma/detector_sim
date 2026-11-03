//! 動画ファイルの書き出し (ffmpeg を外部コマンドとして使う)。
//!
//! - `ensure_ffmpeg`: 最初に ffmpeg があるか確認
//! - `Encoder`:       生の RGB フレームを流し込んで .mp4 にする
//! - `concat`:        シーンごとの .mp4 を再エンコードせずにつなぐ
//! - `write_png`:     1フレームを PNG で保存 (確認用)
//!
//! 途中で失敗・中断しても、書きかけのファイルが完成品の名前で残らないようにしている。

use crate::error::{AppError, Result};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

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

fn ffmpeg() -> Command {
    let mut c = Command::new("ffmpeg");
    c.args(["-hide_banner", "-loglevel", "error", "-y"]);
    c
}

fn spawn_error(e: std::io::Error) -> AppError {
    if e.kind() == ErrorKind::NotFound {
        AppError::FfmpegNotFound
    } else {
        AppError::Ffmpeg(format!("起動できません: {e}"))
    }
}

/// ffmpeg が実行できるか確かめる (重い計算を始める前に)
pub fn ensure_ffmpeg() -> Result<()> {
    let st = Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(spawn_error)?;
    if st.success() {
        Ok(())
    } else {
        Err(AppError::Ffmpeg("`ffmpeg -version` が失敗しました".into()))
    }
}

/// 一時ファイル名: scene1.mp4 → scene1.part.mp4 (拡張子 .mp4 は残す: ffmpeg が形式を判断するため)
fn temp_path(dst: &Path) -> PathBuf {
    let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    dst.with_file_name(format!("{stem}.part.mp4"))
}

/// フレームを1枚ずつ受け取って H.264 の .mp4 に書き出す
pub struct Encoder {
    spec: VideoSpec,
    child: Child,
    stdin: Option<ChildStdin>,
    tmp: PathBuf,
    dst: PathBuf,
    finished: bool,
}

impl Encoder {
    pub fn create(spec: VideoSpec, dst: &Path) -> Result<Encoder> {
        let tmp = temp_path(dst);
        let mut child = ffmpeg()
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &spec.size_arg(), "-r", &spec.fps.to_string(), "-i", "-"])
            .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"])
            .arg(&tmp)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(spawn_error)?;
        let stdin = child.stdin.take();
        Ok(Encoder { spec, child, stdin, tmp, dst: dst.to_path_buf(), finished: false })
    }

    pub fn write_frame(&mut self, rgb: &[u8]) -> Result<()> {
        if rgb.len() != self.spec.frame_bytes() {
            return Err(AppError::Ffmpeg(format!("フレームの大きさが違います ({} バイト, 期待値 {})", rgb.len(), self.spec.frame_bytes())));
        }
        let stdin = self.stdin.as_mut().ok_or_else(|| AppError::Ffmpeg("入力がすでに閉じています".into()))?;
        stdin.write_all(rgb).map_err(|e| {
            if e.kind() == ErrorKind::BrokenPipe {
                AppError::Ffmpeg("ffmpeg が途中で終了しました (上に表示された ffmpeg のメッセージを確認してください)".into())
            } else {
                AppError::io("ffmpeg へのフレーム送信")(e)
            }
        })
    }

    /// 入力を閉じて ffmpeg の終了を待ち、成功したら完成品の名前に変える
    pub fn finish(mut self) -> Result<PathBuf> {
        drop(self.stdin.take());
        let st = self.child.wait().map_err(AppError::io("ffmpeg の終了待ち"))?;
        if !st.success() {
            return Err(AppError::Ffmpeg(format!("エンコードに失敗しました ({st})")));
        }
        std::fs::rename(&self.tmp, &self.dst).map_err(AppError::io_path("出力ファイルの名前変更", &self.dst))?;
        self.finished = true;
        Ok(self.dst.clone())
    }
}

impl Drop for Encoder {
    /// エラーなどで finish されなかったら、ffmpeg を止めて書きかけを消す
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

/// 同じ設定で作った .mp4 を順につなぐ (再エンコードなし)
pub fn concat(parts: &[PathBuf], dst: &Path) -> Result<()> {
    let dir = dst.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let list_path = dir.join("list.txt");
    let mut list = String::new();
    for p in parts {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| AppError::Ffmpeg(format!("ファイル名が不正です: {}", p.display())))?;
        // concat 形式では ' を '\'' と書く
        list.push_str(&format!("file '{}'\n", name.replace('\'', "'\\''")));
    }
    std::fs::write(&list_path, list).map_err(AppError::io_path("連結リストの書き込み", &list_path))?;
    let tmp = temp_path(dst);
    let st = ffmpeg()
        .args(["-f", "concat", "-i"])
        .arg(&list_path)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(&tmp)
        .status()
        .map_err(spawn_error)?;
    if !st.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Ffmpeg(format!("動画の連結に失敗しました ({st})")));
    }
    std::fs::rename(&tmp, dst).map_err(AppError::io_path("出力ファイルの名前変更", dst))
}

/// 1フレームを PNG で保存
pub fn write_png(spec: VideoSpec, rgb: &[u8], dst: &Path) -> Result<()> {
    if rgb.len() != spec.frame_bytes() {
        return Err(AppError::Ffmpeg("フレームの大きさが違います".into()));
    }
    let mut child = ffmpeg()
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &spec.size_arg(), "-i", "-", "-frames:v", "1"])
        .arg(dst)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;
    let write_result = match child.stdin.take() {
        Some(mut s) => s.write_all(rgb),
        None => Ok(()),
    };
    let st = child.wait().map_err(AppError::io("ffmpeg の終了待ち"))?;
    write_result.map_err(AppError::io("ffmpeg への画像送信"))?;
    if st.success() {
        Ok(())
    } else {
        Err(AppError::Ffmpeg(format!("PNG の保存に失敗しました: {}", dst.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_name_keeps_mp4_extension() {
        assert_eq!(temp_path(Path::new("out/scene3.mp4")), PathBuf::from("out/scene3.part.mp4"));
        assert_eq!(temp_path(Path::new("x")), PathBuf::from("x.part.mp4"));
    }
}
