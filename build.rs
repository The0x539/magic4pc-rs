use std::path::Path;

fn main() {
    if !Path::new("./devices.txt").exists() {
        let msg = [
            "Could not find a `devices.txt` file.",
            "Please create the file (in the root of the repository),",
            "and add the IP and/or MAC address(es) of your TV(s).",
            "(One address per line.)",
        ];
        for line in msg {
            println!("cargo::error={line}");
        }
    }
}
