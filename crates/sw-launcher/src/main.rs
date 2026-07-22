fn main() {
    let status = sw_launcher::status();
    println!("sw-launcher ready: {}", status.ready);
}
