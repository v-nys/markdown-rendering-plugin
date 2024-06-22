extern crate learning_paths_tauri_react;

use base64::encode;
use comrak::{markdown_to_html, ComrakOptions};
use learning_paths_tauri_react::plugins::Plugin;
use regex;
use serde_yaml::Value;
use std::io::Read;
use std::time::SystemTime;
use std::{
    cmp::Ordering,
    fs,
    fs::File,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub struct MarkdownRenderingPlugin;

fn find_md_files(dir: &Path) -> Vec<PathBuf> {
    let mut md_files = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "md" {
                    md_files.push(path.to_path_buf());
                }
            }
        }
    }

    md_files
}

fn markdown_to_html_with_inlined_images(markdown: &str) -> String {
    let options = ComrakOptions::default();
    let original_html = markdown_to_html(markdown, &options);
    let mut substituted_html = original_html.clone();

    // Find all image tags and inline the images
    let re = regex::Regex::new(r#"!\[.*?\]\((.*?)\)"#).unwrap();
    for cap in re.captures_iter(&original_html) {
        let img_path = &cap[1];
        if let Ok(inlined_img) = inline_image(img_path) {
            substituted_html = substituted_html.replace(&cap[0], &inlined_img);
        }
    }

    substituted_html
}

fn inline_image(path: &str) -> Result<String, std::io::Error> {
    let path = Path::new(path);
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let base64_img = encode(&buf);
    let ext = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("png");
    let mime_type = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "png" => "image/png",
        _ => "application/octet-stream",
    };

    Ok(format!(
        r#"<img src="data:{};base64,{}" />"#,
        mime_type, base64_img
    ))
}

fn get_modification_date(path: &PathBuf) -> Option<SystemTime> {
    match fs::metadata(path) {
        Ok(metadata) => metadata.modified().ok(),
        Err(_) => None,
    }
}

fn file_is_readable(file_path: &Path) -> bool {
    file_path.is_file() && File::open(file_path).is_ok()
}

impl Plugin for MarkdownRenderingPlugin {
    fn can_process_extension_field(&self, field_name: &str) -> bool {
        false
    }

    fn process_cluster(&self, cluster_path: &Path) {
        println!("Start processing cluster");
        let md_files = find_md_files(cluster_path);
        md_files.iter().for_each(|md_file| {
            let html_counterpart = md_file.with_extension("html");
            let md_modification_date = get_modification_date(md_file);
            let html_modification_date = get_modification_date(&html_counterpart);
            let relation = md_modification_date
                .zip(html_modification_date)
                .map(|(md_time, html_time)| md_time.cmp(&html_time));
            match relation {
                None | Some(Ordering::Equal) | Some(Ordering::Greater) => {
                    let file_contents = std::fs::read_to_string(md_file);
                    match file_contents {
                        Err(_) => {}
                        Ok(file_contents) => {
                            println!("Markdown file should be rerendered.");
                            let html_output = markdown_to_html_with_inlined_images(&file_contents);
                            std::fs::write(html_counterpart, &html_output);
                        }
                    }
                }
                Some(Ordering::Less) => {
                    println!("HTML file is newer than Markdown file");
                }
            }
        });
        println!("Done processing cluster");
    }

    fn process_extension_field(
        &self,
        cluster_path: &Path,
        node_id: &str,
        field_name: &str,
        value: &Value,
        remarks: &mut Vec<String>,
    ) {
        panic!("shouldn't actually have this method");
    }

    fn get_name(&self) -> &str {
        "Markdown rendering"
    }

    fn get_version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    let plugin = Box::new(MarkdownRenderingPlugin);
    Box::into_raw(plugin)
}
