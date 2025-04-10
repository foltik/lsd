use crate::db::{list, user::User};
use askama::Template;

#[derive(Template)]
#[template(path = "lists/view.html")]
pub struct Lists {
    pub user: Option<User>,
    pub lists: Vec<list::List>,
}

#[derive(Template)]
#[template(path = "lists/edit.html")]
pub struct ListEdit {
    pub user: Option<User>,
    pub list: list::List,
    pub members: Vec<list::ListMember>,
}
