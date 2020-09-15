use simpletcp::simpletcp::{TcpServer, TcpStream};
use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};
use std::thread::spawn;
use std::net::ToSocketAddrs;

fn main() {
    let keyfile_path = Path::new("server-key");

    let key;
    if keyfile_path.exists(){
        let mut keyfile = File::open(keyfile_path).unwrap();
        let mut buffer = Vec::new();
        keyfile.read_to_end(&mut buffer).unwrap();
        key = Some(buffer);
        println!("Loaded key from file");
    } else {
        key = None;
        println!("Using new key");
    }

    let mut server = TcpServer::new_with_key("127.0.0.1:4234", key.as_deref()).unwrap();

    // Save new key to file
    if key.is_none(){
        let mut keyfile = File::create(keyfile_path).unwrap();
        keyfile.write_all(&server.key()).unwrap();
    }

    try_connect("127.0.0.1:4234", &mut server);
}

fn try_connect<A: ToSocketAddrs>(addr: A, server: &mut TcpServer)  {
    let mut client = TcpStream::connect(addr).unwrap();
    let handle = spawn(move ||{
        client.wait_until_ready().unwrap();
        println!("Connect to server with fingerprint {:?}", client.fingerprint());
    });

    let mut client = server.accept_blocking().unwrap();
    client.wait_until_ready().unwrap();
    handle.join().unwrap();
}