// フィルタチェーン組み立て（★プレビューとエクスポートで共有する単一の真実の源）
//
// ここで作った vf / pix_fmt / sws_dither を preview と export の両方で使うことで、
// 「見た目と出力が違う」事故を防ぐ。ffmpeg 引数生成をここ一箇所に閉じ込める。

use serde::{Deserialize, Serialize};

/// ディザ方式。ffmpeg の `-sws_dither` に対応する（1bit 化時の中間調表現）。
/// scale パラメータは swscale の CLI に露出していないため持たない
/// （memory `howto_video_to_tmg1` で実証済みの none/bayer/ed のみ扱う）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dither {
    None,
    Bayer,
    Ed, // 誤差拡散 (error diffusion)
}

impl Dither {
    /// `-sws_dither` に渡す値。
    pub fn sws(&self) -> &'static str {
        match self {
            Dither::None => "none",
            Dither::Bayer => "bayer",
            Dither::Ed => "ed",
        }
    }
}

/// 1 区間ぶんのモノクロ化パラメータ。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    /// eq=contrast（1.0 で無変化）。
    #[serde(default = "one")]
    pub contrast: f64,
    /// レベル絞り下限（この値未満を黒に潰す。暗部の孤立白点対策）。0 で無効側。
    #[serde(default)]
    pub level_lo: u8,
    /// レベル絞り上限（この値超を白に飛ばす。前景の欠け対策）。255 で無効側。
    #[serde(default = "u8_255")]
    pub level_hi: u8,
    pub dither: Dither,
}

fn one() -> f64 {
    1.0
}
fn u8_255() -> u8 {
    255
}

/// 組み立て済みのフィルタチェーン。
pub struct Chain {
    /// `-vf` に渡す文字列。
    pub vf: String,
    /// `-pix_fmt`（常に monob。bit=1=白で u8g2 drawXBMP の点灯規約と一致）。
    pub pix_fmt: &'static str,
    /// `-sws_dither`。
    pub sws_dither: &'static str,
}

/// 区間パラメータと出力サイズから ffmpeg のフィルタチェーンを生成する。
///
/// 生成順: scale → setsar → eq(contrast) → lutyuv(レベル絞り)。
/// 1bit 化とディザは pix_fmt/sws_dither（出力オプション側）で行う。
pub fn build_chain(seg: &Segment, width: u32, height: u32) -> Chain {
    let mut parts: Vec<String> = Vec::new();
    // 強制変形（アスペクト無視）。幅は 8 の倍数前提（monob のバイト境界）。
    parts.push(format!("scale={width}:{height}"));
    parts.push("setsar=1".to_string());

    // コントラスト（1.0 なら省略）。
    if (seg.contrast - 1.0).abs() > 1e-9 {
        parts.push(format!("eq=contrast={}", seg.contrast));
    }

    // レベル絞り（lo>0 か hi<255 のときだけ適用）。
    // val<lo→黒潰し, val>hi→白飛ばし, 間だけ [0,255] に線形リマップしてディザ対象にする。
    // 係数 k = 255/(hi-lo)。memory `howto_video_to_tmg1` 手順4の式。
    // フィルタグラフ内の if() のカンマは \, でエスケープする（filter 区切りと区別）。
    if seg.level_lo > 0 || seg.level_hi < 255 {
        let lo = seg.level_lo as f64;
        let hi = (seg.level_hi as f64).max(lo + 1.0);
        let k = 255.0 / (hi - lo);
        parts.push(format!(
            "lutyuv=y=if(lt(val\\,{lo})\\,0\\,if(gt(val\\,{hi})\\,255\\,(val-{lo})*{k}))"
        ));
    }

    Chain {
        vf: parts.join(","),
        pix_fmt: "monob",
        sws_dither: seg.dither.sws(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(contrast: f64, lo: u8, hi: u8, dither: Dither) -> Segment {
        Segment {
            id: "t".into(),
            start_sec: 0.0,
            end_sec: 1.0,
            contrast,
            level_lo: lo,
            level_hi: hi,
            dither,
        }
    }

    #[test]
    fn plain_chain_has_no_eq_or_lut() {
        let c = build_chain(&seg(1.0, 0, 255, Dither::Bayer), 128, 64);
        assert_eq!(c.vf, "scale=128:64,setsar=1");
        assert_eq!(c.pix_fmt, "monob");
        assert_eq!(c.sws_dither, "bayer");
    }

    #[test]
    fn level_squeeze_appends_lutyuv() {
        let c = build_chain(&seg(1.2, 32, 192, Dither::Ed), 128, 64);
        assert!(c.vf.contains("eq=contrast=1.2"));
        assert!(c.vf.contains("lutyuv=y=if(lt(val\\,32)"));
        assert!(c.vf.contains("(val-32)*1.59375"));
        assert_eq!(c.sws_dither, "ed");
    }
}
