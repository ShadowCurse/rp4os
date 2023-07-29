#![feature(os_str_bytes)]

use clap::Parser;
use std::{
    io::{stderr, stdin, Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::PathBuf,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const KERNEL_LOAD_START_SIGNAL: u8 = 0x01;
const KERNEL_LOAD_SIZE_ACK_SIGNAL: u8 = 0x02;
const KERNEL_LOAD_ACK_SIGNAL: u8 = 0x03;

const KERNEL_TRANSFER_SPEED_BYTE_PER_SECOND: f64 = 1024.0 * 1024.0;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    device: PathBuf,
    #[arg(short, long)]
    baud: u32,
    #[arg(short, long)]
    kernel: PathBuf,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // setup stdin
    unsafe {
        let stdin = stdin();
        let mut old_tio: libc::termios = std::mem::zeroed();
        libc::tcgetattr(stdin.as_raw_fd(), &mut old_tio);
        let mut new_tio = old_tio;

        // disable canonical mode (buffered i/o) and local echo
        new_tio.c_lflag &= !libc::ICANON & !libc::ECHO;
        libc::tcsetattr(stdin.as_raw_fd(), libc::TCSANOW, &new_tio);
    }

    // setup serial
    let serial_raw = unsafe {
        let serial_raw = {
            let raw = libc::open(
                cli.device.as_os_str().as_os_str_bytes().as_ptr().cast(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            );
            if raw < 0 {
                println!("Can not open {:?}", cli.device);
                return;
            }
            raw
        };

        // must be tty
        if libc::isatty(serial_raw) != 1 {
            return;
        }

        let mut serial_tio: libc::termios = std::mem::zeroed();
        libc::tcgetattr(serial_raw, &mut serial_tio);

        // poll
        serial_tio.c_cc[libc::VTIME] = 0;
        serial_tio.c_cc[libc::VMIN] = 0;

        // 8N1 mode, no input/output/line processing masks.
        serial_tio.c_iflag = 0;
        serial_tio.c_oflag = 0;
        serial_tio.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL;
        serial_tio.c_lflag = 0;

        libc::cfsetispeed(&mut serial_tio, cli.baud);
        libc::cfsetospeed(&mut serial_tio, cli.baud);

        libc::tcsetattr(serial_raw, libc::TCSAFLUSH, &serial_tio);

        serial_raw
    };

    let mut async_stdin = unsafe { tokio::fs::File::from_raw_fd(stdin().as_raw_fd()) };
    let mut async_serial = unsafe { tokio::fs::File::from_raw_fd(serial_raw) };

    let mut buff = Vec::new();

    loop {
        tokio::select! {
            val = async_stdin.read_u8() => {
                match val {
                    Ok(val) => stdin_action(val, &cli.kernel, &mut async_serial).await,
                    Err(e) => eprintln!("async_stdin error: {:?}", e),
                }
            }
            val = async_serial.read_to_end(&mut buff) => {
                match val {
                    Ok(bytes) => {
                        serial_action(&mut buff, bytes).await;
                    }
                    Err(e) => {
                        eprintln!("async_serial error: {:?}", e);
                    }
                }
            }
        }
    }
}

async fn stdin_action(val: u8, kernel_path: &PathBuf, async_serial: &mut tokio::fs::File) {
    // if pressed `1`
    if val == 49 {
        send_kernel(kernel_path, async_serial).await;
    } else {
        let _ = async_serial.write_u8(val).await;
    }
}

async fn serial_action(buff: &mut Vec<u8>, read: usize) {
    if read != 0 {
        let _ = stderr().write(&buff[0..read]);
        buff.clear();
    }
}

async fn send_kernel(kernel_path: &PathBuf, async_serial: &mut tokio::fs::File) {
    eprintln!("Uploading kernel...");
    match std::fs::File::open(kernel_path) {
        Ok(mut file) => {
            let mut kernel = Vec::new();
            let _ = file.read_to_end(&mut kernel);

            eprintln!("Notifing loader...");
            let _ = async_serial.write_u8(KERNEL_LOAD_START_SIGNAL).await;

            eprintln!("Writing kernel size: {} bytes...", kernel.len());
            for i in 0..4 {
                let c = ((kernel.len() >> (8 * i)) & 0xFF) as u8;
                let _ = async_serial.write_u8(c).await;
            }

            let mut buff = Vec::new();
            while async_serial.read_to_end(&mut buff).await.unwrap() == 0 {}
            if buff != [KERNEL_LOAD_SIZE_ACK_SIGNAL] {
                eprintln!("Did not receive responce to kernel size: {:?}", buff);
                return;
            }
            eprintln!("Recieved kernel size ack...");

            eprintln!(
                "Sending kernel with speed: {} KB/s ...",
                KERNEL_TRANSFER_SPEED_BYTE_PER_SECOND / 1024.0
            );
            let now = std::time::Instant::now();
            for (i, byte) in kernel.iter().enumerate() {
                eprint!("\x1b[GSending {}/{} byte", i, kernel.len());
                let _ = async_serial.write_u8(*byte).await;
                std::thread::sleep(std::time::Duration::from_secs_f64(
                    1.0 / KERNEL_TRANSFER_SPEED_BYTE_PER_SECOND,
                ));
            }
            eprintln!("\n Time took: {:#?}", now.elapsed());

            while async_serial.read_to_end(&mut buff).await.unwrap() == 0 {}
            if buff != [KERNEL_LOAD_ACK_SIGNAL] {
                eprintln!("Did not receive responce to kernel successuf upload");
            } else {
                eprintln!("Recieved kernel rcv ack...");
            }
        }
        Err(e) => eprintln!("Couldn't upload kernel: {:?}", e),
    }
}
