//! コマンドライン引数の解析と検証。

use crate::error::{AppError, Result};
use crate::scenes;
use std::path::PathBuf;

pub const USAGE: &str = "\
使い方:
  cargo run --release                         動画をすべて作る (out/photodetector.mp4 と out/pmt.mp4)
  cargo run --release -- render pd            フォトダイオードの動画だけ作る
  cargo run --release -- render pmt           PMT の動画だけ作る
  cargo run --release -- still <シーン> <フレーム>...
                                              指定フレームを PNG で保存 (例: still pmt 120 300)
  cargo run --release -- gainstats [apd|apd70|pmt|hpd] [回数]
                                              増倍率の統計を表示 (既定: apd, 1000 回)
  cargo run --release -- help                 この説明を表示

オプション:
  --out <フォルダ>    出力先 (既定: out)

シーン:
  フォトダイオード (pd):  title, si, apd, compare, histogram, clt
  PMT (pmt):              pmt_title, pmt, pmt_hist, pmt_vs_apd, hpd, pmt_vs_hpd
  「pmt:1」のように 動画名:番号 でも指定できます
環境変数: PD_FONT=フォントのパス (日本語 TrueType フォントを指定)";

/// 統計コマンドの回数の上限 (時間とメモリの保護)
const MAX_TRIALS: usize = 2_000_000;

/// 統計を出す検出器
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Detector {
    /// 動画の APD (〈M〉 ≈ 22.5)
    Apd,
    /// 〈M〉 ≈ 70 の APD (PMT との比較用)
    Apd70,
    /// 7 段 PMT
    Pmt,
    /// HPD (打ち込み 200 + APD 70)
    Hpd,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    /// videos が空なら全部
    Render { out: PathBuf, videos: Vec<&'static str> },
    Still { out: PathBuf, scene: &'static str, frames: Vec<u32> },
    GainStats { detector: Detector, trials: usize },
    Help,
}

fn parse_trials(n: &str) -> Result<usize> {
    n.parse::<usize>().ok().filter(|&n| (2..=MAX_TRIALS).contains(&n)).ok_or_else(|| AppError::Usage(format!("回数は 2〜{MAX_TRIALS} の整数にしてください: {n}")))
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command> {
    let mut out = PathBuf::from("out");
    let mut pos = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" | "-o" => {
                let v = it.next().ok_or_else(|| AppError::Usage("--out の後にフォルダ名が必要です".into()))?;
                if v.trim().is_empty() {
                    return Err(AppError::Usage("--out のフォルダ名が空です".into()));
                }
                out = PathBuf::from(v);
            }
            "-h" | "--help" => return Ok(Command::Help),
            s if s.starts_with('-') && s.parse::<i64>().is_err() => return Err(AppError::Usage(format!("知らないオプション: {s}"))),
            _ => pos.push(a),
        }
    }
    let mut pos = pos.into_iter();
    match pos.next().as_deref() {
        None => Ok(Command::Render { out, videos: Vec::new() }),
        Some("render") => {
            let mut videos = Vec::new();
            for v in pos {
                if v.eq_ignore_ascii_case("all") {
                    videos.clear();
                    break;
                }
                let def = scenes::video(&v).ok_or_else(|| AppError::Usage(format!("動画 '{v}' はありません (pd, pmt, all)")))?;
                if !videos.contains(&def.key) {
                    videos.push(def.key);
                }
            }
            Ok(Command::Render { out, videos })
        }
        Some("help") => Ok(Command::Help),
        Some("still") => {
            let s = pos.next().ok_or_else(|| AppError::Usage("still にはシーンとフレーム番号が必要です".into()))?;
            let scene = scenes::resolve(&s).ok_or_else(|| AppError::Usage(format!("シーン '{s}' はありません (help で一覧を表示)")))?;
            let frames = pos
                .map(|f| f.parse::<u32>().map_err(|_| AppError::Usage(format!("フレーム番号 '{f}' は 0 以上の整数にしてください"))))
                .collect::<Result<Vec<_>>>()?;
            if frames.is_empty() {
                return Err(AppError::Usage("フレーム番号を1つ以上指定してください".into()));
            }
            Ok(Command::Still { out, scene, frames })
        }
        Some("gainstats") => {
            let mut detector = Detector::Apd;
            let mut trials = 1000;
            let mut rest: Vec<String> = pos.collect();
            if let Some(first) = rest.first() {
                let d = match first.to_ascii_lowercase().as_str() {
                    "apd" => Some(Detector::Apd),
                    "apd70" => Some(Detector::Apd70),
                    "pmt" => Some(Detector::Pmt),
                    "hpd" => Some(Detector::Hpd),
                    _ => None,
                };
                if let Some(d) = d {
                    detector = d;
                    rest.remove(0);
                }
            }
            match rest.as_slice() {
                [] => {}
                [n] => trials = parse_trials(n)?,
                [_, extra, ..] => return Err(AppError::Usage(format!("余分な引数: {extra}"))),
            }
            Ok(Command::GainStats { detector, trials })
        }
        Some(other) => Err(AppError::Usage(format!("知らないコマンド: {other}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn p(s: &[&str]) -> Result<Command> {
        parse(s.iter().map(|x| x.to_string()))
    }

    #[test]
    fn valid_commands() {
        assert_eq!(p(&[]).unwrap(), Command::Render { out: "out".into(), videos: vec![] });
        assert_eq!(p(&["render", "pmt"]).unwrap(), Command::Render { out: "out".into(), videos: vec!["pmt"] });
        assert_eq!(p(&["render", "pd", "all"]).unwrap(), Command::Render { out: "out".into(), videos: vec![] });
        assert_eq!(p(&["--out", "v", "render", "PD"]).unwrap(), Command::Render { out: "v".into(), videos: vec!["pd"] });
        assert_eq!(p(&["still", "apd", "1", "2"]).unwrap(), Command::Still { out: "out".into(), scene: "apd", frames: vec![1, 2] });
        assert_eq!(p(&["still", "pmt:2", "5"]).unwrap(), Command::Still { out: "out".into(), scene: "pmt_hist", frames: vec![5] });
        assert_eq!(p(&["gainstats"]).unwrap(), Command::GainStats { detector: Detector::Apd, trials: 1000 });
        assert_eq!(p(&["gainstats", "50"]).unwrap(), Command::GainStats { detector: Detector::Apd, trials: 50 });
        assert_eq!(p(&["gainstats", "pmt", "500"]).unwrap(), Command::GainStats { detector: Detector::Pmt, trials: 500 });
        assert_eq!(p(&["gainstats", "HPD"]).unwrap(), Command::GainStats { detector: Detector::Hpd, trials: 1000 });
        assert_eq!(p(&["help"]).unwrap(), Command::Help);
    }

    #[test]
    fn invalid_commands_are_errors_not_panics() {
        for bad in [
            &["still"][..],
            &["still", "apd"],
            &["still", "nope", "1"],
            &["still", "pmt:9", "1"],
            &["still", "apd", "-3"],
            &["still", "apd", "x"],
            &["gainstats", "0"],
            &["gainstats", "99999999999"],
            &["gainstats", "pmt", "10", "3"],
            &["gainstats", "laser"],
            &["render", "tv"],
            &["--out"],
            &["--out", " "],
            &["--bogus"],
            &["dance"],
        ] {
            assert!(matches!(p(bad), Err(AppError::Usage(_))), "{bad:?}");
        }
    }
}
