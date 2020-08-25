# simpletcp
Crate for simple and secure TCP communication

## Encryption
All traffic is encrypted with 256-bit AES-CBC

## Initialization
1. Server generates RSA key and sends it to client
1. Client generates AES key, encrypts it with server key and send it to the server
1. From now, all communication is encrypted with 256-bit AES in CBC mode

## Usage
```
//Connect
let mut client = TcpStream::connect("127.0.0.1:4234").unwrap();

//Wait until connection is initialized
client.wait_until_ready().unwrap();

//Build message
let mut msg = Message::new();
msg.write_f64(1.23455);
msg.write_buffer(&[3, 1, 4, 56]);

//Send message
client.write(&msg).unwrap();
```

See `examples`