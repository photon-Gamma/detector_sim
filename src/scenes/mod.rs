//! 動画のシーン。動画ごとにフォルダを分け、1シーン = 1ファイル。
//!
//! photodetector.mp4 (`pd/`)
//! | 名前        | 内容                                   |
//! |-------------|----------------------------------------|
//! | `title`     | タイトル                                |
//! | `si`        | Si PIN フォトダイオード                  |
//! | `apd`       | APD のなだれ増倍                         |
//! | `compare`   | 同じ光での出力比較と増倍のゆらぎ           |
//! | `histogram` | 1光子ごとの利得 M の分布                  |
//! | `clt`       | 平均利得の分布 (中心極限定理)              |
//!
//! pmt.mp4 (`pmt/`)
//! | 名前          | 内容                                  |
//! |---------------|---------------------------------------|
//! | `pmt_title`   | タイトル                               |
//! | `pmt`         | 7段のダイノードでの増倍                  |
//! | `pmt_hist`    | PMT の増倍率の分布                      |
//! | `pmt_vs_apd`  | PMT と APD (〈M〉≈70) の増倍ゆらぎの比較  |
//! | `hpd`         | HPD: 電子打ち込み (×200) + APD (×70)     |
//! | `pmt_vs_hpd`  | PMT と HPD の増倍ゆらぎと F の比較         |

mod pd;
mod pmt;

use crate::render::Canvas;

pub trait Scene {
    /// 進捗表示用の名前
    fn title(&self) -> &'static str;
    /// フレーム数 (1以上)
    fn frames(&self) -> u32;
    /// フレーム f を描く。f は 0 から順に1ずつ呼ばれる前提 (中でシミュレーションを進めるため)
    fn render(&mut self, c: &mut Canvas, f: u32);
}

/// 1本の動画
pub struct VideoDef {
    /// コマンドラインで使う名前
    pub key: &'static str,
    /// 出力ファイル名
    pub file: &'static str,
    /// シーン名 (再生順)
    pub scenes: &'static [&'static str],
}

pub const VIDEOS: [VideoDef; 2] = [
    VideoDef { key: "pd", file: "photodetector.mp4", scenes: &["title", "si", "apd", "compare", "histogram", "clt"] },
    VideoDef { key: "pmt", file: "pmt.mp4", scenes: &["pmt_title", "pmt", "pmt_hist", "pmt_vs_apd", "hpd", "pmt_vs_hpd"] },
];

pub fn video(key: &str) -> Option<&'static VideoDef> {
    VIDEOS.iter().find(|v| v.key.eq_ignore_ascii_case(key))
}

/// シーンを名前で作る。重い事前計算があるシーンもあるので、必要なときに1つずつ作る。
pub fn create(name: &str) -> Option<Box<dyn Scene>> {
    Some(match name {
        "title" => Box::new(pd::title::Title),
        "si" => Box::new(pd::si_pin::SiPinScene::new()),
        "apd" => Box::new(pd::apd::ApdScene::new()),
        "compare" => Box::new(pd::compare::CompareScene::new()),
        "histogram" => Box::new(pd::histogram::HistogramScene::new()),
        "clt" => Box::new(pd::clt::CltScene::new()),
        "pmt_title" => Box::new(pmt::title::PmtTitle),
        "pmt" => Box::new(pmt::cascade::CascadeScene::new()),
        "pmt_hist" => Box::new(pmt::histogram::PmtHistogramScene::new()),
        "pmt_vs_apd" => Box::new(pmt::compare::PmtVsApdScene::new()),
        "hpd" => Box::new(pmt::hpd::HpdScene::new()),
        "pmt_vs_hpd" => Box::new(pmt::compare_hpd::PmtVsHpdScene::new()),
        _ => return None,
    })
}

/// "apd" のような名前か、"pmt:1" (動画名:番号) をシーン名に変換
pub fn resolve(s: &str) -> Option<&'static str> {
    if let Some((v, i)) = s.split_once(':') {
        let i: usize = i.parse().ok()?;
        return video(v)?.scenes.get(i).copied();
    }
    VIDEOS.iter().flat_map(|v| v.scenes.iter()).find(|n| n.eq_ignore_ascii_case(s)).copied()
}

/// シーンの最初と最後を暗転させる
fn edge_fade(c: &mut Canvas, f: u32, n: u32) {
    const FADE: f32 = 12.0;
    let to_end = n.saturating_sub(1).saturating_sub(f) as f32;
    c.fade((f as f32 / FADE).min(to_end / FADE).min(1.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve() {
        assert_eq!(resolve("apd"), Some("apd"));
        assert_eq!(resolve("CLT"), Some("clt"));
        assert_eq!(resolve("pd:4"), Some("histogram"));
        assert_eq!(resolve("pmt:1"), Some("pmt"));
        assert_eq!(resolve("pmt:9"), None);
        assert_eq!(resolve("x:1"), None);
        assert_eq!(resolve("4"), None);
        assert_eq!(resolve(""), None);
    }

    #[test]
    fn every_listed_scene_exists() {
        // 一覧に載っているのに作れないシーンが無いか (重い事前計算は避けて名前の対応だけ確認)
        for v in &VIDEOS {
            for n in v.scenes {
                assert!(matches!(*n, "title" | "si" | "apd" | "compare" | "histogram" | "clt" | "pmt_title" | "pmt" | "pmt_hist" | "pmt_vs_apd" | "hpd" | "pmt_vs_hpd"), "{n}");
            }
        }
        assert!(create("nope").is_none());
    }
}
