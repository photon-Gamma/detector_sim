//! アプリ全体のエラー型。
//!
//! どこで失敗しても `AppError` にまとめて main まで返し、
//! 日本語のメッセージと終了コードを出して終わる (パニックはしない)。

use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum AppError {
    /// コマンドライン引数の誤り
    Usage(String),
    /// 使えるフォントが見つからない / 読めない
    Font(String),
    /// ffmpeg コマンドが見つからない
    FfmpegNotFound,
    /// ffmpeg の実行に失敗した
    Ffmpeg(String),
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
            AppError::FfmpegNotFound | AppError::Ffmpeg(_) => 4,
            AppError::Io { .. } => 5,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Usage(m) => write!(f, "引数が正しくありません: {m}"),
            AppError::Font(m) => write!(f, "フォントの問題: {m}"),
            AppError::FfmpegNotFound => write!(
                f,
                "ffmpeg が見つかりません。\n  Mac:     brew install ffmpeg\n  Ubuntu:  sudo apt install ffmpeg\n  Windows: winget install ffmpeg\nインストール後、ターミナルで `ffmpeg -version` が動くことを確認してください。"
            ),
            AppError::Ffmpeg(m) => write!(f, "ffmpeg でエラー: {m}"),
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
