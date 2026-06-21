use std::sync::RwLock;

use bytes::Bytes;

use crate::db::event_flyer::{EventFlyer, GalleryEventFlyer};
use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/gallery", get(gallery_page)))
}

async fn gallery_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    #[derive(Template, WebTemplate)]
    #[template(path = "gallery.html")]
    struct Html {
        user: Option<User>,
        flyers: Vec<GalleryEventFlyer>,
    }

    let cached = CACHE.read().unwrap().get(&user);
    let html = match cached {
        Some(html) => html,
        None => {
            let unlisted = user.is_some();
            let flyers = EventFlyer::list_gallery(&state.db, unlisted).await?;

            let html = Bytes::from(Html { user: user.clone(), flyers }.render()?);
            CACHE.write().unwrap().insert(&user, html.clone());

            html
        }
    };

    Ok(([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response())
}

static CACHE: RwLock<GalleryCache> =
    RwLock::new(GalleryCache { generation: 0, public: None, unlisted: None });

#[derive(Default)]
struct GalleryCache {
    generation: u64,
    public: Option<Bytes>,
    unlisted: Option<Bytes>,
}

impl GalleryCache {
    pub fn get(&self, user: &Option<User>) -> Option<Bytes> {
        if EventFlyer::cache_generation() > self.generation {
            return None;
        }
        match user {
            Some(_) => self.unlisted.clone(),
            None => self.public.clone(),
        }
    }

    pub fn insert(&mut self, user: &Option<User>, html: Bytes) {
        self.generation = EventFlyer::cache_generation();
        match user {
            Some(_) => self.unlisted = Some(html),
            None => self.public = Some(html),
        }
    }
}
