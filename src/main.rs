use clap::{App, Arg, SubCommand};
use serde::{Deserialize, Serialize};
use std::error::Error;

static PARENT_FOLDER: &str = ".graft.lock";
static TMP_FOLDER: &str = "tmp";
static CONFIG_FILE: &str = "graft.txt";
static LOCK_FILE: &str = "conflict.lock";

#[derive(Serialize, Deserialize, Debug)]
struct Repo {
    upstream_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    repos_to_fetch: Vec<Repo>,
}

fn read_config<P: AsRef<std::path::Path>>(path: P) -> Result<Config, Box<dyn Error>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}

fn write_file_content(content: String, file_path: &str) -> Result<(), Box<dyn Error>> {
    match std::path::Path::new(file_path).parent() {
        Some(p) => {
            std::fs::create_dir_all(p);
        }
        None => {}
    }
    let mut out = std::fs::File::create(file_path)
        .expect(format!("ERROR: failed to create file: {:?}", file_path).as_str());
    std::io::copy(&mut content.as_bytes(), &mut out).expect("ERROR: failed to copy content");
    Ok(())
}

fn extract_files(path: &str) -> Result<(), Box<dyn Error>> {
    let file = std::fs::File::open(path).unwrap();
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    ar.unpack(format!("{}/{}", PARENT_FOLDER, TMP_FOLDER).as_str())
        .unwrap();
    Ok(())
}

fn download_file(url: &str) -> Result<String, Box<dyn Error>> {
    let response = reqwest::blocking::get(url)?;
    let fname = response
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .and_then(|name| if name.is_empty() { None } else { Some(name) })
        .unwrap_or("tmp.bin")
        .to_owned();
    let mut dest = std::fs::File::create(&fname)?;
    let content = response
        .bytes()
        .expect(format!("ERROR: failed to download file from path: {:?}", url).as_str());
    std::io::copy(&mut content.as_ref(), &mut dest)?;
    Ok(fname)
}

fn merge_or_copy(file_path: &str) -> Result<bool, Box<dyn Error>> {
    let file_path = {
        let mut p = file_path.to_string();
        p.replace_range(
            ..(format!("{}/{}", PARENT_FOLDER, TMP_FOLDER).as_str().len() + 1),
            "",
        );
        p
    };

    let nf_path = format!("{}/{}/{}", PARENT_FOLDER, TMP_FOLDER, file_path.as_str());
    let nf_content = match std::fs::read_to_string(nf_path.as_str()) {
        Ok(nf_content) => nf_content,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                println!("WARNING: failed to read file: {:?}", nf_path.as_str());
                return Ok(true);
            } else {
                return Err(Box::new(e));
            }
        }
    };

    let pf_path = format!("{}/{}", PARENT_FOLDER, file_path.as_str());
    match std::fs::read_to_string(pf_path.as_str()) {
        Ok(pf_content) => {
            let cf_content = match std::fs::read_to_string(file_path.as_str()) {
                Ok(cf_content) => cf_content,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        "".to_string()
                    } else {
                        return Err(Box::new(e));
                    }
                }
            };

            match diffy::merge(
                pf_content.as_str(),
                nf_content.as_str(),
                cf_content.as_str(),
            ) {
                Ok(merged_content) => {
                    write_file_content(merged_content, file_path.as_str());
                    Ok(true)
                }
                Err(conflict_content) => {
                    write_file_content(conflict_content, file_path.as_str());
                    println!("WARNING: conflict in file: {:?}", file_path.as_str());
                    Ok(false)
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                write_file_content(nf_content, file_path.as_str());
                write_file_content(nf_content, pf_path);
                Ok(true)
            } else {
                return Err(Box::new(e));
            }
        }
    }
}

fn process_folder(path: &str) -> Result<bool, Box<dyn Error>> {
    let mut is_resolved_state = true;
    let dir = std::path::Path::new(path);
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let path_str = path.to_str().unwrap();
            if path.is_dir() {
                is_resolved_state = process_folder(path_str)? & is_resolved_state;
            } else {
                is_resolved_state = merge_or_copy(path_str)? & is_resolved_state;
            }
        }
    }
    Ok(is_resolved_state)
}

fn resolve() -> Result<(), Box<dyn Error>> {
    std::fs::remove_file(format!("{}/{}/{}", PARENT_FOLDER, TMP_FOLDER, LOCK_FILE).as_str());
    process_folder(format!("{}/{}", PARENT_FOLDER, TMP_FOLDER).as_str(), true).map(|_| ())
}

fn add(url_link: &str) -> Result<(), Box<dyn Error>> {
    let conf_path = format!("{}/{}", PARENT_FOLDER, CONFIG_FILE);
    let mut conf = if !std::path::Path::new(conf_path.as_str()).exists() {
        Config {
            repos_to_fetch: vec![],
        }
    } else {
        match read_config(conf_path.as_str()) {
            Ok(conf) => conf,
            Err(e) => {
                println!("error: {:?}", e);
                return Err(e);
            }
        }
    };

    let r = Repo {
        upstream_path: url_link.to_string(),
    };
    conf.repos_to_fetch.push(r);
    let s = serde_json::to_string(&conf)?;
    write_file_content(s, conf_path.as_str());
    Ok(())
}

fn cleanup() -> Result<(), Box<dyn Error>> {
    if !std::path::Path::new(format!("{}/{}/{}", PARENT_FOLDER, TMP_FOLDER, LOCK_FILE).as_str())
        .exists()
    {
        std::fs::remove_dir_all(format!("{}/{}", PARENT_FOLDER, TMP_FOLDER).as_str());
    }
    Ok(())
}

fn update() -> Result<&str, Box<dyn Error>> {
    let message = "";
    match read_config(format!("{}/{}", PARENT_FOLDER, CONFIG_FILE).as_str()) {
        Ok(conf) => {
            for c in conf.repos_to_fetch.iter() {
                match download_file(c.upstream_path.as_str()) {
                    Ok(file_name) => {
                        extract_files(file_name.as_str());
                        std::fs::remove_file(file_name.as_str())?;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            match process_folder(format!("{}/{}", PARENT_FOLDER, TMP_FOLDER).as_str(), false) {
                Ok(true) => {
                    resolve();
                    message = "grafting completed";
                }
                Ok(false) => {
                    let _lock_file = std::fs::File::create(format!(
                        "{}/{}/{}",
                        PARENT_FOLDER, TMP_FOLDER, LOCK_FILE
                    ))?;
                    message = "conflicts found, please resolve and run `graft resolve`";
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(message)
}

fn main() {
    // TODO: add verbosity -V

    let matches = App::new("graft")
        .version("0.1")
        .about("sync your common files across projects")
        .subcommand(
            SubCommand::with_name("add")
                .about("add new upstream remote repository")
                .version("0.1")
                .arg(
                    Arg::with_name("input")
                        .short("i")
                        .takes_value(true)
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("update local files")
                .version("0.1"),
        )
        .subcommand(
            SubCommand::with_name("resolve")
                .about("run to resolve graft state, copies files to parent folder")
                .version("0.1"),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("add") {
        let in_url = matches.value_of("input").unwrap();
        add(in_url);
        match update() {
            Ok(msg) => {
                println!("{}", msg);
            }
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        }
    }
    if let Some(_matches) = matches.subcommand_matches("update") {
        match update() {
            Ok(msg) => {
                println!("{}", msg);
            }
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        }
    }
    if let Some(_matches) = matches.subcommand_matches("resolve") {
        match resolve() {
            Ok(_) => (),
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        }
    }

    cleanup();
}
