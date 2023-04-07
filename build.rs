use slint_build::*;

fn main() {
	let path = "src/ui/start.slint";
	
	let conf = CompilerConfiguration::new()
		// .with_style("material".into())
		;
	
	compile_with_config(path, conf).unwrap();
}
