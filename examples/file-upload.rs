use simpletcp::simpletcp::{Message, TcpServer, TcpStream};
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::io::{stdin, Read};
use std::fs::File;

fn main() {
    let server = TcpServer::new("127.0.0.1:4434").unwrap();
    spawn(|| {
        //Give server some time to start
        sleep(Duration::from_millis(50));
        client_thread();
    });

    server_thread(server);
}

fn server_thread(server: TcpServer) {
    let mut client = server.accept_blocking().unwrap();

    println!("[Server] Accepted new client");
    client.wait_until_ready().unwrap();
    let mut i = 1;
    loop {
        match client.read_blocking() {
            Ok(mut msg) => {
                println!("Received chunk {} size {}",i, msg.read_i32().unwrap());
                i += 1;
            },
            Err(err) => {
                println!("{:?}", err);
                break;
            },
        }

    }
}

fn client_thread() {
    let mut client = TcpStream::connect("127.0.0.1:4434").unwrap();
    client.wait_until_ready().unwrap();
    println!("[Client] Enter file to upload: ");
    let stdin = stdin();
    let mut filename = String::new();
    stdin.read_line(&mut filename).unwrap();
    filename.retain(|c|{
        c != '\n' && c != '\r'
    });

    let mut file = File::open(filename).unwrap();

    let mut i = 0;
    let mut buf = [0;1024*1024];
    loop {
        println!("[Client] Uploading chunk {}", i);
        let length = file.read(&mut buf).unwrap();
        if length == 0 {break;}
        let mut msg = Message::new();
        msg.write_i32(length as i32);
        msg.write_buffer(&buf[..length]);
        client.write_blocking(&msg).unwrap();
        i += 1;
    }
    println!("[Client] Upload complete.");
}
