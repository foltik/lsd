use askama::Template;
use askama_web::WebTemplate;

use crate::db::list;

#[derive(Template, WebTemplate)]
#[template(path = "lists/view.html")]
pub struct Lists {
    pub lists: Vec<list::List>,
}

#[derive(Template, WebTemplate)]
#[template(path = "lists/edit.html")]
pub struct ListEdit {
    pub list: list::List,
    pub members: Vec<list::ListMember>,
}
