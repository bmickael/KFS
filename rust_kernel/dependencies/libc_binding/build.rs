use std::process::Command;

fn main() {
    let out = Command::new("make").output().unwrap();
    if !out.status.success() {
        panic!("{:?}", String::from_utf8(out.stderr));
    }
}
