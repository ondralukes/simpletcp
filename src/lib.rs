pub mod simpletcp {
    use std::convert::TryInto;
    use std::fmt;
    use std::fmt::Formatter;
    use std::io;
    use std::io::{ErrorKind, Read, Write};
    use std::net;
    use std::net::ToSocketAddrs;

    extern crate openssl;
    use openssl::error::ErrorStack;
    use openssl::pkey::Private;
    use openssl::rsa::Padding;
    use openssl::rsa::Rsa;
    use openssl::symm;
    use openssl::symm::Cipher;

    extern crate rand;
    use rand::prelude::StdRng;
    use rand::RngCore;
    use rand::SeedableRng;

    use State::{NotInitialized, Ready, WaitingForPublicKey, WaitingForSymmKey};

    #[cfg(test)]
    mod tests;

    pub enum Error {
        NotReady,
        EncryptionError(ErrorStack),
        TcpError(io::Error),
    }

    impl fmt::Debug for Error {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            return match self {
                Error::NotReady => f.write_str("Error::NotReady"),
                Error::EncryptionError(openssl_err) => {
                    f.write_fmt(format_args!("Error::EncryptionError: {}", openssl_err))
                }
                Error::TcpError(io_err) => f.write_fmt(format_args!("Error::TcpError: {}", io_err)),
            };
        }
    }

    impl From<io::Error> for Error {
        fn from(io_err: io::Error) -> Self {
            Error::TcpError(io_err)
        }
    }

    impl From<ErrorStack> for Error {
        fn from(openssl_err: ErrorStack) -> Self {
            Error::EncryptionError(openssl_err)
        }
    }

    #[derive(PartialEq)]
    pub enum State {
        NotInitialized,
        WaitingForPublicKey,
        WaitingForSymmKey,
        Ready,
    }

    pub struct TcpServer {
        socket: net::TcpListener,
        key: Rsa<Private>,
    }

    impl TcpServer {
        pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
            let key = Rsa::generate(4096)?;
            Self::new_with_key(addr, key)
        }
        pub fn new_with_key<A: ToSocketAddrs>(addr: A, key: Rsa<Private>) -> Result<Self, Error> {
            let socket = net::TcpListener::bind(addr)?;
            socket.set_nonblocking(true)?;
            return Ok(Self { socket, key });
        }

        pub fn accept(&self) -> Result<Option<TcpStream>, Error> {
            match self.socket.accept() {
                Ok((socket, _addr)) => {
                    let mut stream = TcpStream::from_socket(socket)?;
                    stream.server_init(&self.key)?;
                    Ok(Some(stream))
                }
                Err(io_err) => match io_err.kind() {
                    ErrorKind::WouldBlock => Ok(None),
                    _ => Err(Error::TcpError(io_err)),
                },
            }
        }
    }

    macro_rules! try_io {
        ($r: expr, $wb_closure: expr) => {
            match $r {
                Ok(res) => res,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => {
                        $wb_closure();
                        return Ok(None);
                    }
                    _ => {
                        return Err(Error::TcpError(e));
                    }
                },
            }
        };
    }

    pub struct TcpStream {
        socket: net::TcpStream,
        buffer: Vec<u8>,
        key: [u8; 32],
        state: State,
        rsa_key: Option<Rsa<Private>>,
        rand: StdRng,
    }

    impl TcpStream {
        fn from_socket(socket: net::TcpStream) -> Result<Self, Error> {
            socket.set_nonblocking(true)?;
            Ok(Self {
                socket,
                buffer: Vec::new(),
                key: Default::default(),
                state: NotInitialized,
                rsa_key: None,
                rand: StdRng::from_entropy(),
            })
        }

        pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
            let socket = net::TcpStream::connect(addr)?;
            socket.set_nonblocking(true)?;

            Ok(Self {
                socket,
                buffer: Vec::new(),
                key: Default::default(),
                state: WaitingForPublicKey,
                rsa_key: None,
                rand: StdRng::from_entropy(),
            })
        }

        fn server_init(&mut self, rsa_key: &Rsa<Private>) -> Result<(), Error> {
            self.write_raw(&rsa_key.public_key_to_der()?)?;
            self.rsa_key = Some(rsa_key.clone());
            self.state = WaitingForSymmKey;
            Ok(())
        }

        fn init_step(&mut self) -> Result<(), Error> {
            match self.state {
                NotInitialized => panic!("TcpStream init_step state NotInitialized"),
                WaitingForPublicKey => match self.read_raw()? {
                    Some(rsa_key) => {
                        self.rand.fill_bytes(&mut self.key);

                        let rsa_key = Rsa::public_key_from_der(&rsa_key)?;
                        let mut encrypted_key: Vec<u8> = vec![0; rsa_key.size() as usize];
                        let encrypted_size = rsa_key.public_encrypt(
                            &self.key,
                            &mut encrypted_key,
                            Padding::PKCS1_OAEP,
                        )?;
                        encrypted_key.resize(encrypted_size, 0);
                        self.write_raw(&encrypted_key)?;
                        self.state = Ready;
                    }
                    None => {}
                },
                WaitingForSymmKey => match self.read_raw()? {
                    Some(encrypted_key) => {
                        let rsa_key = self.rsa_key.as_ref().unwrap();
                        let mut key: Vec<u8> = vec![0; rsa_key.size() as usize];
                        let key_size = rsa_key.private_decrypt(
                            &encrypted_key,
                            &mut key,
                            Padding::PKCS1_OAEP,
                        )?;
                        key.resize(key_size, 0);
                        assert_eq!(key_size, 32);

                        self.key.copy_from_slice(&key);
                        self.state = Ready;
                    }
                    None => {}
                },
                _ => {}
            }

            Ok(())
        }

        pub fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
            if self.state != Ready {
                return Ok(None);
            }

            match self.read_raw()? {
                None => Ok(None),
                Some(buf) => {
                    let iv = &buf[..16];
                    let decrypted =
                        symm::decrypt(Cipher::aes_256_cbc(), &self.key, Some(iv), &buf[16..])?;

                    Ok(Some(decrypted))
                }
            }
        }

        pub fn write(&mut self, msg: &[u8]) -> Result<(), Error> {
            if self.state != Ready {
                return Err(Error::NotReady);
            }

            let mut iv = [0; 16];
            self.rand.fill_bytes(&mut iv);

            let mut encrypted = symm::encrypt(Cipher::aes_256_cbc(), &self.key, Some(&iv), msg)?;

            let mut raw = iv.to_vec();
            raw.append(&mut encrypted);
            self.write_raw(&raw)
        }

        pub fn wait_until_ready(&mut self) -> Result<(), Error> {
            while !self.get_ready()? {
                //TODO: Poll the socket or something like that
            }

            Ok(())
        }

        pub fn get_ready(&mut self) -> Result<bool, Error> {
            if self.state == Ready {
                return Ok(true);
            }
            match self.init_step() {
                Err(e) => match e {
                    Error::TcpError(io_err) if io_err.kind() == ErrorKind::WouldBlock => {
                        return Ok(false)
                    }
                    _ => {}
                },
                _ => {}
            }

            Ok(self.state == Ready)
        }

        fn write_raw(&mut self, msg: &[u8]) -> Result<(), Error> {
            let length = msg.len() as u32;
            self.socket.write(&length.to_le_bytes())?;
            self.socket.write(msg)?;

            Ok(())
        }

        fn read_raw(&mut self) -> Result<Option<Vec<u8>>, Error> {
            if self.buffer.len() < 4 {
                let start = self.buffer.len();
                self.buffer.resize(4, 0);
                let bytes_read = try_io!(self.socket.read(&mut self.buffer[start..]), || {
                    self.buffer.resize(start, 0);
                });

                self.buffer.resize(start + bytes_read, 0);
                if self.buffer.len() != 4 {
                    return Ok(None);
                }
            }

            let len = u32::from_le_bytes(self.buffer[..4].try_into().unwrap()) as usize;

            let start = self.buffer.len();
            self.buffer.resize(4 + len, 0);
            let bytes_read = try_io!(self.socket.read(&mut self.buffer[start..]), || {
                self.buffer.resize(start, 0);
            });
            self.buffer.resize(start + bytes_read, 0);
            if self.buffer.len() == len + 4 {
                let result = self.buffer[4..].to_vec();
                self.buffer.clear();
                return Ok(Some(result));
            }

            Ok(None)
        }
    }
}
