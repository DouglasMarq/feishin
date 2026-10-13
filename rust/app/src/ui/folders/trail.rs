use std::cell::RefCell;
use std::iter;
use std::rc::Rc;

pub const ROOT_NAME: &str = "Folders";
const UNKNOWN_FOLDER_NAME: &str = "Folder";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub id: String,
    pub name: String,
}

impl Crumb {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    fn root() -> Self {
        Self::new("", ROOT_NAME)
    }
}

#[derive(Debug, Default)]
pub struct Trail {
    opened: Vec<Crumb>,
}

pub type SharedTrail = Rc<RefCell<Trail>>;

impl Trail {
    pub fn enter(&mut self, parent_id: &str, child: Crumb) {
        let kept = self.depth_of(parent_id).unwrap_or(0);
        self.opened.truncate(kept);
        self.opened.push(child);
    }

    pub fn path_to(&mut self, id: &str) -> Vec<Crumb> {
        let depth = self.depth_of(id).unwrap_or_else(|| {
            self.opened = vec![Crumb::new(id, UNKNOWN_FOLDER_NAME)];
            1
        });
        iter::once(Crumb::root())
            .chain(self.opened.iter().take(depth).cloned())
            .collect()
    }

    fn depth_of(&self, id: &str) -> Option<usize> {
        match id {
            "" => Some(0),
            _ => self
                .opened
                .iter()
                .position(|crumb| crumb.id == id)
                .map(|position| position + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(crumbs: &[Crumb]) -> Vec<&str> {
        crumbs.iter().map(|crumb| crumb.name.as_str()).collect()
    }

    #[test]
    fn opening_folders_builds_the_path() {
        let mut trail = Trail::default();
        trail.enter("", Crumb::new("a", "Artist"));
        trail.enter("a", Crumb::new("b", "Album"));
        assert_eq!(names(&trail.path_to("b")), ["Folders", "Artist", "Album"]);
        assert_eq!(names(&trail.path_to("")), ["Folders"]);
    }

    #[test]
    fn going_back_keeps_deeper_folders_until_another_branch_opens() {
        let mut trail = Trail::default();
        trail.enter("", Crumb::new("a", "Artist"));
        trail.enter("a", Crumb::new("b", "Album"));
        assert_eq!(names(&trail.path_to("a")), ["Folders", "Artist"]);
        assert_eq!(names(&trail.path_to("b")), ["Folders", "Artist", "Album"]);

        trail.enter("a", Crumb::new("c", "Live"));
        assert_eq!(names(&trail.path_to("c")), ["Folders", "Artist", "Live"]);
        assert_eq!(names(&trail.path_to("b")), ["Folders", "Folder"]);
    }
}
