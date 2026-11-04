//! アプリ全体のエラー型。
//!
//! どこで失敗しても `AppError` にまとめて main まで返し、
//! 日本語のメッセージと終了コードを出して終わる (パニックはしない)。

use crate::video::EncoderChoice;
use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum AppError {
    /// コマンドライン引数の誤り
    Usage(String),
    /// 使えるフォントが見つからない / 読めない
    Font(String),
    /// 使える動画エンコーダ (ffmpeg / Python + OpenCV) が見つからない
    NoEncoder(EncoderChoice),
    /// 動画エンコーダの実行に失敗した
    Encoder(String),
    /// ファイル入出力の失敗
    Io { context: String, source: io::Error },
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    /// ファイル操作のエラーに「何をしようとしていたか」を付ける
    pub fn io(context: impl Into<String>) -> impl FnOnce(io::Error) -> AppError {
        let context = context.into();
        move |source| AppError::Io { context, source }
    }

    pub fn io_path(what: &str, path: &Path) -> impl FnOnce(io::Error) -> AppError {
        Self::io(format!("{} ({})", what, path.display()))
    }

    /// プロセスの終了コード
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::Usage(_) => 2,
            AppError::Font(_) => 3,
            AppError::NoEncoder(_) | AppError::Encoder(_) => 4,
            AppError::Io { .. } => 5,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Usage(m) => write!(f, "引数が正しくありません: {m}"),
            AppError::Font(m) => write!(f, "フォントの問題: {m}"),
            AppError::NoEncoder(choice) => {
                const FF: &str = "ffmpeg を入れる:\n  Mac:     brew install ffmpeg\n  Windows: winget install --id Gyan.FFmpeg -e\n  Ubuntu:  sudo apt install ffmpeg";
                const CV: &str = "OpenCV を使う:\n  pip install opencv-python\n  (Python のコマンド名が python3 / python / py 以外なら PD_PYTHON=コマンド名 で指定)";
                match choice {
                    EncoderChoice::Ffmpeg => write!(f, "ffmpeg が見つかりません。\n{FF}"),
                    EncoderChoice::OpenCv => write!(f, "OpenCV が使える Python が見つかりません。\n{CV}"),
                    EncoderChoice::Auto => write!(f, "動画のエンコーダが見つかりません。どちらかを用意してください。\n{FF}\n{CV}"),
                }
            }
            AppError::Encoder(m) => write!(f, "動画の書き出しでエラー: {m}"),
            AppError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
