use clap::Parser;
use rocket::figment::{
    providers::{Format, Toml},
    Figment,
};
use rocket::form::Form;
use rocket::http::ContentType;
use rocket::response::content::RawHtml;
use rocket::serde::{Deserialize, Serialize};
use rocket::State;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use tera::Tera;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate lazy_static;

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

    fn load_json(tag_path: &Path) -> Vec<String> {
        let lm_data = labelme_rs::LabelMeData::load(tag_path).unwrap();
        lm_data
            .flags
            .into_iter()
            .filter(|(_, flag)| *flag)
            .map(|(label, _)| label)
            .collect()
    }

    pub fn new(image_path: PathBuf, tag_dir: &Path) -> Self {
        let image_name = image_path.to_str().unwrap().into();
        let mut tag_path = tag_dir.join(image_path.file_name().unwrap());
        tag_path.set_extension("txt");
        let image_path = Path::new("/images").join(image_path.file_name().unwrap());
        let checked_tags = if tag_path.exists() {
            Self::load_tags(&tag_path)
        } else {
            tag_path.set_extension("json");
            if tag_path.exists() {
                Self::load_json(&tag_path)
            } else {
                tag_path.set_extension("txt");
                Vec::new()
            }
        };
        DBItem {
            image_name,
            image_path,
            tag_path,
            checked_tags,
        }
    }
    pub fn update_tags(&mut self, tags: Vec<String>) {
        if self.tag_path.extension().unwrap() == ".txt" {
            std::fs::write(&self.tag_path, tags.join("\n")).expect("Failed to write.");
        } else {
            println!("{:?}", self.tag_path);
            let mut lm_data = labelme_rs::LabelMeData::load(&self.tag_path).unwrap();
            lm_data
                .flags
                .iter_mut()
                .for_each(|(label, flag)| *flag = tags.contains(label));
            lm_data.save(&self.tag_path).unwrap();
        }
        self.checked_tags = tags;
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct TxtDB {
    items: Vec<DBItem>,
}
use std::sync::{Arc, Mutex};
type TxtDBPointer = Arc<Mutex<TxtDB>>;

impl TxtDB {
    pub fn new(img_dir: &Path, tag_dir: &Path) -> Result<TxtDBPointer, Box<dyn std::error::Error>> {
        let re = regex::Regex::new(r"^.+\.jpg|png|jpeg$").unwrap();
        let mut items: Vec<_> = std::fs::read_dir(img_dir)?
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|path| re.is_match(path.file_name().to_str().unwrap()))
            .map(|img_path| DBItem::new(img_path.path(), tag_dir))
            .collect();
        items.sort_unstable_by(|a, b| a.image_name.cmp(&b.image_name));
        Ok(Arc::new(Mutex::new(TxtDB { items })))
    }
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
fn index(config: &State<ToolConfig>) -> RawHtml<String> {
    let mut context = tera::Context::new();
    context.insert("tags", &config.tags);
    RawHtml(TEMPLATES.render("index.html", &context).unwrap())
}

#[get("/list")]
fn list(config: &State<ToolConfig>, db: &State<TxtDBPointer>) -> RawHtml<String> {
    let mut context = tera::Context::new();
    context.insert("tags", &config.tags);
    context.insert("multilabel", &config.multilabel);
    let db = db.lock().unwrap();
    context.insert("image_name_path_tags", &db.items);
    RawHtml(TEMPLATES.render("list.html", &context).unwrap())
}

type FormTags = HashMap<String, bool>;
type QueryTags = HashMap<String, String>;

fn vec_compare(va: &[bool], vb: &[bool]) -> bool {
    (va.len() == vb.len()) && va.iter().zip(vb).all(|(a, b)| a == b)
}

#[get("/query?<tags..>")]
fn query(config: &State<ToolConfig>, db: &State<TxtDBPointer>, tags: QueryTags) -> RawHtml<String> {
    let mut context = tera::Context::new();
    context.insert("tags", &config.tags);
    context.insert("multilabel", &config.multilabel);
    let db = db.lock().unwrap();
    let include_tags: HashSet<String> = tags
        .iter()
        .filter(|t| *t.1 == "in")
        .map(|t| (*t.0).clone())
        .collect();
    let exclude_tags: HashSet<String> = tags
        .iter()
        .filter(|t| *t.1 == "ex")
        .map(|t| (*t.0).clone())
        .collect();
    let mut queried_tags: Vec<&String> = include_tags.union(&exclude_tags).into_iter().collect();
    queried_tags.sort();
    let queried_tags_vector: Vec<bool> = queried_tags
        .iter()
        .map(|t| include_tags.contains(*t))
        .collect();
    let qts = &queried_tags;
    let qtsv = &queried_tags_vector;
    let items: Vec<&DBItem> = db
        .items
        .iter()
        .filter(|i| {
            let checked_tags: HashSet<&String> = HashSet::from_iter(i.checked_tags.iter());
            let checked_tags_vector: Vec<bool> =
                qts.iter().map(|t| checked_tags.contains(t)).collect();
            vec_compare(checked_tags_vector.as_slice(), qtsv.as_slice())
        })
        .collect();
    context.insert("image_name_path_tags", &items);
    RawHtml(TEMPLATES.render("list.html", &context).unwrap())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
struct StatItem {
    key: String,
    count: usize,
    url: String,
}

static DELIM: &str = " & ";

#[get("/stats")]
fn stats(config: &State<ToolConfig>, db: &State<TxtDBPointer>) -> RawHtml<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let db = db.lock().unwrap();
    db.items.iter().for_each(|item| {
        let key = item.checked_tags.join(DELIM);
        *counts.entry(key).or_insert(0) += 1;
    });
    println!("{:?}", counts);
    let all_tags: HashSet<&str> = HashSet::from_iter(config.tags.iter().map(|t| t.as_str()));
    let mut v_stats: Vec<StatItem> = counts
        .into_iter()
        .map(|(key, count)| {
            let queries: Vec<&str> = if key.is_empty() {
                Vec::new()
            } else {
                key.split(DELIM).collect()
            };
            let mut params: Vec<String> = queries.iter().map(|q| format!("{}=in", q)).collect();
            let query_set = HashSet::from_iter(queries);
            let ex_tags = all_tags.difference(&query_set).copied();
            let exs: Vec<String> = ex_tags.map(|t| format!("{}=ex", t)).collect();
            params.extend(exs);
            let url = format!("/query?{}", params.join("&"));
            StatItem { key, count, url }
        })
        .collect();
    v_stats.sort_by(|a, b| a.key.cmp(&b.key));
    let mut context = tera::Context::new();
    context.insert("stats", &v_stats);
    RawHtml(TEMPLATES.render("stats.html", &context).unwrap())
}

#[put("/put?<name>", data = "<checked_tags>")]
fn put(db: &State<TxtDBPointer>, checked_tags: Form<FormTags>, name: &str) -> String {
    let mut db = db.lock().unwrap();
    let item = db
        .items
        .binary_search_by_key(&name, |item| &item.image_name);
    if let Ok(index) = item {
        let checked_tags: Vec<String> = checked_tags
            .iter()
            .filter(|v| *v.1)
            .map(|v| v.0.into())
            .collect();
        db.items[index].update_tags(checked_tags);
    }
    "".into()
}

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
    let mut config: ToolConfig = Figment::new().merge(Toml::file(&args.config)).extract()?;
    if config.img_dir.is_relative() {
        config.img_dir = args.config.parent().unwrap().join(config.img_dir);
    }
    if config.tag_dir.is_relative() {
        config.tag_dir = args.config.parent().unwrap().join(config.tag_dir);
    }
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

    let db = TxtDB::new(config.img_dir.as_path(), config.tag_dir.as_path())?;
    let fs = rocket::fs::FileServer::from(config.img_dir.as_path());

    if args.open {
        let url = format!("http://{}:{}", rocket_config.address, rocket_config.port);
        webbrowser::open(&url)?;
    }
    let r = rocket::custom(rocket_config)
        .mount(
            "/",
            routes![index, list, query, put, stats, mainjs, stylecss],
        )
        .mount("/images", fs)
        .manage(config)
        .manage(db);
    let _ = r.launch().await;
    Ok(())
}
