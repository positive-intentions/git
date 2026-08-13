//! Force-linked Git gallery stories.

mod clone;
mod files;
mod init;
mod status;

use whatsup_ui::gallery::{GuiStory, TuiStory};

pub static GUI_STORIES: &[&GuiStory] = &[
    &init::GUI_STORY,
    &files::GUI_STORY,
    &status::GUI_STORY,
    &clone::GUI_STORY,
];

pub static TUI_STORIES: &[&TuiStory] = &[];
