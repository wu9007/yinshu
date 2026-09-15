use crate::protocol::EffectivePaper;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, ImageReader, Rgb};
use printpdf::{
    ops::PdfFontHandle, BuiltinFont, Color, Line, LinePoint, Mm, Op, PdfDocument, PdfPage,
    PdfSaveOptions, Point, Pt, RawImage, RawImageData, RawImageFormat, Rgb as PdfRgb, TextItem,
    XObjectTransform,
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

/// 测试页上按行绘制的可读内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPageContent {
    pub lines: Vec<String>,
}

impl TestPageContent {
    /// 使用已排好的文本行构造测试页内容。
    pub fn new(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            lines: lines.into_iter().map(Into::into).collect(),
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

/// 构造测试页边框和从上到下排版的文本行。
fn test_page_ops(paper: &EffectivePaper, content: &TestPageContent) -> Vec<Op> {
    let min_side = paper.width_mm.min(paper.height_mm);
    let margin = (min_side * 0.06).clamp(0.8, 3.0).min(min_side / 4.0);
    let width = paper.width_mm;
    let height = paper.height_mm;
    let font_pt = (min_side * 0.14).clamp(5.0, 8.0);
    let line_height_mm = font_pt * 25.4 / 72.0 * 1.28;
    let max_width_mm = (width - margin * 2.0).max(4.0);
    let max_chars = max_chars_for_width(max_width_mm, font_pt);
    let wrapped: Vec<String> = content
        .lines
        .iter()
        .flat_map(|line| wrap_pdf_text(line, max_chars))
        .collect();

    let mut ops = vec![
        Op::SaveGraphicsState,
        Op::SetOutlineColor {
            col: grayscale(0.0),
        },
        Op::SetOutlineThickness { pt: Pt(0.4) },
        Op::DrawLine {
            line: closed_line(&[
                (margin * 0.45, margin * 0.45),
                (width - margin * 0.45, margin * 0.45),
                (width - margin * 0.45, height - margin * 0.45),
                (margin * 0.45, height - margin * 0.45),
            ]),
        },
        Op::StartTextSection,
        Op::SetFillColor {
            col: grayscale(0.0),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(font_pt as f32),
        },
    ];

    let mut y = height - margin - font_pt * 25.4 / 72.0;
    for line in wrapped {
        if y < margin {
            break;
        }
        ops.push(Op::SetTextCursor {
            pos: point_mm(margin, y),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(line)],
        });
        y -= line_height_mm;
    }

    ops.push(Op::EndTextSection);
    ops.push(Op::RestoreGraphicsState);
    ops
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
