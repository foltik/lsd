use crate::prelude::*;
use std::path::Path;
use tokio::fs;

/// Process image file: validate, resize, and save.
pub async fn process_image(base64_data: &str, filename: &Option<String>) -> AppResult<String> {
    // Decode base64 data
    let image_bytes = BASE64_STANDARD
        .decode(base64_data)
        .map_err(|e| AppError::ValidationError(format!("Invalid base64 image: {}", e)))?;

    //TODO: Temporarily commented out for testing
    // Validate image size
    // const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024; //5MB
    // if image_bytes.len() > MAX_IMAGE_SIZE {
    //     return Err(AppError::ValidationError("Image size exceeds 5MB limit".to_string()));
    // }

    //Validate image by checking magic bytes
    if !is_valid_image(&image_bytes) {
        return Err(AppError::ValidationError("Invalid image format".to_string()));
    }

    //Generate a unique filename if not provided
    let file_extension = get_file_extension(filename.as_deref(), &image_bytes);
    let unique_img_filename = format!(
        "event_{}.{}.{}",
        Utc::now().timestamp(),
        uuid::Uuid::new_v4().to_string()[..8].to_string(),
        file_extension
    );

    // Create directory for uploaded images if it doesn't exist
    let uploads_dir = Path::new("frontend/static/uploads/images/events");
    fs::create_dir_all(uploads_dir)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to create uploads directory: {}", e)))?;

    // Save the original image file
    let image_path = uploads_dir.join(&unique_img_filename);
    fs::write(&image_path, &image_bytes)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to save image:{}", e)))?;

    // TODO: Thumbnail creation and resizing

    Ok(format!("/static/uploads/images/events/{}", unique_img_filename))
}

/// Validate image by checking magic bytes - file signatures
fn is_valid_image(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    // Check for common image formats
    match &bytes[0..4] {
        [0xFF, 0xD8, 0xFF, ..] => true,   // JPEG
        [0x89, 0x50, 0x4E, 0x47] => true, // PNG
        [0x47, 0x49, 0x46, 0x38] => true, // GIF
        [0x52, 0x49, 0x46, 0x46] => {
            // WebP (RIFF container)
            bytes.len() >= 12 && &bytes[8..12] == b"WEBP"
        }
        _ => false,
    }
}

/// Get appropriate file extension based on filename or image content
fn get_file_extension(filename: Option<&str>, bytes: &[u8]) -> String {
    // Try to get extension from filename first
    if let Some(fname) = filename {
        if let Some(ext) = fname.split('.').last() {
            match ext.to_lowercase().as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "webp" => return ext.to_lowercase(),
                _ => {}
            }
        }
    }

    // Fall back to detecting from content
    if bytes.len() >= 4 {
        match &bytes[0..4] {
            [0xFF, 0xD8, 0xFF, ..] => "jpg".to_string(),
            [0x89, 0x50, 0x4E, 0x47] => "png".to_string(),
            [0x47, 0x49, 0x46, 0x38] => "gif".to_string(),
            [0x52, 0x49, 0x46, 0x46] => "webp".to_string(),
            _ => "jpg".to_string(), // Default fallback
        }
    } else {
        "jpg".to_string()
    }
}

/// Delete an image file from disk
pub async fn delete_image(file_path: &str) -> AppResult<()> {
    if file_path.starts_with("/static/uploads/") {
        let full_path = format!("frontend{}", file_path); // Convert web path to file path
        if Path::new(&full_path).exists() {
            fs::remove_file(&full_path)
                .await
                .map_err(|e| AppError::InternalError(format!("Failed to delete image: {}", e)))?;
        }
    }
    Ok(())
}
