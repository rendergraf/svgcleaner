#[macro_use]
extern crate libtest;
extern crate clap;
extern crate time;
extern crate serde_json;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::io::prelude::*;
use std::fs;
use std::fs::File;
use std::collections::HashMap;

use libtest::*;

use clap::{Arg, App};

// NOTE: This app uses a panic-based error processing.
//       It's bad, but simpler for such small applications like this.

// TODO: collect rerendered images

macro_rules! dir_iter {
    ($input_dir:expr) => (
        WalkDir::new($input_dir).into_iter().filter_entry(|x| is_svg(x)).map(|x| x.unwrap())
    )
}

struct Config {
    valid_ae_map: HashMap<String, u32>,
    min_valid_ae: u32,
    ignore_list: Vec<String>,
}

struct Data<'a> {
    work_dir: &'a str,
    svgcleaner: &'a str,
    render: &'a str,
    err_view: Option<&'a str>,
    input_dir: &'a str,
    orig_pngs_dir: &'a str,
    config: Config,
    config_path: Option<&'a str>,
}

fn main() {
    let m = App::new("files-testing")
        .arg(Arg::with_name("workdir")
            .long("workdir").help("Sets path to the work dir")
            .value_name("DIR")
            .required(true))
        .arg(Arg::with_name("input-data")
            .long("input-data").help("Sets path to the SVG files dir")
            .value_name("DIR")
            .required(true))
        .arg(Arg::with_name("cache-db")
            .long("cache-db").help("Sets path to the test cache db.")
            .value_name("PATH")
            .required(true))
        .arg(Arg::with_name("input-data-config")
            .long("input-data-config").help("Sets path to the input data config.")
            .value_name("PATH"))
        .get_matches();

    let err_view_path = Path::new("../err-view/err_view");
    if !err_view_path.exists() {
        println!("Error: {:?} not found.", err_view_path);
        return;
    }

    let config;
    let err_view;
    if m.is_present("input-data-config") {
        let json = load_config(m.value_of("input-data-config").unwrap());
        let min_ae = json.as_object().unwrap().get("min_valid_ae").unwrap().as_f64().unwrap() as u32;

        config = Config {
            valid_ae_map: get_valid_ae_list(&json),
            min_valid_ae: min_ae,
            ignore_list: get_ignore_list(&json),
        };

        err_view = Some(err_view_path.to_str().unwrap());
    } else {
        config = Config {
            valid_ae_map: HashMap::new(),
            min_valid_ae: 0,
            ignore_list: Vec::new(),
        };
        err_view = None;
    }

    let orig_pngs = Path::new(m.value_of("workdir").unwrap()).join("orig_pngs");
    let input_dir = m.value_of("input-data").unwrap();

    let render = Path::new("../svgrender/svgrender");
    if !render.exists() {
        println!("Error: {:?} not found.", render);
        return;
    }

    let svgcleaner_path = Path::new("../../target/release/svgcleaner");
    if !svgcleaner_path.exists() {
        println!("Error: {:?} not found.", svgcleaner_path);
        return;
    }

    let data = Data {
        work_dir: m.value_of("workdir").unwrap(),
        svgcleaner: svgcleaner_path.to_str().unwrap(),
        render: render.to_str().unwrap(),
        err_view: err_view,
        input_dir: input_dir,
        orig_pngs_dir: orig_pngs.to_str().unwrap(),
        config: config,
        config_path: m.value_of("input-data-config"),
    };

    create_dir(&data.work_dir);
    create_dir(&data.orig_pngs_dir);

    let last_pos = load_last_pos(&data.work_dir);
    let cache = TestCache::init(m.value_of("cache-db").unwrap());

    let start = time::precise_time_ns();
    run_tests(&data, input_dir, last_pos, &cache);
    let end = time::precise_time_ns();
    println!("Elapsed: {}s", ((end - start) as f64 / 1000000.0) as u64 / 1000);
}

fn load_last_pos(work_dir: &str) -> usize {
    let mut f = match File::open(Path::new(work_dir).join("pos.txt")) {
        Ok(f) => f,
        Err(_) => return 0,
    };

    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s.trim().parse::<usize>().unwrap()
}

fn save_curr_pos(work_dir: &str, pos: usize) {
    let mut f = fs::File::create(Path::new(work_dir).join("pos.txt")).unwrap();
    f.write_all(pos.to_string().as_bytes()).unwrap();
}

fn load_config(path: &str) -> serde_json::Value {
    let data = load_file(path);
    let value: serde_json::Value = serde_json::from_slice(&data).unwrap();
    value
}

fn get_valid_ae_list(json: &serde_json::Value) -> HashMap<String, u32> {
    let mut map = HashMap::new();

    let items = match json.as_object().unwrap().get("custom_ae") {
        Some(i) => i,
        None => return map,
    };

    for item in items.as_array().unwrap() {
        let obj = item.as_object().unwrap();
        let name = obj.get("name").unwrap().as_str().unwrap();
        match obj.get("valid_ae") {
            Some(v) => {
                match map.insert(name.to_string(), v.as_f64().unwrap() as u32) {
                    Some(_) => {
                        panic!("Error: {} already exist in the list.", name);
                    }
                    None => {}
                }
            }
            None => {}
        }
    }

    map
}

fn get_ignore_list(json: &serde_json::Value) -> Vec<String> {
    let mut vec = Vec::new();

    let items = match json.as_object().unwrap().get("ignore") {
        Some(i) => i,
        None => return vec,
    };

    for item in items.as_array().unwrap() {
        let obj = item.as_object().unwrap();
        let name = obj.get("name").unwrap().as_str().unwrap();
        if vec.iter().any(|n| n == name) {
            panic!("Error: {} already exist.", name);
        }

        vec.push(name.to_string());
    }

    vec
}

fn gen_orig_png_dir(data: &Data, svg_path: &Path) -> PathBuf {
    let sub_path = svg_path.strip_prefix(data.input_dir).unwrap();
    Path::new(data.orig_pngs_dir).join(sub_path)
}

fn run_tests(data: &Data, input_dir: &str, start_pos: usize, cache: &TestCache) {
    let mut total = 0;
    for entry in dir_iter!(input_dir) {
        if entry.file_type().is_file() {
            total += 1;
        }
    }

    let mut idx = 1;
    for entry in dir_iter!(input_dir) {
        if entry.file_type().is_dir() {
            create_dir(gen_orig_png_dir(data, entry.path()));
            continue;
        }

        // skip symlinks
        if !entry.file_type().is_file() {
            continue;
        }

        if idx < start_pos {
            idx += 1;
            continue;
        }

        let file_path = entry.path();
        let sub_path = file_path.strip_prefix(data.input_dir).unwrap();

        println!("Test {} of {}: {}", idx, total, sub_path.display());

        if data.config.ignore_list.iter().any(|ref s| *s == sub_path.to_str().unwrap()) {
            println!("Test skipped.");
            idx += 1;
            continue;
        }

        if !run_test(data, &file_path, cache) {
            println!("Test failed.");
            save_curr_pos(&data.work_dir, idx);
            break;
        } else {
            println!("Test passed.");
        }

        idx += 1;

        if idx == total {
            let pos_path = Path::new(&data.work_dir).join("pos.txt");
            if pos_path.exists() {
                fs::remove_file(pos_path).unwrap();
            }
        }
    }
}

fn run_test(data: &Data, svg_path: &Path, cache: &TestCache) -> bool {
    let svg_path_str = svg_path.to_str().unwrap();
    let orig_file_name = svg_path.file_name().unwrap().to_str().unwrap();
    let new_svg_path = Path::new(data.work_dir).join(orig_file_name);
    let out_path = new_svg_path.to_str().unwrap();
    let svg_suffix = svg_path.strip_prefix(data.input_dir).unwrap().to_str().unwrap();

    if new_svg_path.exists() {
        fs::remove_file(&new_svg_path).unwrap();
    }

    if !clean_svg(data.svgcleaner, svg_path_str, out_path) {
        println!("'svgcleaner' crashed.");
        return false;
    }

    let cache_id = cache.cache_id(svg_suffix);
    match cache_id {
        Some(id) => {
            let new_hash = file_md5sum(out_path);
            if cache.get_hash(id) == new_hash {
                // if md5 is same as in cache - no need to render and compare images
                fs::remove_file(&new_svg_path).unwrap();
                return true;
            }
        }
        None => {}
    }

    let new_png_path = gen_png_path(&out_path, "_new");
    let diff_path = gen_png_path(&out_path, "_diff");

    // render original svg
    let orig_png_path = gen_orig_png_dir(data, Path::new(svg_path)).with_extension("png");
    let orig_png_path_str = orig_png_path.to_str().unwrap();
    if !orig_png_path.exists() {
        if !render_svg(&data.render, svg_path_str, orig_png_path_str) {
            println!("Rendering of the original image is failed.");
            return false;
        }
    }

    if !render_svg(&data.render, &out_path, &new_png_path) {
        println!("Rendering of the cleaned image is failed.");
        return false;
    }

    let diff = match compare_imgs(&data.work_dir, &new_png_path, orig_png_path_str, &diff_path) {
        Some(d) => d,
        None => {
            println!("'compare' failed.");
            return false;
        }
    };

    let valid_ae = match data.config.valid_ae_map.get(svg_suffix) {
        Some(v) => *v,
        None => data.config.min_valid_ae,
    };

    let is_ok = if diff <= valid_ae {
        true
    } else {
        match data.err_view {
            Some(err_view) => {
                let res = Command::new(err_view)
                            .arg(svg_suffix)
                            .arg(orig_png_path_str)
                            .arg(&new_png_path)
                            .arg(&diff_path)
                            .arg(diff.to_string())
                            .arg(data.config_path.unwrap())
                            .status().unwrap();

                res.success()
            }
            None => {
                false
            }
        }
    };

    if is_ok {
        match cache_id {
            Some(id) => cache.update_hash(id, &file_md5sum(&out_path)),
            None => cache.append_hash(svg_suffix, &file_md5sum(&out_path)),
        }

        fs::remove_file(&new_svg_path).unwrap();
        fs::remove_file(new_png_path).unwrap();
        fs::remove_file(diff_path).unwrap();

        return true;
    } else {
        println!("AE: {:?} of {:?}", diff, valid_ae);
        return false;
    }
}

fn clean_svg(exe_path: &str, in_path: &str, out_path: &str) -> bool {
    let res = Command::new(exe_path)
                .arg(in_path)
                .arg(out_path)
                .arg("--indent=2")
                .arg("--copy-on-error=true")
                .arg("--quiet=true")
                .arg("--remove-gradient-attributes=true")
                .arg("--join-arcto-flags=true")
                .arg("--remove-unreferenced-ids=false")
                .arg("--trim-ids=false")
                .output();

    match res {
        Ok(o) => {
            let se: String = String::from_utf8_lossy(&o.stderr).into_owned();
            if !se.is_empty() {
                println!("{}", se);
                return false;
            }

            let mut so: String = String::from_utf8_lossy(&o.stdout).into_owned();
            so = so.trim().to_owned();

            if !so.is_empty() {
                println!("{}", so);
            }

            if so.find("Error").is_some() {
                // list of "not errors"
                if    so.find("Error: Scripting is not supported.").is_some()
                   || so.find("Error: Animation is not supported.").is_some()
                   || so.find("Error: Valid FuncIRI").is_some()
                   || so.find("Error: Broken FuncIRI").is_some()
                   || so.find("Error: Unsupported CSS at").is_some()
                   || so.find("Error: Element crosslink").is_some()
                   || so.find("Error: Conditional processing").is_some()
                   || so.find("Error: The 'xlink:href' attribute").is_some()
                   || so.find("Error: Unsupported ENTITY").is_some()
                   || so.find("Error: The 'use' element with").is_some()
                   || so.find("Error: The attribute 'offset'").is_some()
                   || so.find("Error: Document didn't have any nodes").is_some()
                   || so.find("Error: Invalid color at").is_some()
                   || so.find("Error: Unsupported token at").is_some()
                   || so.find("Error: Invalid length at").is_some()
                   || so.find("Error: Cleaned file is bigger").is_some() {
                    return true;
                }
                return false;
            }

            return true;
        }
        Err(e) => {
            println!("My err: {:?}", e);
            return false;
        }
    }
}
