use renderer::html_handlebars::helpers;
use renderer::Renderer;
use book::MDBook;
use book::bookitem::BookItem;
use {utils, theme};

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::error::Error;
use std::io::{self, Read};
use std::collections::BTreeMap;

use handlebars::Handlebars;

use serde_json;
use serde_json::value::ToJson;


pub struct HtmlHandlebars;

impl HtmlHandlebars {
    pub fn new() -> Self {
        HtmlHandlebars
    }
}

impl Renderer for HtmlHandlebars {
    fn render(&self, book: &MDBook) -> Result<(), Box<Error>> {
        debug!("[fn]: render");
        let mut handlebars = Handlebars::new();

        // Load theme
        let theme = theme::Theme::new(book.get_theme_path());

        // Register template
        debug!("[*]: Register handlebars template");
        try!(handlebars.register_template_string("index", try!(String::from_utf8(theme.index))));

        // Register helpers
        debug!("[*]: Register handlebars helpers");
        handlebars.register_helper("toc", Box::new(helpers::toc::RenderToc));
        handlebars.register_helper("previous", Box::new(helpers::navigation::previous));
        handlebars.register_helper("next", Box::new(helpers::navigation::next));

        let mut data = try!(make_data(book));

        // Print version
        let mut print_content: String = String::new();

        // Check if dest directory exists
        debug!("[*]: Check if destination directory exists");
        if let Err(_) = fs::create_dir_all(book.get_dest()) {
            return Err(Box::new(io::Error::new(io::ErrorKind::Other,
                                               "Unexpected error when constructing destination path")));
        }

        // Render a file for every entry in the book
        let mut index = true;
        for item in book.iter() {

            match *item {
                BookItem::Chapter(_, ref ch) |
                BookItem::Affix(ref ch) => {
                    if ch.path != PathBuf::new() {

                        let path = book.get_src().join(&ch.path);

                        debug!("[*]: Opening file: {:?}", path);
                        let mut f = try!(File::open(&path));
                        let mut content: String = String::new();

                        debug!("[*]: Reading file");
                        try!(f.read_to_string(&mut content));

                        // Parse for playpen links
                        if let Some(p) = path.parent() {
                            content = helpers::playpen::render_playpen(&content, p);
                        }

                        // Render markdown using the pulldown-cmark crate
                        content = utils::render_markdown(&content);
                        print_content.push_str(&content);

                        // Update the context with data for this file
                        let path = ch.path.to_str().ok_or(io::Error::new(io::ErrorKind::Other,
                                                          "Could not convert path to str"))?;
                        data.insert("path".to_owned(), path.to_json());
                        data.insert("content".to_owned(), content.to_json());
                        data.insert("chapter_title".to_owned(), ch.name.to_json());
                        data.insert("path_to_root".to_owned(), utils::fs::path_to_root(&ch.path).to_json());

                        // Render the handlebars template with the data
                        debug!("[*]: Render template");
                        let rendered = try!(handlebars.render("index", &data));

                        // Write to file
                        let filename = Path::new(&ch.path).with_extension("html");
                        info!("[*] Creating {:?} ✓", filename.display());
                        try!(book.write_file(filename, &rendered.into_bytes()));

                        // Create an index.html from the first element in SUMMARY.md
                        if index {
                            debug!("[*]: index.html");

                            let mut content = String::new();
                            let _source = try!(File::open(book.get_dest().join(&ch.path.with_extension("html"))))
                                .read_to_string(&mut content);

                            // This could cause a problem when someone displays code containing <base href=...>
                            // on the front page, however this case should be very very rare...
                            content = content.lines()
                                .filter(|line| !line.contains("<base href="))
                                .collect::<Vec<&str>>()
                                .join("\n");

                            try!(book.write_file("index.html", content.as_bytes()));

                            info!("[*] Creating index.html from {:?} ✓",
                                  book.get_dest().join(&ch.path.with_extension("html")));
                            index = false;
                        }
                    }
                },
                _ => {},
            }
        }

        // Print version

        // Update the context with data for this file
        data.insert("path".to_owned(), "print.md".to_json());
        data.insert("content".to_owned(), print_content.to_json());
        data.insert("path_to_root".to_owned(), utils::fs::path_to_root(Path::new("print.md")).to_json());

        // Render the handlebars template with the data
        debug!("[*]: Render template");
        let rendered = try!(handlebars.render("index", &data));
        try!(book.write_file(Path::new("print").with_extension("html"), &rendered.into_bytes()));
        info!("[*] Creating print.html ✓");

        // Copy static files (js, css, images, ...)

        debug!("[*] Copy static files");
        try!(book.write_file("book.js", &theme.js));
        try!(book.write_file("book.css", &theme.css));
        try!(book.write_file("favicon.png", &theme.favicon));
        try!(book.write_file("jquery.js", &theme.jquery));
        try!(book.write_file("highlight.css", &theme.highlight_css));
        try!(book.write_file("tomorrow-night.css", &theme.tomorrow_night_css));
        try!(book.write_file("highlight.js", &theme.highlight_js));
        try!(book.write_file("_FontAwesome/css/font-awesome.css", theme::FONT_AWESOME));
        try!(book.write_file("_FontAwesome/fonts/fontawesome-webfont.eot", theme::FONT_AWESOME_EOT));
        try!(book.write_file("_FontAwesome/fonts/fontawesome-webfont.svg", theme::FONT_AWESOME_SVG));
        try!(book.write_file("_FontAwesome/fonts/fontawesome-webfont.ttf", theme::FONT_AWESOME_TTF));
        try!(book.write_file("_FontAwesome/fonts/fontawesome-webfont.woff", theme::FONT_AWESOME_WOFF));
        try!(book.write_file("_FontAwesome/fonts/fontawesome-webfont.woff2", theme::FONT_AWESOME_WOFF2));
        try!(book.write_file("_FontAwesome/fonts/FontAwesome.ttf", theme::FONT_AWESOME_TTF));

        // Copy all remaining files
        try!(utils::fs::copy_files_except_ext(book.get_src(), book.get_dest(), true, &["md"]));

        Ok(())
    }
}

fn make_data(book: &MDBook) -> Result<serde_json::Map<String, serde_json::Value>, Box<Error>> {
    debug!("[fn]: make_data");

    let mut data = serde_json::Map::new();
    data.insert("language".to_owned(), "en".to_json());
    data.insert("title".to_owned(), book.get_title().to_json());
    data.insert("description".to_owned(), book.get_description().to_json());
    data.insert("favicon".to_owned(), "favicon.png".to_json());
    if let Some(livereload) = book.get_livereload() {
        data.insert("livereload".to_owned(), livereload.to_json());
    }

    let mut chapters = vec![];

    for item in book.iter() {
        // Create the data to inject in the template
        let mut chapter = BTreeMap::new();

        match *item {
            BookItem::Affix(ref ch) => {
                chapter.insert("name".to_owned(), ch.name.to_json());
                let path = ch.path.to_str().ok_or(io::Error::new(io::ErrorKind::Other,
                                                                 "Could not convert path to str"))?;
                chapter.insert("path".to_owned(), path.to_json());
            },
            BookItem::Chapter(ref s, ref ch) => {
                chapter.insert("section".to_owned(), s.to_json());
                chapter.insert("name".to_owned(), ch.name.to_json());
                let path = ch.path.to_str().ok_or(io::Error::new(io::ErrorKind::Other,
                                                                 "Could not convert path to str"))?;
                chapter.insert("path".to_owned(), path.to_json());
            },
            BookItem::Spacer => {
                chapter.insert("spacer".to_owned(), "_spacer_".to_json());
            },

        }

        chapters.push(chapter);
    }

    data.insert("chapters".to_owned(), chapters.to_json());

    debug!("[*]: JSON constructed");
    Ok(data)
}
