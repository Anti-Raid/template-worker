// Generates AntiRaid documentation from docgen data
use crate::docgen::{document_all_plugins, document_all_primitives};

pub fn docgen() {
    let mut markdown = String::new();

    // First, document all the plugins
    markdown.push_str(&document_all_plugins(1));

    // Next, document all the primitive types
    markdown.push_str(&document_all_primitives(1));

    println!("{}", markdown);
}
