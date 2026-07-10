use crate::types::SubtitleItem;

fn parse_timestamp(s: &str) -> Option<f64> {
    // "HH:MM:SS,mmm"（也容忍 '.' 作毫秒分隔）
    let s = s.trim().replace('.', ",");
    let (hms, ms) = s.split_once(',')?;
    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].trim().parse().ok()?;
    let m: f64 = parts[1].trim().parse().ok()?;
    let sec: f64 = parts[2].trim().parse().ok()?;
    let ms: f64 = ms.trim().parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + sec + ms / 1000.0)
}

fn contains_hangul(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}'
        )
    })
}

/// 解析 SRT 內容。雙行文字視為本工具的雙語格式（第一行中文、其餘韓文）；
/// 單行文字依是否含韓文字母判斷放入 ko 或 zh
pub fn parse(content: &str) -> Vec<SubtitleItem> {
    let normalized = content.replace("\r\n", "\n").replace('\u{FEFF}', "");
    let mut items: Vec<SubtitleItem> = Vec::new();

    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let Some(time_pos) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let (start_raw, end_raw) = match lines[time_pos].split_once("-->") {
            Some(pair) => pair,
            None => continue,
        };
        let (Some(start), Some(end)) = (parse_timestamp(start_raw), parse_timestamp(end_raw))
        else {
            continue;
        };

        let text_lines = &lines[time_pos + 1..];
        if text_lines.is_empty() {
            continue;
        }
        let (zh, ko) = if text_lines.len() >= 2 {
            (text_lines[0].to_string(), text_lines[1..].join(" "))
        } else if contains_hangul(text_lines[0]) {
            (String::new(), text_lines[0].to_string())
        } else {
            (text_lines[0].to_string(), String::new())
        };

        items.push(SubtitleItem {
            index: items.len(),
            start,
            end,
            ko,
            zh,
        });
    }
    items
}

fn format_timestamp(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_secs = total_ms / 1000;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(start: f64, end: f64, ko: &str, zh: &str) -> SubtitleItem {
        SubtitleItem {
            index: 0,
            start,
            end,
            ko: ko.into(),
            zh: zh.into(),
        }
    }

    #[test]
    fn bilingual_roundtrip() {
        let src = vec![
            item(0.0, 2.5, "안녕하세요", "你好"),
            item(2.5, 5.0, "만나서 반갑습니다", "很高興見到你"),
        ];
        let parsed = parse(&build(&src, true));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].zh, "你好");
        assert_eq!(parsed[0].ko, "안녕하세요");
        assert!((parsed[1].start - 2.5).abs() < 0.001);
        assert!((parsed[1].end - 5.0).abs() < 0.001);
    }

    #[test]
    fn single_line_zh_roundtrip() {
        let parsed = parse(&build(&[item(1.0, 3.0, "안녕", "你好")], false));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].zh, "你好");
        assert_eq!(parsed[0].ko, "");
    }

    #[test]
    fn single_line_hangul_goes_to_ko() {
        let parsed = parse("1\n00:00:00,000 --> 00:00:02,000\n안녕하세요\n\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].ko, "안녕하세요");
        assert_eq!(parsed[0].zh, "");
    }

    #[test]
    fn tolerates_crlf_and_missing_counter() {
        let parsed = parse("00:00:01,500 --> 00:00:03,000\r\n測試\r\n\r\n");
        assert_eq!(parsed.len(), 1);
        assert!((parsed[0].start - 1.5).abs() < 0.001);
        assert_eq!(parsed[0].zh, "測試");
    }
}

/// 組出 SRT 內容。`bilingual` 為 true 時每條字幕輸出「中文 + 韓文」雙行
pub fn build(subtitles: &[SubtitleItem], bilingual: bool) -> String {
    let mut out = String::new();
    for (i, item) in subtitles.iter().enumerate() {
        let text = if bilingual && !item.ko.trim().is_empty() {
            format!("{}\n{}", item.zh.trim(), item.ko.trim())
        } else if item.zh.trim().is_empty() {
            item.ko.trim().to_string()
        } else {
            item.zh.trim().to_string()
        };
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_timestamp(item.start),
            format_timestamp(item.end),
            text
        ));
    }
    out
}
