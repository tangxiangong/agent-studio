use agent_client_protocol::ImageContent;
use gpui::{Image, ImageFormat};

pub async fn image_to_content(image: Image) -> anyhow::Result<(ImageContent, String)> {
    let temp_path = crate::utils::file::write_image_to_temp_file(&image).await?;

    let filename = std::path::Path::new(&temp_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image.png")
        .to_string();

    let bytes = std::fs::read(&temp_path);
    let _ = std::fs::remove_file(&temp_path);
    let bytes = bytes?;

    use base64::Engine;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let mime_type = mime_type_for_format(image.format);
    let image_content = ImageContent::new(base64_data, mime_type.to_string());

    Ok((image_content, filename))
}

fn mime_type_for_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Webp => "image/webp",
        ImageFormat::Gif => "image/gif",
        ImageFormat::Svg => "image/svg+xml",
        ImageFormat::Bmp => "image/bmp",
        ImageFormat::Tiff => "image/tiff",
        ImageFormat::Ico => "image/icon",
    }
}
