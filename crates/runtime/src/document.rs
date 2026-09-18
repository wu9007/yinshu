use crate::protocol::EffectivePaper;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, ImageReader, Rgb};
use printpdf::{
    ops::PdfFontHandle, BuiltinFont, Color, Line, LinePoint, Mm, Op, PaintMode, PdfDocument,
    PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing, Pt, RawImage, RawImageData,
    RawImageFormat, Rgb as PdfRgb, TextItem, WindingOrder, XObjectTransform,
};
use std::{fs, io::Read, path::Path};
use thiserror::Error;

const HEADER_SIZE: usize = 8;
const DEFAULT_DPI: f32 = 203.0;

/// Agent 在打印前可以校验或规范化的文档格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Png,
    Jpeg,
}

/// 用于把图片放入目标纸张尺寸内的矩形区域。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 检测或规范化可打印文档时返回的错误。
#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("unsupported document format")]
    UnsupportedFormat,
    #[error("invalid image dimensions")]
    InvalidImageDimensions,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
}

/// 文档检测和 PDF 生成的统一返回结果。
pub type DocumentResult<T> = Result<T, DocumentError>;

/// 根据文件开头字节检测文档格式。
pub fn detect_format_from_bytes(bytes: &[u8]) -> Option<DocumentFormat> {
    if bytes.starts_with(b"%PDF-") {
        Some(DocumentFormat::Pdf)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(DocumentFormat::Png)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(DocumentFormat::Jpeg)
    } else {
        None
    }
}

/// 只读取文件签名头来检测文件格式。
pub fn detect_format(path: &Path) -> DocumentResult<Option<DocumentFormat>> {
    let mut header = [0_u8; HEADER_SIZE];
    let bytes_read = fs::File::open(path)?.read(&mut header)?;

    Ok(detect_format_from_bytes(&header[..bytes_read]))
}

/// 计算图片在页面中居中且完整包含的排版矩形。
pub fn fit_contain(
    image_width: f64,
    image_height: f64,
    page_width: f64,
    page_height: f64,
) -> DocumentResult<FitRect> {
    if !is_positive_finite(image_width)
        || !is_positive_finite(image_height)
        || !is_positive_finite(page_width)
        || !is_positive_finite(page_height)
    {
        return Err(DocumentError::InvalidImageDimensions);
    }

    let scale = (page_width / image_width).min(page_height / image_height);
    let width = image_width * scale;
    let height = image_height * scale;

    Ok(FitRect {
        x: (page_width - width) / 2.0,
        y: (page_height - height) / 2.0,
        width,
        height,
    })
}

/// 把 PNG 或 JPEG 转成匹配目标纸张的一页 PDF。
pub fn image_to_pdf(
    image_path: &Path,
    paper: &EffectivePaper,
    output_path: &Path,
) -> DocumentResult<()> {
    let image_format = match detect_format(image_path)? {
        Some(DocumentFormat::Png) => ImageFormat::Png,
        Some(DocumentFormat::Jpeg) => ImageFormat::Jpeg,
        _ => return Err(DocumentError::UnsupportedFormat),
    };

    paper
        .validate()
        .map_err(|_| DocumentError::InvalidImageDimensions)?;

    let mut image_reader = ImageReader::open(image_path)?;
    image_reader.set_format(image_format);
    let image = image_reader.decode()?;
    let (image_width, image_height) = image.dimensions();
    if image_width == 0 || image_height == 0 {
        return Err(DocumentError::InvalidImageDimensions);
    }

    let fit = fit_contain(
        image_width as f64,
        image_height as f64,
        paper.width_mm,
        paper.height_mm,
    )?;
    let raw_image = raw_image_from_dynamic_image(image);

    // PDF 打印坐标使用物理单位，所以先把图片像素映射到稳定的
    // 标签打印机 DPI，再缩放进计算好的排版矩形。
    let mut doc = PdfDocument::new("yinshu-document");
    let image_id = doc.add_image(&raw_image);
    let natural_width_mm = image_width as f64 * 25.4 / f64::from(DEFAULT_DPI);
    let natural_height_mm = image_height as f64 * 25.4 / f64::from(DEFAULT_DPI);

    let page = PdfPage::new(
        Mm(paper.width_mm as f32),
        Mm(paper.height_mm as f32),
        vec![Op::UseXobject {
            id: image_id,
            transform: XObjectTransform {
                translate_x: Some(mm_to_pt(fit.x)),
                translate_y: Some(mm_to_pt(fit.y)),
                rotate: None,
                scale_x: Some((fit.width / natural_width_mm) as f32),
                scale_y: Some((fit.height / natural_height_mm) as f32),
                dpi: Some(DEFAULT_DPI),
            },
        }],
    );

    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    fs::write(output_path, bytes)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestPageSection {
    pub heading: String,
    pub rows: Vec<TestPageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestPageRow {
    pub label: String,
    pub value: String,
}

/// 测试页上按区块绘制的可读内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPageContent {
    pub(crate) title: String,
    pub(crate) kicker: String,
    pub(crate) subtitle: String,
    pub(crate) sections: Vec<TestPageSection>,
    pub(crate) footer: String,
}

impl TestPageContent {
    /// 使用已排好的文本行构造测试页内容。
    pub fn new(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            title: String::new(),
            kicker: String::new(),
            subtitle: String::new(),
            sections: vec![TestPageSection {
                heading: String::new(),
                rows: lines
                    .into_iter()
                    .map(|line| TestPageRow {
                        label: String::new(),
                        value: line.into(),
                    })
                    .collect(),
            }],
            footer: String::new(),
        }
    }
}

/// 生成一张列出当前配置和设备信息的测试页。
pub fn test_page_to_pdf(
    paper: &EffectivePaper,
    content: &TestPageContent,
    output_path: &Path,
) -> DocumentResult<()> {
    paper
        .validate()
        .map_err(|_| DocumentError::InvalidImageDimensions)?;
    if !is_positive_finite(paper.width_mm) || !is_positive_finite(paper.height_mm) {
        return Err(DocumentError::InvalidImageDimensions);
    }

    let mut doc = PdfDocument::new("test-page");
    let page = PdfPage::new(
        Mm(paper.width_mm as f32),
        Mm(paper.height_mm as f32),
        test_page_ops(paper, content),
    );
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    fs::write(output_path, bytes)?;

    Ok(())
}

#[derive(Clone, Copy)]
struct PageType {
    margin: f64,
    title_pt: f64,
    kicker_pt: f64,
    subtitle_pt: f64,
    heading_pt: f64,
    body_pt: f64,
    footer_pt: f64,
    label_col_mm: f64,
    show_kicker: bool,
    show_subtitle: bool,
    show_scale: bool,
}

fn page_type(paper: &EffectivePaper) -> PageType {
    let min_side = paper.width_mm.min(paper.height_mm);
    PageType {
        margin: (min_side * 0.085).clamp(1.2, 18.0),
        title_pt: (min_side * 0.124).clamp(8.0, 26.0),
        kicker_pt: (min_side * 0.043).clamp(5.5, 9.0),
        subtitle_pt: (min_side * 0.052).clamp(6.0, 11.0),
        heading_pt: (min_side * 0.038).clamp(5.5, 8.0),
        body_pt: (min_side * 0.052).clamp(6.5, 11.0),
        footer_pt: (min_side * 0.038).clamp(5.5, 8.0),
        label_col_mm: (min_side * 0.114).clamp(12.0, 24.0),
        show_kicker: min_side >= 36.0,
        show_subtitle: min_side >= 80.0,
        show_scale: min_side >= 140.0,
    }
}

/// 构造测试页边框、分区和按纸张缩放的正文。
fn test_page_ops(paper: &EffectivePaper, content: &TestPageContent) -> Vec<Op> {
    let layout = page_type(paper);
    let width = paper.width_mm;
    let height = paper.height_mm;
    let margin = layout.margin;
    let max_width = (width - margin * 2.0).max(4.0);
    let latin = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
    let floor = if layout.show_scale {
        margin + 14.0
    } else if !content.footer.is_empty() {
        margin + 8.0
    } else {
        margin
    };

    let mut ops = vec![Op::SaveGraphicsState];
    ops.push(Op::SetOutlineColor {
        col: grayscale(0.0),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.35) });
    ops.push(Op::DrawLine {
        line: closed_line(&[
            (margin * 0.4, margin * 0.4),
            (width - margin * 0.4, margin * 0.4),
            (width - margin * 0.4, height - margin * 0.4),
            (margin * 0.4, height - margin * 0.4),
        ]),
    });

    let mut y = height - margin - pt_mm(layout.title_pt) * 0.88;
    if !content.title.is_empty() {
        y = draw_wrapped(
            &mut ops,
            &bold,
            layout.title_pt,
            margin,
            y,
            floor,
            max_width,
            &content.title,
            0.0,
        );
    }
    if layout.show_kicker && !content.kicker.is_empty() {
        y -= pt_mm(layout.kicker_pt) * 1.55;
        y = draw_wrapped(
            &mut ops,
            &latin,
            layout.kicker_pt,
            margin,
            y,
            floor,
            max_width,
            &content.kicker,
            0.4,
        );
    }
    if !content.title.is_empty() || !content.kicker.is_empty() {
        y -= 3.4;
        ops.push(Op::SetOutlineColor {
            col: grayscale(0.0),
        });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.35) });
        ops.push(Op::DrawLine {
            line: Line {
                points: vec![line_point(margin, y), line_point(margin + max_width, y)],
                is_closed: false,
            },
        });
        y -= 5.2;
    }
    if layout.show_subtitle && !content.subtitle.is_empty() {
        y = draw_wrapped(
            &mut ops,
            &latin,
            layout.subtitle_pt,
            margin,
            y,
            floor,
            max_width,
            &content.subtitle,
            0.28,
        );
        y -= 6.5;
    }

    for section in &content.sections {
        if y < floor + 6.0 {
            break;
        }
        if !section.heading.is_empty() {
            y -= 4.2;
            y = draw_wrapped(
                &mut ops,
                &bold,
                layout.heading_pt,
                margin,
                y,
                floor,
                max_width,
                &section.heading.to_ascii_uppercase(),
                0.4,
            );
            y -= pt_mm(layout.body_pt) * 1.45;
        }
        for row in &section.rows {
            if y < floor {
                break;
            }
            y = draw_row(
                &mut ops,
                &latin,
                layout.body_pt,
                margin,
                y,
                floor,
                max_width,
                layout.label_col_mm,
                row,
            );
            y -= pt_mm(layout.body_pt) * 1.38;
        }
    }

    let bar_y = margin + 3.4;
    if layout.show_scale {
        let bar_width = 20.0_f64.min(max_width);
        ops.push(Op::SetFillColor {
            col: grayscale(0.0),
        });
        ops.push(filled_rect(margin, bar_y, margin + bar_width, bar_y + 1.3));
        draw_wrapped(
            &mut ops,
            &latin,
            7.0,
            margin,
            bar_y + 5.4,
            margin,
            max_width,
            "20 mm",
            0.35,
        );
    }
    if !content.footer.is_empty() {
        let footer_width = text_width_mm(&content.footer, layout.footer_pt);
        let footer_x = if layout.show_scale {
            (width - margin - footer_width).max(margin + 28.0)
        } else {
            margin
        };
        draw_wrapped(
            &mut ops,
            &latin,
            layout.footer_pt,
            footer_x,
            if layout.show_scale {
                bar_y + 5.4
            } else {
                margin + 3.2
            },
            margin,
            max_width,
            &content.footer,
            0.4,
        );
    }

    ops.push(Op::RestoreGraphicsState);
    ops
}

fn draw_row(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    size_pt: f64,
    x: f64,
    y: f64,
    floor: f64,
    max_width: f64,
    label_col_mm: f64,
    row: &TestPageRow,
) -> f64 {
    if row.label.is_empty() {
        return draw_wrapped(ops, font, size_pt, x, y, floor, max_width, &row.value, 0.0);
    }
    let value_x = x + label_col_mm;
    let value_width = (max_width - label_col_mm).max(8.0);
    draw_wrapped(
        ops,
        font,
        size_pt,
        x,
        y,
        floor,
        label_col_mm - 1.5,
        &row.label,
        0.4,
    );
    draw_wrapped(ops, font, size_pt, value_x, y, floor, value_width, &row.value, 0.0)
}

fn draw_wrapped(
    ops: &mut Vec<Op>,
    font: &PdfFontHandle,
    size_pt: f64,
    x: f64,
    mut y: f64,
    floor: f64,
    max_width_mm: f64,
    text: &str,
    color: f32,
) -> f64 {
    let line_height_mm = pt_mm(size_pt) * 1.28;
    let max_chars = max_chars_for_width(max_width_mm, size_pt);
    let mut first = true;
    for line in wrap_pdf_text(text, max_chars) {
        if !first {
            y -= line_height_mm;
        }
        first = false;
        if y < floor {
            break;
        }
        if line.is_empty() {
            continue;
        }
        ops.push(Op::StartTextSection);
        ops.push(Op::SetFillColor {
            col: grayscale(color),
        });
        ops.push(Op::SetFont {
            font: font.clone(),
            size: Pt(size_pt as f32),
        });
        ops.push(Op::SetTextCursor {
            pos: point_mm(x, y),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(line)],
        });
        ops.push(Op::EndTextSection);
    }
    y
}

fn pt_mm(size_pt: f64) -> f64 {
    size_pt * 25.4 / 72.0
}

fn text_width_mm(text: &str, size_pt: f64) -> f64 {
    text.chars().count() as f64 * size_pt * 0.52 * 25.4 / 72.0
}

fn filled_rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Op {
    Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    line_point(x0, y0),
                    line_point(x1, y0),
                    line_point(x1, y1),
                    line_point(x0, y1),
                ],
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        },
    }
}

fn max_chars_for_width(max_width_mm: f64, font_pt: f64) -> usize {
    let max_pt = max_width_mm * 72.0 / 25.4;
    ((max_pt / (font_pt * 0.52)).floor() as usize).max(8)
}

fn wrap_pdf_text(text: &str, max_chars: usize) -> Vec<String> {
    let text = pdf_safe(text);
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut rest = text.as_str();
    while rest.chars().count() > max_chars {
        let split_at = rest
            .char_indices()
            .take(max_chars)
            .last()
            .map(|(index, character)| index + character.len_utf8())
            .unwrap_or(rest.len());
        let candidate = &rest[..split_at];
        let cut = candidate
            .rfind(' ')
            .filter(|&index| index >= max_chars / 2)
            .unwrap_or(split_at);
        let cut = if cut == 0 { split_at } else { cut };
        lines.push(rest[..cut].trim_end().to_string());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        lines.push(rest.to_string());
    }
    lines
}

fn pdf_safe(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_graphic() || character == ' ' {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn closed_line(points: &[(f64, f64)]) -> Line {
    Line {
        points: points.iter().map(|(x, y)| line_point(*x, *y)).collect(),
        is_closed: true,
    }
}

fn line_point(x: f64, y: f64) -> LinePoint {
    LinePoint {
        p: point_mm(x, y),
        bezier: false,
    }
}

fn point_mm(x: f64, y: f64) -> Point {
    Point {
        x: mm_to_pt(x),
        y: mm_to_pt(y),
    }
}

fn grayscale(value: f32) -> Color {
    Color::Rgb(PdfRgb {
        r: value,
        g: value,
        b: value,
        icc_profile: None,
    })
}

/// 把毫米转换为 printpdf 变换所需的 PDF 点。
fn mm_to_pt(value: f64) -> Pt {
    Mm(value as f32).into()
}

/// 把图片转换为 printpdf 需要的 RGB 原始图片格式。
fn raw_image_from_dynamic_image(image: DynamicImage) -> RawImage {
    let (width, height, pixels) = flatten_to_white(image);

    RawImage {
        pixels: RawImageData::U8(pixels),
        width: width as usize,
        height: height as usize,
        data_format: RawImageFormat::RGB8,
        tag: Vec::new(),
    }
}

/// 把透明度合成到白底上，保证透明标签打印结果可预期。
fn flatten_to_white(image: DynamicImage) -> (u32, u32, Vec<u8>) {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = ImageBuffer::new(width, height);

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let red = blend_channel(pixel[0], alpha);
        let green = blend_channel(pixel[1], alpha);
        let blue = blend_channel(pixel[2], alpha);
        rgb.put_pixel(x, y, Rgb([red, green, blue]));
    }

    (width, height, rgb.into_raw())
}

/// 把单个颜色通道与白色背景混合。
fn blend_channel(channel: u8, alpha: f32) -> u8 {
    (f32::from(channel) * alpha + 255.0 * (1.0 - alpha)).round() as u8
}

/// 检查尺寸是否能安全参与布局计算。
fn is_positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use super::{max_chars_for_width, wrap_pdf_text};

    #[test]
    fn wrap_pdf_text_breaks_long_values_on_spaces() {
        assert_eq!(
            wrap_pdf_text("one two three four", 10),
            vec!["one two", "three four"]
        );
    }

    #[test]
    fn wrap_pdf_text_replaces_non_ascii_for_builtin_font() {
        assert_eq!(wrap_pdf_text("打印机 Zebra", 20), vec!["??? Zebra"]);
    }

    #[test]
    fn max_chars_stays_readable_on_small_labels() {
        assert!(max_chars_for_width(54.0, 6.0) >= 20);
    }
}
