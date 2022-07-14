#[macro_use]
extern crate rocket;
use clap::Parser;
use rocket::figment::{
    providers::{Format, Toml},
    Figment,
};
use rocket::response::content::RawHtml;
use rocket::serde::{Deserialize, Serialize};
use rocket::State;
use tera::Tera;
#[macro_use]
extern crate lazy_static;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

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
fn index(state: &State<ToolConfig>) -> RawHtml<String> {
    // "Hello, world!"
    // format!("Hello world\n{:?}", state.tags)
    let mut context = tera::Context::new();
    context.insert("tags", &state.tags);
    RawHtml(TEMPLATES.render("index.html", &context).unwrap())
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
    let config: ToolConfig = Figment::new().merge(Toml::file(args.config)).extract()?;
    rocket_config.port = config.server.port.clone();
    let ip_addr = match config.server.host.as_str() {
        "localhost" => IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        octets @ _ => octets.parse()?,
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

    if args.open {
        let url = format!("http://{}:{}", rocket_config.address, rocket_config.port);
        webbrowser::open(&url)?;
    }
    let r = rocket::custom(rocket_config)
        .mount("/", routes![index])
        // .attach(Template::fairing())
        .manage(config);
    let _ = r.launch().await;
    Ok(())
}
