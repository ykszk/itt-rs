#[macro_use]
extern crate rocket;
use clap::Parser;
use rocket::figment::{
    providers::{Format, Toml},
    Figment,
};
use rocket::response::content::{self, RawHtml};
use rocket::serde::{Deserialize, Serialize};
use rocket::State;
use tera::Tera;
#[macro_use]
extern crate lazy_static;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]

struct DBItem {
    image_name: String,
    image_path: PathBuf,
    tag_path: PathBuf,
    checked_tags: Vec<String>,
}

impl DBItem {
    fn load_tags(tag_path: &Path) -> Vec<String> {
        let contents = std::fs::read_to_string(tag_path);
        if let Ok(contents) = contents {
            contents.lines().map(|s| s.into()).collect()
        } else {
            Vec::new()
        }
    }
    pub fn new(image_path: PathBuf, tag_dir: &Path) -> Self {
        let image_name = image_path.to_str().unwrap().into();
        let mut tag_path = tag_dir.join(image_path.file_name().unwrap());
        tag_path.set_extension("txt");
        let image_path = Path::new("/images").join(image_path.file_name().unwrap());
        let checked_tags = Self::load_tags(tag_path.as_path());
        // println!("{:?}: {:?}", tag_path, checked_tags);
        DBItem {
            image_name,
            image_path,
            tag_path,
            checked_tags,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct TxtDB {
    items: Vec<DBItem>,
}

impl TxtDB {
    pub fn new(img_dir: &Path, tag_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let re = regex::Regex::new(r"^.+\.jpg|png|jpeg$").unwrap();
        let items: Vec<_> = std::fs::read_dir(img_dir)?
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|path| re.is_match(path.file_name().to_str().unwrap()))
            .map(|img_path| DBItem::new(img_path.path(), tag_dir))
            .collect();
        Ok(TxtDB { items })
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ToolState {
    config: ToolConfig,
    db: TxtDB,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ToolConfig {
    img_dir: PathBuf,
    tag_dir: PathBuf,
    tags: Vec<String>,
    multilabel: bool,
    server: ToolConfigServer,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct ToolConfigServer {
    host: String,
    port: u16,
    threads: u16,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Config toml file path
    config: PathBuf,
    /// Open web browser
    #[clap(short, long)]
    open: bool,
}

#[get("/")]
fn index(state: &State<ToolState>) -> RawHtml<String> {
    let mut context = tera::Context::new();
    context.insert("tags", &state.config.tags);
    RawHtml(TEMPLATES.render("index.html", &context).unwrap())
}

#[get("/list")]
fn list(state: &State<ToolState>) -> RawHtml<String> {
    let mut context = tera::Context::new();
    context.insert("tags", &state.config.tags);
    context.insert("multilabel", &state.config.multilabel);
    context.insert("image_name_path_tags", &state.db.items);
    RawHtml(TEMPLATES.render("list.html", &context).unwrap())
}

// use rocket::form::Form;
use rocket::form::{Form, Strict};
use rocket::http::RawStr;
#[derive(FromForm, Debug)]
struct MyForm {
    numbers: Vec<(String, Boolean)>,
}
#[put("/put", data = "<data>")]
fn put(state: &State<ToolState>, data: Form<MyForm>) -> String {
    println!("{:?}", data);
    let tags = if state.config.multilabel { "" } else { "" };
    "".into()
}
use rocket::http::ContentType;
#[get("/static/main.js")]
fn mainjs() -> (ContentType, &'static str) {
    let s = include_str!("../python/static/main.js");
    (ContentType::JavaScript, s)
}

#[get("/static/style.css")]
fn stylecss() -> (ContentType, &'static str) {
    let s = include_str!("../python/static/style.css");
    (ContentType::CSS, s)
}

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = tera::Tera::new("/dev/null/*").unwrap();
        tera.autoescape_on(vec![]);
        tera.add_raw_templates(vec![
            (
                "footer.html",
                include_str!("../python/templates/footer.html"),
            ),
            ("index.html", include_str!("../python/templates/index.html")),
            (
                "layout.html",
                include_str!("../python/templates/layout.html"),
            ),
            ("list.html", include_str!("../python/templates/list.html")),
            ("stats.html", include_str!("../python/templates/stats.html")),
        ])
        .unwrap();
        tera
    };
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut rocket_config = rocket::Config::default();
    let config: ToolConfig = Figment::new().merge(Toml::file(&args.config)).extract()?;
    rocket_config.port = config.server.port;
    let ip_addr = match config.server.host.as_str() {
        "localhost" => IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        octets => octets.parse()?,
    };
    rocket_config.address = ip_addr;
    {
        let mut tera = tera::Tera::new("/dev/null/*").unwrap();
        tera.autoescape_on(vec![]);
        tera.add_raw_templates(vec![
            (
                "footer.html",
                include_str!("../python/templates/footer.html"),
            ),
            ("index.html", include_str!("../python/templates/index.html")),
            (
                "layout.html",
                include_str!("../python/templates/layout.html"),
            ),
            ("list.html", include_str!("../python/templates/list.html")),
            ("stats.html", include_str!("../python/templates/stats.html")),
        ])?;
    }

    // if config.img_dir.is_relative() {
    //     config.img_dir = args.config.parent().unwrap().join(config.img_dir.as_path());
    // };
    println!("{:?}", config.img_dir);
    let db = TxtDB::new(config.img_dir.as_path(), config.tag_dir.as_path())?;
    let fs = rocket::fs::FileServer::from(config.img_dir.as_path());
    let state = ToolState { config, db };
    if args.open {
        let url = format!("http://{}:{}", rocket_config.address, rocket_config.port);
        webbrowser::open(&url)?;
    }
    let r = rocket::custom(rocket_config)
        .mount("/", routes![index, list, put, mainjs, stylecss])
        .mount("/images", fs)
        .manage(state);
    let _ = r.launch().await;
    Ok(())
}
