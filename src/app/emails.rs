use crate::db::list::List;
use crate::prelude::*;

/// Add all `email` routes to the router.
#[rustfmt::skip]
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| {
        r.route("/emails/{id}/footer.gif", get(email_opened))
         .route("/emails/{id}/unsubscribe", get(email_unsubscribe_view).post(email_unsubscribe_form))
    })
}

async fn email_opened(Path(email_id): Path<i64>, State(state): State<SharedAppState>) -> HtmlResult {
    Email::mark_opened(&state.db, email_id).await?;
    let pixel = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/gif")
        .body(PIXEL.into())
        .unwrap();
    Ok(pixel)
}

async fn email_unsubscribe_view(
    user: Option<User>, Path(email_id): Path<i64>, State(state): State<SharedAppState>,
) -> HtmlResult {
    // TODO: Better error handling rather than silently eating
    if let Some(email) = Email::lookup(&state.db, email_id).await? {
        let list_id = email.list_id.ok_or_else(invalid)?;
        let list = List::lookup_by_id(&state.db, list_id).await?.ok_or_else(invalid)?;

        #[derive(Template, WebTemplate)]
        #[template(path = "emails/unsubscribe.html")]
        struct UnsubscribeHtml {
            user: Option<User>,
            list: List,
            email_id: i64,
        }
        Ok(UnsubscribeHtml { user, list, email_id }.into_response())
    } else {
        // XXX: Use a unique unsubscribe id instead of sequential email ids.
        // TODO: Record unsubscription, rather than assuming no match means you already unsubscribed.
        #[derive(Template, WebTemplate)]
        #[template(path = "emails/already_unsubscribed.html")]
        struct AlreadyUnsubscribedHtml {
            user: Option<User>,
        }
        Ok(AlreadyUnsubscribedHtml { user }.into_response())
    }
}

async fn email_unsubscribe_form(
    Path(email_id): Path<i64>, State(state): State<SharedAppState>,
) -> HtmlResult {
    // TODO: Better error handling rather than silently eating
    if let Some(email) = Email::lookup(&state.db, email_id).await?
        && let Some(list_id) = email.list_id
    {
        List::remove_member(&state.db, list_id, email.user_id).await?;
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
