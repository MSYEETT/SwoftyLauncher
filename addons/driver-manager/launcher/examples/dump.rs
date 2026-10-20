use driver_manager::game;
use swofty_formats::gvdb::Gvdb;
use swofty_formats::lng::Lng;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: dump <gamedatabases.2025.bin> <language_eng.lng|-> <build key> <out.json>");
        std::process::exit(2);
    }
    let db = Gvdb::parse(&std::fs::read(&args[1]).unwrap()).unwrap();
    let lng = (args[2] != "-").then(|| Lng::parse(&std::fs::read(&args[2]).unwrap()).unwrap());
    let data = game::extract(&db, lng.as_ref(), &args[3]);
    data.save_cached(std::path::Path::new(&args[4]));
}
