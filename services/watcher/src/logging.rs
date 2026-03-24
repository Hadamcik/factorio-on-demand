pub fn log(msg: impl AsRef<str>) {
    println!("{}", msg.as_ref());
}
