//! 光検出器 (Si PIN フォトダイオード / APD / 光電子増倍管) の動作アニメーション
//!
//! モジュール構成:
//! - `cli`     コマンドライン引数
//! - `error`   エラー型 (パニックせず、ここに集めて終了コードで返す)
//! - `physics` 物理モデル: デバイス構造・粒子シミュレーション・統計
//! - `render`  描画: ラスタライザ・フォント・キャンバス・画面部品
//! - `scenes`  動画の各シーン (動画ごとにフォルダ、1シーン1ファイル)
//! - `video`   ffmpeg による動画出力

mod cli;
mod error;
mod physics;
mod render;
mod scenes;
mod video;

use cli::Command;
use error::{AppError, Result};
use render::theme::{BG, FPS, HEIGHT, WIDTH};
use render::Canvas;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use video::VideoSpec;

const SPEC: VideoSpec = VideoSpec { width: WIDTH, height: HEIGHT, fps: FPS };

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {e}");
            if matches!(e, AppError::Usage(_)) {
                eprintln!("\n{}", cli::USAGE);
            }
            ExitCode::from(e.exit_code())
        }
    }
}

fn run() -> Result<()> {
    match cli::parse(std::env::args().skip(1))? {
        Command::Help => {
            println!("{}", cli::USAGE);
            Ok(())
        }
        Command::GainStats { detector, trials } => gain_stats(detector, trials),
        Command::Render { out, videos } => {
            video::ensure_ffmpeg()?;
            create_dir(&out)?;
            let mut canvas = new_canvas()?;
            for v in &scenes::VIDEOS {
                if videos.is_empty() || videos.contains(&v.key) {
                    render_video(&mut canvas, &out, v)?;
                }
            }
            Ok(())
        }
        Command::Still { out, scene, frames } => render_stills(&out, scene, &frames),
    }
}

fn new_canvas() -> Result<Canvas> {
    let font = render::font::find_japanese_font()?;
    Ok(Canvas::new(WIDTH, HEIGHT, BG, font))
}

fn create_dir(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out).map_err(AppError::io_path("出力フォルダを作れません", out))
}

/// 1本の動画を作る: シーンごとに .mp4 にして、最後につなぐ
fn render_video(canvas: &mut Canvas, out: &Path, def: &scenes::VideoDef) -> Result<()> {
    let t0 = Instant::now();
    eprintln!("== {} ==", def.file);
    let mut parts = Vec::new();
    let mut total_frames = 0u32;
    for (i, name) in def.scenes.iter().enumerate() {
        let Some(mut scene) = scenes::create(name) else { continue };
        let n = scene.frames();
        total_frames += n;
        eprintln!("[{}/{}] {} ({} フレーム)", i + 1, def.scenes.len(), scene.title(), n);
        let mut enc = video::Encoder::create(SPEC, &out.join(format!("{}_{}.mp4", def.key, name)))?;
        for f in 0..n {
            scene.render(canvas, f);
            enc.write_frame(canvas.pixels())?;
        }
        parts.push(enc.finish()?);
    }
    let dst = out.join(def.file);
    video::concat(&parts, &dst)?;
    println!("完成: {} ({} 秒の動画, 処理 {:.1} 秒)", dst.display(), total_frames / FPS, t0.elapsed().as_secs_f32());
    Ok(())
}

/// 指定シーンの指定フレームを PNG に保存する
fn render_stills(out: &Path, name: &str, frames: &[u32]) -> Result<()> {
    video::ensure_ffmpeg()?;
    let mut scene = scenes::create(name).ok_or_else(|| AppError::Usage(format!("シーン {name} はありません")))?;
    let n = scene.frames();
    if let Some(bad) = frames.iter().find(|&&f| f >= n) {
        return Err(AppError::Usage(format!("フレーム {bad} は範囲外です (このシーンは 0〜{})", n - 1)));
    }
    create_dir(out)?;
    let mut canvas = new_canvas()?;
    let last = frames.iter().copied().max().unwrap_or(0);
    // シミュレーションは順に進める必要があるので、0 から last まで描く
    for f in 0..=last {
        scene.render(&mut canvas, f);
        if frames.contains(&f) {
            let p = out.join(format!("still_{name}_{f}.png"));
            video::write_png(SPEC, canvas.pixels(), &p)?;
            println!("{}", p.display());
        }
    }
    Ok(())
}

/// 増倍率の統計を表示
fn gain_stats(detector: cli::Detector, trials: usize) -> Result<()> {
    use physics::pmt::{precompute_pmt_gains, PmtSpec};
    use physics::stats::{gain_stats, precompute_gains_for};
    let t0 = Instant::now();
    let (label, v) = match detector {
        cli::Detector::Apd => ("APD (動画の設定)", precompute_gains_for(&physics::DeviceSpec::apd(), trials, 0xC0FFEE)),
        cli::Detector::Apd70 => ("APD (〈M〉≈70)", precompute_gains_for(&physics::DeviceSpec::apd_with_alpha(physics::device::APD70_ALPHA_E), trials, 0xA70)),
        cli::Detector::Pmt => ("PMT (7 段, 各段 4)", precompute_pmt_gains(&PmtSpec::standard(), trials, 0x9417)),
        cli::Detector::Hpd => ("HPD (打ち込み 200 × APD 70)", physics::hpd::precompute_hpd_gains(trials, 0x4BD)),
    };
    let s = gain_stats(&v).ok_or_else(|| AppError::Usage("統計を出すにはデータが足りません".into()))?;
    println!("{label}: 試行 {} 回 ({:.2} 秒)", s.n, t0.elapsed().as_secs_f32());
    println!("  平均 〈M〉      = {:.2}", s.mean);
    println!("  標準偏差 σ     = {:.2}", s.sd);
    println!("  過剰雑音係数 F = {:.3}", s.excess_noise);
    println!("  中央値 = {},  最大 = {}", s.median, s.max);
    match detector {
        cli::Detector::Pmt => println!("  理論値 F = {:.4} (各段ポアソン)", PmtSpec::standard().excess_noise_theory()),
        cli::Detector::Hpd => {
            if let (Some(a), Some(t)) = (physics::hpd::apd70_stats(), physics::hpd::excess_noise_theory()) {
                println!("  2 段目の APD: 〈M〉 = {:.2}, F = {:.3}", a.mean, a.excess_noise);
                println!("  理論値 F = F1 + (F2 − 1)/G1 = {:.4} + ({:.3} − 1)/{} = {:.4}", physics::hpd::eb_excess_noise(), a.excess_noise, physics::hpd::EB_GAIN, t);
            }
        }
        _ => {}
    }
    Ok(())
}
