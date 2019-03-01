use colored::*;
use getopts::Options;
use std::env;
use std::fs::File;
use std::io::Read;
use std::process::Command;
use std::time::Duration;
use toml::Value;
use wait_timeout::ChildExt;

const TIMEOUT: Duration = Duration::from_secs(20);

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} FILE [options]", program);
    eprint!("{}", opts.usage(&brief));
}

enum TestError {
    Failed,
    Timeout,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("g", "graphical", "launch qemu with console");
    opts.optflag("", "nocapture", "show output even if test succeed");
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(_f) => {
            print_usage(&program, opts);
            std::process::exit(1);
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let all_tests = if !matches.free.is_empty() {
        matches.free.clone()
    } else {
        let mut file = File::open("./Cargo.toml").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let value = contents.parse::<Value>().unwrap();
        let btree = match &value["features"] {
            Value::Table(btree) => btree,
            _ => panic!("not a btree"),
        };
        btree.into_iter().filter_map(|f| if f.0.starts_with("test-") { Some(f.0.clone()) } else { None }).collect()
    };
    println!("running {} tests", all_tests.len());
    let all_result: Vec<Result<(), TestError>> = all_tests
        .iter()
        .map(|feature| {
            println!("test: {}", (*feature).clone().magenta().bold());
            let compilation_output = {
                let mut cmd = Command::new("make");
                cmd.args(&[
                    "DEBUG=yes",
                    &format!(
                        "cargo_flags=--features {},test,{}",
                        feature,
                        if matches.opt_present("g") { "qemu-graphical" } else { "" }
                    ),
                ]);

                println!("{} {:?}", "EXECUTING".blue().bold(), cmd);
                cmd.output().expect("failed to execute process")
            };
            println!("COMPILATION stdout {}", String::from_utf8_lossy(&compilation_output.stdout));
            println!("COMPILATION stderr {}", String::from_utf8_lossy(&compilation_output.stderr));
            let output_file = format!("{}/test-output/{}", env!("PWD"), format!("{}-output", feature));
            let mut child = {
                let mut qemu_command = Command::new("qemu-system-x86_64");
                qemu_command
                    .args(&["--enable-kvm", "-cpu", "IvyBridge", "-m", "128M", "-kernel", "build/kernel.elf"])
                    .args(&["-serial", &format!("file:{}", output_file)])
                    .args(&["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
                    .args(if matches.opt_present("g") { [].iter() } else { ["-display", "none"].iter() });
                println!("{}: {:?}", "EXECUTING".blue().bold(), qemu_command);
                qemu_command.spawn().expect("failed to execute process")
            };

            let show_output = || {
                let mut output = String::new();
                File::open(output_file).unwrap().read_to_string(&mut output).unwrap();
                println!("{}: {}", "OUTPUT".blue().bold(), output);
            };

            match child.wait_timeout(TIMEOUT) {
                Err(e) => panic!("Internal error: {}", e),
                Ok(Some(exit_status)) => {
                    let exit_status = exit_status.code().unwrap() >> 1;
                    if exit_status != 0 {
                        show_output();
                        println!("{}", "Failed".red().bold());
                        Err(TestError::Failed)
                    } else {
                        if matches.opt_present("nocapture") {
                            show_output();
                        }
                        println!("{}", "Ok".green().bold());
                        Ok(())
                    }
                }
                Ok(None) => {
                    child.kill().expect("cant kill");
                    show_output();
                    println!("{}", "TIMEOUT".red().bold());
                    Err(TestError::Timeout)
                }
            }
        })
        .collect();
    let total_succeed = all_result.iter().filter(|r| r.is_ok()).count();
    let total_failed = all_result.iter().filter(|r| r.is_err()).count();
    println!(
        "test result: {} {} passed; {} failed",
        if total_succeed == all_tests.len() { "SUCCEED".green().bold() } else { "FAILED".red().bold() },
        total_succeed,
        total_failed
    );
}
