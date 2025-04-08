use crate::db::list;
use askama::Template;

#[derive(Template)]
#[template(path = "lists/view.html")]
pub struct Lists {
    pub lists: Vec<list::List>,
}

#[derive(Template)]
#[template(path = "lists/edit.html")]
pub struct ListEdit {
    pub list: list::List,
    pub members: Vec<list::ListMember>,
}
