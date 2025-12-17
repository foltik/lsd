use crate::prelude::*;

pub struct EditorContent {
    pub html: String,
    pub updated_at: NaiveDateTime,
}
pub struct Editor {
    /// Where the content gets POSTed to.
    /// The string "{id}" is replaced with the current entity id.
    /// Returns JSON, either {id: 123} or {error: ""}
    pub url: &'static str,
    pub snapshot_prefix: &'static str,

    pub entity_id: Option<i64>,
    pub content: Option<EditorContent>,
}
