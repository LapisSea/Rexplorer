use std::{env, fs, io};
use std::fs::{File, FileType, Metadata};
use std::path::Path;
use std::time::SystemTime;

use slint_build::*;

fn main() {
	let path = "src/ui/start.slint";
	
	let conf = CompilerConfiguration::new()
		// .with_style("material".into())
		;
	
	compile_with_config(path, conf).unwrap();
}
