use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{
    db::{email::Email, list::List},
    utils::{
        error::AppResult,
        types::{AppRouter, SharedAppState},
    },
    views,
};

/// Add all `email` routes to the router.
pub fn routes() -> AppRouter {
    AppRouter::new()
        .route("/{id}/footer.gif", get(email_opened))
        .route("/{id}/unsubscribe", get(email_unsubscribe_view).post(email_unsubscribe_form))
}

async fn email_opened(Path(email_id): Path<i64>, State(state): State<SharedAppState>) -> AppResult<Response> {
    Email::mark_opened(&state.db, email_id).await?;
    let pixel = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/gif")
        .body(PIXEL.into())
        .unwrap();
    Ok(pixel)
}

async fn email_unsubscribe_view(
    Path(email_id): Path<i64>,
    State(state): State<SharedAppState>,
) -> AppResult<Response> {
    // TODO: Better error handling rather than silently eating
    if let Some(email) = Email::lookup(&state.db, email_id).await? {
        return Ok(views::emails::Unsubscribe { email_id, email_address: email.address }.into_response());
    }
    Ok("You have been unsubscribed.".into_response())
}

async fn email_unsubscribe_form(
    Path(email_id): Path<i64>,
    State(state): State<SharedAppState>,
) -> AppResult<Response> {
    // TODO: Better error handling rather than silently eating
    if let Some(email) = Email::lookup(&state.db, email_id).await? {
        if let Some(list_id) = email.list_id {
            List::remove_member(&state.db, list_id, &email.address).await?;
        }
    }
    Ok("You have been unsubscribed.".into_response())
}

/// A 1x1 transparent GIF.
#[rustfmt::skip]
const PIXEL: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // Header: "GIF89a"
    0x01, 0x00, // Logical Screen Width: 1
    0x01, 0x00, // Logical Screen Height: 1
    0x80,       // GCT flag = 1, Color Resolution = 0, Sort = 0, GCT Size = 2^(0+1)=2 colors
    0x00,       // Background Color Index = 0
    0x00,       // Pixel Aspect Ratio = 0 (no aspect ratio given)
    // Global Color Table (2 entries, each 3 bytes: RGB)
    0x00, 0x00, 0x00, // Index #0: black (will be set as transparent)
    0x00, 0x00, 0x00, // Index #1: black
    // Graphic Control Extension
    0x21, 0xF9, 0x04, // Extension Introducer (0x21), GCE Label (0xF9), Block Size (4)
    0x01,             // Packed Fields: bit 0 = 1 => Transparent Color Flag
    0x00, 0x00,       // Delay Time = 0
    0x00,             // Transparent Color Index = 0
    0x00,             // Block Terminator
    // Image Descriptor
    0x2C,                   // Image separator: ','
    0x00, 0x00, 0x00, 0x00, // Image Position: (0,0)
    0x01, 0x00,             // Image Width: 1
    0x01, 0x00,             // Image Height: 1
    0x00,                   // No Local Color Table, no interlace, etc.
    // Image Data
    0x02,       // LZW Minimum Code Size
    0x02,       // Block Size (number of bytes of LZW data in this sub-block)
    0x4C, 0x01, // LZW-compressed data
    0x00,       // Block Terminator (end of image data)
    0x3B,       // Trailer: ';'
];
