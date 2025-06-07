pub use std::collections::HashMap;
pub use std::convert::Infallible;
pub use std::sync::Arc;
pub use std::time::Duration;

pub use anyhow::{Context as _, Result};
pub use askama::Template;
pub use askama_web::WebTemplate;
pub use axum::extract::{Path, Query, Request, State};
pub use axum::http::{header, StatusCode};
pub use axum::middleware::Next;
pub use axum::response::{IntoResponse, Redirect, Response};
pub use axum::routing::{delete, get, post};
pub use axum::Form;
pub use axum::Json;
pub use base64::prelude::*;
pub use chrono::Utc;

pub use crate::db::email::Email;
pub use crate::db::user::User;
pub use crate::db::Db;
pub use crate::utils::config::Config;
pub use crate::utils::error::{AppError, AppResult};
pub use crate::utils::image::{delete_image, process_image};
pub use crate::utils::routing::{AppRouter, AxumRouter};
pub use crate::utils::templates::filters;
pub use crate::utils::types::SharedAppState;
