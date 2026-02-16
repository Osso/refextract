use crate::types::{Block, Line, PageChars, Word};

/// Group characters into words, lines, and blocks for a single page.
pub fn group_page(page: &PageChars) -> Vec<Block> {
    if page.chars.is_empty() {
        return Vec::new();
    }

    let avg_char_width = compute_avg_char_width(page);
    let dominant_font_size = compute_dominant_font_size(page);

    let words = group_chars_into_words(page, avg_char_width, dominant_font_size);
    let lines = group_words_into_lines(&words);
    let lines = split_columns(lines, page.width);
    group_lines_into_blocks(&lines)
}

fn compute_avg_char_width(page: &PageChars) -> f32 {
    let widths: Vec<f32> = page
        .chars
        .iter()
        .filter(|c| c.width > 0.0)
        .map(|c| c.width)
        .collect();
    if widths.is_empty() {
        return 5.0;
    }
    widths.iter().sum::<f32>() / widths.len() as f32
}

fn compute_dominant_font_size(page: &PageChars) -> f32 {
    let mut size_counts: Vec<(i32, usize)> = Vec::new();
    for ch in &page.chars {
        let key = (ch.font_size * 10.0) as i32;
        if let Some(entry) = size_counts.iter_mut().find(|(k, _)| *k == key) {
            entry.1 += 1;
        } else {
            size_counts.push((key, 1));
        }
    }
    size_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(key, _)| *key as f32 / 10.0)
        .unwrap_or(10.0)
}

fn is_superscript(ch_size: f32, dominant_size: f32) -> bool {
    ch_size < dominant_size * 0.75
}

struct WordAccum {
    text: String,
    x: f32,
    y: f32,
    max_x: f32,
    max_y: f32,
    font_size: f32,
    prev_right: f32,
}

impl WordAccum {
    fn new() -> Self {
        Self { text: String::new(), x: 0.0, y: 0.0, max_x: 0.0, max_y: 0.0, font_size: 0.0, prev_right: 0.0 }
    }

    fn start_char(&mut self, ch: &crate::types::PdfChar) {
        self.x = ch.x;
        self.y = ch.y;
        self.max_x = ch.x + ch.width;
        self.max_y = ch.y + ch.height;
        self.font_size = ch.font_size;
    }

    fn extend_char(&mut self, ch: &crate::types::PdfChar) {
        self.max_x = self.max_x.max(ch.x + ch.width);
        self.max_y = self.max_y.max(ch.y + ch.height);
    }

    fn flush(&mut self, words: &mut Vec<Word>, dominant_font_size: f32) {
        if self.text.is_empty() {
            return;
        }
        words.push(Word {
            text: std::mem::take(&mut self.text),
            x: self.x,
            y: self.y,
            width: self.max_x - self.x,
            height: self.max_y - self.y,
            font_size: self.font_size,
            is_superscript: is_superscript(self.font_size, dominant_font_size),
        });
    }
}

fn group_chars_into_words(
    page: &PageChars,
    avg_char_width: f32,
    dominant_font_size: f32,
) -> Vec<Word> {
    let mut words = Vec::new();
    let gap_threshold = avg_char_width * 0.3;
    let mut acc = WordAccum::new();

    for (i, ch) in page.chars.iter().enumerate() {
        let is_break = i == 0
            || ch.ch == ' '
            || (ch.x - acc.prev_right) > gap_threshold
            || (ch.y - acc.y).abs() > dominant_font_size * 0.5;

        if ch.ch == ' ' {
            acc.flush(&mut words, dominant_font_size);
            acc.prev_right = ch.x + ch.width;
            continue;
        }
        if is_break && !acc.text.is_empty() {
            acc.flush(&mut words, dominant_font_size);
        }
        if acc.text.is_empty() {
            acc.start_char(ch);
        } else {
            acc.extend_char(ch);
        }
        acc.text.push(ch.ch);
        acc.prev_right = ch.x + ch.width;
    }
    acc.flush(&mut words, dominant_font_size);
    words
}

fn group_words_into_lines(words: &[Word]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();

    for word in words {
        let merged = lines.iter_mut().rev().take(5).find(|line| {
            (word.y - line.y).abs() < word.font_size * 0.5
        });

        if let Some(line) = merged {
            line.words.push(word.clone());
            line.x_start = line.x_start.min(word.x);
            line.x_end = line.x_end.max(word.x + word.width);
        } else {
            lines.push(Line {
                words: vec![word.clone()],
                y: word.y,
                x_start: word.x,
                x_end: word.x + word.width,
                font_size: word.font_size,
            });
        }
    }

    // Sort words within each line by x position
    for line in &mut lines {
        line.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    }
    // Sort lines by y position (top to bottom = high y to low y in PDF coords)
    lines.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap());
    lines
}

/// Detect two-column layout and split lines into reading order.
///
/// If a consistent vertical gap divides the page into two columns,
/// splits each line at the boundary and returns left-column lines
/// followed by right-column lines (both top-to-bottom).
fn split_columns(lines: Vec<Line>, page_width: f32) -> Vec<Line> {
    let boundary = detect_column_boundary(&lines, page_width);
    let Some(boundary) = boundary else {
        return lines;
    };

    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();

    for line in &lines {
        let (left_words, right_words) = partition_words(&line.words, boundary);
        if !left_words.is_empty() {
            left_lines.push(make_line(left_words, line.y, line.font_size));
        }
        if !right_words.is_empty() {
            right_lines.push(make_line(right_words, line.y, line.font_size));
        }
    }

    left_lines.extend(right_lines);
    left_lines
}

/// Find the x-coordinate of a column gap, if the page is two-column.
///
/// Looks for a vertical strip in the middle 30-70% of the page where
/// no words exist, but words exist on both sides.
fn detect_column_boundary(lines: &[Line], page_width: f32) -> Option<f32> {
    // Use 200 buckets (~3pt each on letter paper) to detect narrow column
    // gaps typical of RevTeX/APS two-column layouts (~10pt gap).
    let n_buckets = 200;
    let bucket_width = page_width / n_buckets as f32;
    let mut coverage = vec![0u32; n_buckets];

    for line in lines {
        for word in &line.words {
            let start = ((word.x / page_width) * n_buckets as f32) as usize;
            let end = (((word.x + word.width) / page_width) * n_buckets as f32) as usize;
            for bucket in &mut coverage[start.min(n_buckets - 1)..=end.min(n_buckets - 1)] {
                *bucket += 1;
            }
        }
    }

    find_gap_in_coverage(&coverage, bucket_width, lines.len())
}

fn find_gap_in_coverage(
    coverage: &[u32],
    bucket_width: f32,
    num_lines: usize,
) -> Option<f32> {
    let n_buckets = coverage.len();
    // Look for empty/sparse gap in the middle 30-70% of the page
    let search_start = n_buckets * 30 / 100;
    let search_end = n_buckets * 70 / 100;
    let threshold = (num_lines as u32) / 10; // allow sparse coverage

    let mut best_gap_start = 0;
    let mut best_gap_len = 0;
    let mut gap_start = 0;
    let mut in_gap = false;

    for (i, &val) in coverage[search_start..search_end].iter().enumerate() {
        let i = i + search_start;
        if val <= threshold {
            if !in_gap {
                gap_start = i;
                in_gap = true;
            }
            let gap_len = i - gap_start + 1;
            if gap_len > best_gap_len {
                best_gap_len = gap_len;
                best_gap_start = gap_start;
            }
        } else {
            in_gap = false;
        }
    }

    // Gap must span at least 1 bucket (~3pt on letter paper).
    // Typical two-column gaps are 8-15pt (3-5 buckets at 200 resolution).
    if best_gap_len < 1 {
        return None;
    }

    let gap_center = (best_gap_start as f32 + best_gap_len as f32 / 2.0) * bucket_width;
    Some(gap_center)
}

fn partition_words(words: &[Word], boundary: f32) -> (Vec<Word>, Vec<Word>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for word in words {
        let word_center = word.x + word.width / 2.0;
        if word_center < boundary {
            left.push(word.clone());
        } else {
            right.push(word.clone());
        }
    }
    (left, right)
}

fn make_line(words: Vec<Word>, y: f32, font_size: f32) -> Line {
    let x_start = words.iter().map(|w| w.x).reduce(f32::min).unwrap();
    let x_end = words.iter().map(|w| w.x + w.width).reduce(f32::max).unwrap();
    Line { words, y, x_start, x_end, font_size }
}

fn group_lines_into_blocks(lines: &[Line]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();

    for line in lines {
        let should_merge = blocks.last().is_some_and(|block: &Block| {
            let prev_line = block.lines.last().unwrap();
            let gap = (prev_line.y - line.y).abs();
            let x_overlap = line.x_start < prev_line.x_end
                && line.x_end > prev_line.x_start;
            gap < line.font_size * 1.5 && x_overlap
        });

        if should_merge {
            let block = blocks.last_mut().unwrap();
            block.lines.push(line.clone());
            update_block_bounds(block);
        } else {
            blocks.push(Block {
                lines: vec![line.clone()],
                x: line.x_start,
                y: line.y,
                width: line.x_end - line.x_start,
                height: line.font_size,
                font_size: line.font_size,
            });
        }
    }
    blocks
}

fn update_block_bounds(block: &mut Block) {
    let min_x = block.lines.iter().map(|l| l.x_start).reduce(f32::min).unwrap();
    let max_x = block.lines.iter().map(|l| l.x_end).reduce(f32::max).unwrap();
    let max_y = block.lines.iter().map(|l| l.y).reduce(f32::max).unwrap();
    let min_y = block.lines.iter().map(|l| l.y).reduce(f32::min).unwrap();
    block.x = min_x;
    block.y = max_y;
    block.width = max_x - min_x;
    block.height = max_y - min_y + block.font_size;
}
