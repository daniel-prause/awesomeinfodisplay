use serialport;
use std::cmp;
use std::io::Write;
use std::time::Duration;

pub fn init_serial(
    device_string: &String,
    baud: u32,
) -> Option<std::boxed::Box<dyn serialport::SerialPort>> {
    let ports = serialport::available_ports().expect("No ports found!");

    if ports.len() == 0 {
        return None;
    }
    for p in ports.clone() {
        match p.port_type {
            serialport::SerialPortType::UsbPort(info) => {
                let comp = format!("{:04x}{:04x}", info.vid, info.pid);
                if device_string.eq(&comp) {
                    let port = match serialport::new(p.port_name, baud)
                        .timeout(Duration::from_millis(1000))
                        .open()
                    {
                        Ok(port) => Some(port),
                        Err(_) => continue,
                    };
                    return port;
                }
            }
            _ => {}
        }
    }
    return None;
}

pub fn write_screen_buffer(
    port: &mut Option<std::boxed::Box<dyn serialport::SerialPort>>,
    screen_buf: &[u8],
) -> bool {
    let mut bytes_send = 0;
    while bytes_send < screen_buf.len() {
        let slice = &screen_buf[bytes_send..cmp::min(bytes_send + 64, screen_buf.len())];
        bytes_send += slice.len();

        if port.as_deref_mut().is_some() {
            match port.as_deref_mut().unwrap().write(&slice) {
                Ok(_) => {
                    // everything alright, continue
                }
                Err(_) => {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    return true;
}

pub fn read_bme_sensor(port: &mut Option<std::boxed::Box<dyn serialport::SerialPort>>) -> String {
    let mut data: [u8; 14] = [0; 14];
    if port.as_deref_mut().is_some() {
        match port.as_deref_mut().unwrap().read_exact(&mut data) {
            Ok(_) => {
                // everything alright,
                return std::str::from_utf8(&data).unwrap().to_string();
            }
            Err(_) => {
                return String::new();
            }
        }
    }
    return String::new();
}

pub fn send_command(
    port: &mut Option<std::boxed::Box<dyn serialport::SerialPort>>,
    command: &[u8],
) -> bool {
    if port.as_deref_mut().is_some() {
        match port.as_deref_mut().unwrap().write(command) {
            Ok(_) => match std::io::stdout().flush() {
                Ok(_) => {
                    return true;
                }
                Err(_) => {}
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("{:?}", e),
        }
    }

    return false;
}